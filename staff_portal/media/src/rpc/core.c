#include <amqp_tcp_socket.h>
#include "rpc/core.h"

// rabbitmq-c handles heartbeat frames internally (TODO, figure out how) to see whether a
// given connection is active, some of primary API functions will return AMQP_STATUS_HEARTBEAT_TIMEOUT
// in case that the connection is inactive (closed) before invoking this function
#define APP_AMQP_HEARTBEAT_DEFAULT_SECONDS  30
// Note: limitation of librabbitmq:
//       https://github.com/alanxz/rabbitmq-c#writing-applications-using-librabbitmq
//
// TODO, figure out how to make non-blocking call to publish function, in order to
// use different channels sending data for different HTTP requests within a single
// TCP connection (to AMQP broker)
#define APP_AMQP_CHANNEL_DEFAULT_ID  1

struct arpc_conn_t {
    arpc_cfg_t  *ref_cfg;
    amqp_socket_t *sock;
    amqp_connection_state_t  conn;
};

struct arpc_internal_ctx_t {
    size_t size;
    struct arpc_conn_t *entries;
};


static  amqp_status_enum  apprpc_msgq_conn_init(struct arpc_conn_t *item)
{
    amqp_status_enum  status = AMQP_STATUS_OK;
    arpc_cfg_t *cfg = item->ref_cfg;
    status = amqp_socket_open(item->sock, cfg->credential.host, cfg->credential.port);
    if(status != AMQP_STATUS_OK) {
        fprintf(stderr, "[RPC] connection failure %s:%hu \n", cfg->credential.host, cfg->credential.port );
        goto done;
    }
    {
        int  max_nbytes_per_frame = (int) cfg->attributes.max_kb_per_frame << 10;
        amqp_rpc_reply_t  _reply = amqp_login(item->conn, cfg->attributes.vhost,
                cfg->attributes.max_channels, max_nbytes_per_frame, APP_AMQP_HEARTBEAT_DEFAULT_SECONDS,
                AMQP_SASL_METHOD_PLAIN, cfg->credential.username, cfg->credential.password );
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            fprintf(stderr, "[RPC] authentication failure %s@%s:%hu \n", cfg->credential.username,
                    cfg->credential.host, cfg->credential.port );
            status = AMQP_STATUS_INVALID_PARAMETER;
            goto done;
        } // TODO, figure out where to dig more error detail
    }
    // AMQP channel should be long-lived , not for each operation (e.g. HTTP request)
    amqp_channel_open_ok_t *chn_res = amqp_channel_open(item->conn, APP_AMQP_CHANNEL_DEFAULT_ID);
    if(!chn_res || !chn_res->channel_id.bytes) {
        fprintf(stderr, "[RPC] failed to open default channel %s:%hu \n", cfg->credential.host,
                cfg->credential.port );
        status = AMQP_STATUS_NO_MEMORY;
    }
done:
    return status;
} // end of apprpc_msgq_conn_init

static  void apprpc_ensure_send_queue(struct arpc_conn_t *item)
{
    size_t idx = 0;
    arpc_cfg_t *cfg = item->ref_cfg;
    amqp_rpc_reply_t _reply = {0};
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *bind_cfg = &cfg->bindings.entries[0];
        // TODO , amqp_table_t doesn't seem to work in amqp_queue_declare(...) , figure
        // out how to make it work
        //// amqp_table_entry_t  *q_arg_n_elms = {0}; //  malloc(sizeof(amqp_table_entry_t));
        //// *q_arg_n_elms = (amqp_table_entry_t) {.key = amqp_cstring_bytes("x-max-length"),
        ////         .value = {.kind = AMQP_FIELD_KIND_U32, .value = {.u32 = bind_cfg->max_msgs_pending}}};
        //// amqp_table_t  q_arg_table = {.num_entries=1, .entries=q_arg_n_elms};
        amqp_queue_declare( item->conn, APP_AMQP_CHANNEL_DEFAULT_ID,
                amqp_cstring_bytes(bind_cfg->q_name), (amqp_boolean_t)bind_cfg->flags.passive,
                (amqp_boolean_t)bind_cfg->flags.durable, (amqp_boolean_t)bind_cfg->flags.exclusive,
                (amqp_boolean_t)bind_cfg->flags.auto_delete, amqp_empty_table // q_arg_table
            );
        _reply = amqp_get_rpc_reply(item->conn);
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            fprintf(stderr, "[RPC] fail to declare a queue : %s ", bind_cfg->q_name);
            if(_reply.reply_type == AMQP_RESPONSE_SERVER_EXCEPTION) {
                // TODO, separate channel error and connection error
                amqp_channel_close_t *m = (amqp_channel_close_t *)_reply.reply.decoded;
                fprintf(stderr, ", reason: server channel error %uh, message: %.*s ",
                     m->reply_code, (int)m->reply_text.len, (char *)m->reply_text.bytes);
            } else if(_reply.reply_type == AMQP_RESPONSE_LIBRARY_EXCEPTION) {
                const char *errmsg = amqp_error_string2(_reply.library_error);
                fprintf(stderr, ", reason: library error, %s ", errmsg);
            }
            fprintf(stderr, "\n");
            continue;
        }
        amqp_queue_bind( item->conn, APP_AMQP_CHANNEL_DEFAULT_ID,
                amqp_cstring_bytes(bind_cfg->q_name),  amqp_cstring_bytes(bind_cfg->exchange_name),
                amqp_cstring_bytes(bind_cfg->routing_key),  amqp_empty_table);
        _reply = amqp_get_rpc_reply(item->conn);
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            fprintf(stderr, "[RPC] fail to bind the routing key (%s) with the queue (%s) \n",
                    bind_cfg->routing_key, bind_cfg->q_name);
        }
    } // end of loop
} // end of apprpc_ensure_send_queue

static void apprpc_conn_deinit__per_item(struct arpc_conn_t *item) {
    // channels within a TCP connection will be automatically closed as soon as
    // the TCP connection is closed , no need to do it explicitly. 
    if(item->conn) {
        amqp_connection_close(item->conn, AMQP_REPLY_SUCCESS);
        amqp_destroy_connection(item->conn);
        item->conn = NULL;
        item->sock = NULL;
    } else if(item->sock) { // in case the connection wasn't created properly
        free(item->sock);
        item->sock = NULL;
    }
} // end of apprpc_conn_deinit__per_item

void * app_rpc_conn_init(arpc_cfg_t *cfgs, size_t nitem) {
    if(!cfgs || nitem == 0) {
        return NULL;
    }
    size_t ctx_sz = sizeof(struct arpc_internal_ctx_t) + nitem * sizeof(struct arpc_conn_t);
    struct arpc_internal_ctx_t *ctx = (struct arpc_internal_ctx_t *) malloc(ctx_sz);
    size_t idx = 0;
    memset(ctx, 0, ctx_sz);
    ctx->size = nitem;
    ctx->entries = (struct arpc_conn_t *) ((char *)ctx + sizeof(struct arpc_internal_ctx_t));
    for(idx = 0; idx < nitem; idx++) {
        struct arpc_conn_t *item = &ctx->entries[idx];
        item->ref_cfg = &cfgs[idx];
        item->conn = amqp_new_connection();
        item->sock = amqp_tcp_socket_new(item->conn); // socket will be deleted once connection is closed
        if(!item->conn || !item->sock) {
            fprintf(stderr, "[RPC][init] missing connection object or  TCP socket at entry %ld \n", idx);
            continue;
        }
        if(apprpc_msgq_conn_init(item) != AMQP_STATUS_OK) {
            fprintf(stderr, "[RPC][init] login failure at entry %ld \n", idx);
            apprpc_conn_deinit__per_item(item);
            continue;
        }
        apprpc_ensure_send_queue(item);
    } // end of loop
    return (void *)ctx;
} // end of app_rpc_conn_init


void app_rpc_conn_deinit(void *conn) {
    if(!conn) { return; }
    struct arpc_internal_ctx_t *ctx = (struct arpc_internal_ctx_t *) conn;
    size_t idx = 0;
    for(idx = 0; idx < ctx->size; idx++) {
        apprpc_conn_deinit__per_item( &ctx->entries[idx] );
    }
    free(ctx);
} // end of app_rpc_conn_deinit


static struct arpc_conn_t *apprpc_msgq_cfg_lookup(struct arpc_internal_ctx_t *ctx, const char *alias)
{
    struct arpc_conn_t *chosen = &ctx->entries[0];
    return chosen;
} // end of apprpc_msgq_cfg_lookup

static arpc_cfg_bind_t *apprpc_msgq_bind_cfg_lookup(arpc_cfg_t *cfg, const char *routing_key)
{
    arpc_cfg_bind_t *chosen = &cfg->bindings.entries[0];
    return chosen;
} // end of apprpc_msgq_bind_cfg_lookup

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *args)
{
    ARPC_STATUS_CODE  app_status = APPRPC_RESP_ACCEPTED;
    if(!args || !args->job_id || !args->msg_body.bytes || !args->routing_key || !args->conn
            || !args->alias) {
        app_status = APPRPC_RESP_ARG_ERROR;
        goto done;
    }
    struct arpc_internal_ctx_t *ctx = (struct arpc_internal_ctx_t *) args->conn;
    struct arpc_conn_t *mq_cfg = apprpc_msgq_cfg_lookup(ctx, args->alias);
    if(!mq_cfg) {
        app_status = APPRPC_RESP_ARG_ERROR;
        goto done;
    }
    arpc_cfg_bind_t *bind_cfg = apprpc_msgq_bind_cfg_lookup(mq_cfg->ref_cfg, args->routing_key);
    if(!bind_cfg) {
        app_status = APPRPC_RESP_ARG_ERROR;
        goto done;
    }
    // TODO:
    // * publish message , if receiving heartbeat timeout , than login and open channel again
    memcpy(args->job_id, "aC1o3k", 6);
done:
    return app_status;
} // end of app_rpc_start



ARPC_STATUS_CODE app_rpc_get_reply(arpc_exe_arg_t *args)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_ACCEPTED;
    return status;
} // end of app_rpc_get_reply
