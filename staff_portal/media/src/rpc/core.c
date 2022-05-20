#include "rpc/core.h"

// Note: limitation of librabbitmq:
//       https://github.com/alanxz/rabbitmq-c#writing-applications-using-librabbitmq
//
// TODO, figure out how to make non-blocking call to publish function, in order to
// use different channels sending data for different HTTP requests within a single
// TCP connection (to AMQP broker)
#define APP_AMQP_CHANNEL_DEFAULT_ID  1


static  amqp_status_enum  apprpc_msgq_conn_init(struct arpc_ctx_t *item)
{
    amqp_status_enum  status = AMQP_STATUS_OK;
    arpc_cfg_t *cfg = item->ref_cfg;
    status = amqp_socket_open(item->sock, cfg->credential.host, cfg->credential.port);
    if(status != AMQP_STATUS_OK) {
        fprintf(stderr, "[RPC] connection failure %s:%hu \n", cfg->credential.host, cfg->credential.port );
        goto done;
    }
    // rabbitmq-c handles heartbeat frames internally in its API functions to see whether a
    // given connection is active, some of primary API functions will return AMQP_STATUS_HEARTBEAT_TIMEOUT
    // in case that the connection is inactive (closed) before invoking this function
    {
        int  max_nbytes_per_frame = (int) cfg->attributes.max_kb_per_frame << 10;
        amqp_rpc_reply_t  _reply = amqp_login(item->conn, cfg->attributes.vhost,
                cfg->attributes.max_channels, max_nbytes_per_frame, cfg->attributes.timeout_secs,
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

static void apprpc_declare_q_report_error(amqp_rpc_reply_t *reply, char *q_name)
{
    fprintf(stderr, "[RPC] fail to declare a queue : %s ", q_name);
    if(reply->reply_type == AMQP_RESPONSE_SERVER_EXCEPTION) {
        // TODO, separate channel error and connection error
        amqp_channel_close_t *m = (amqp_channel_close_t *)reply->reply.decoded;
        fprintf(stderr, ", reason: server channel error %uh, message: %.*s ",
             m->reply_code, (int)m->reply_text.len, (char *)m->reply_text.bytes);
    } else if(reply->reply_type == AMQP_RESPONSE_LIBRARY_EXCEPTION) {
        const char *errmsg = amqp_error_string2(reply->library_error);
        fprintf(stderr, ", reason: library error, %s ", errmsg);
    }
    fprintf(stderr, "\n");
} // end of apprpc_declare_q_report_error

static  void apprpc_ensure_send_queue(struct arpc_ctx_t *item)
{
    size_t idx = 0;
    arpc_cfg_t *cfg = item->ref_cfg;
    amqp_rpc_reply_t _reply = {0};
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *bind_cfg = &cfg->bindings.entries[0];
        // AMQP broker does NOT allow  unsigned number as argument when declaring a queue ? find out the source(TODO)
         amqp_table_entry_t  q_arg_n_elms = {.key = amqp_cstring_bytes("x-max-length"),
                 .value = {.kind = AMQP_FIELD_KIND_I32, .value = {.i32 = bind_cfg->max_msgs_pending}}};
         amqp_table_t  q_arg_table = {.num_entries=1, .entries=&q_arg_n_elms};
        amqp_queue_declare( item->conn, APP_AMQP_CHANNEL_DEFAULT_ID,
                amqp_cstring_bytes(bind_cfg->q_name), (amqp_boolean_t)bind_cfg->flags.passive,
                (amqp_boolean_t)bind_cfg->flags.durable, (amqp_boolean_t)bind_cfg->flags.exclusive,
                (amqp_boolean_t)bind_cfg->flags.auto_delete, q_arg_table
            );
        _reply = amqp_get_rpc_reply(item->conn);
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            apprpc_declare_q_report_error(&_reply, bind_cfg->q_name);
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

static ARPC_STATUS_CODE  apprpc_ensure_reply_queue(amqp_connection_state_t raw_conn,
        arpc_cfg_bind_reply_t *reply_cfg, char *q_name) {
    // Currently, reply queue is always bound with default exchange (specify empty
    // exchange name) , in case some AMQP brokers sends return value back to the reply
    // queue ONLY using default exchange. (TODO: switch to non-default exchange)
    ARPC_STATUS_CODE  app_status = APPRPC_RESP_OK;
    amqp_queue_declare( raw_conn, APP_AMQP_CHANNEL_DEFAULT_ID,
            amqp_cstring_bytes(q_name), (amqp_boolean_t)reply_cfg->flags.passive,
            (amqp_boolean_t)reply_cfg->flags.durable, (amqp_boolean_t)reply_cfg->flags.exclusive,
            (amqp_boolean_t)reply_cfg->flags.auto_delete, amqp_empty_table );
    amqp_rpc_reply_t _reply = amqp_get_rpc_reply(raw_conn);
    if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
        apprpc_declare_q_report_error(&_reply, q_name);
        app_status = APPRPC_RESP_MSGQ_REMOTE_UNKNOWN_ERROR;
    }
    return app_status;
} // end of apprpc_ensure_reply_queue

static void apprpc_conn_deinit__per_item(struct arpc_ctx_t *item) {
    // channels within a TCP connection will be automatically closed as soon as
    // the TCP connection is closed , no need to do it explicitly. 
    if(item->conn) {
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
    size_t ctx_sz = sizeof(struct arpc_ctx_list_t) + nitem * sizeof(struct arpc_ctx_t);
    struct arpc_ctx_list_t *ctx = (struct arpc_ctx_list_t *) malloc(ctx_sz);
    size_t idx = 0;
    memset(ctx, 0, ctx_sz);
    ctx->size = nitem;
    ctx->entries = (struct arpc_ctx_t *) ((char *)ctx + sizeof(struct arpc_ctx_list_t));
    for(idx = 0; idx < nitem; idx++) {
        struct arpc_ctx_t *item = &ctx->entries[idx];
        item->ref_cfg = &cfgs[idx];
        item->conn = amqp_new_connection();
        item->sock = amqp_tcp_socket_new(item->conn); // socket will be deleted once connection is closed
        if(!item->conn || !item->sock) {
            fprintf(stderr, "[RPC][init] missing connection object or  TCP socket at entry %ld \n", idx);
            continue;
        }
        if(apprpc_msgq_conn_init(item) != AMQP_STATUS_OK) {
            fprintf(stderr, "[RPC][init] login failure at entry %ld \n", idx);
            if(item->conn) {
                amqp_connection_close(item->conn, AMQP_REPLY_SUCCESS);
            }
            apprpc_conn_deinit__per_item(item);
            continue;
        }
        apprpc_ensure_send_queue(item);
    } // end of loop
    return (void *)ctx;
} // end of app_rpc_conn_init


void app_rpc_conn_deinit(void *ctx) {
    if(!ctx) { return; }
    struct arpc_ctx_list_t *_ctx = (struct arpc_ctx_list_t *) ctx;
    size_t idx = 0;
    for(idx = 0; idx < _ctx->size; idx++) {
        struct arpc_ctx_t *item = &_ctx->entries[idx];
        if(item->conn) {
            amqp_connection_close(item->conn, AMQP_REPLY_SUCCESS);
        }
        apprpc_conn_deinit__per_item(item);
    }
    free(_ctx);
} // end of app_rpc_conn_deinit


static struct arpc_ctx_t *apprpc_msgq_cfg_lookup(struct arpc_ctx_list_t *ctx, const char *alias)
{
    struct arpc_ctx_t *chosen = NULL;
    size_t idx = 0;
    for(idx = 0; ctx && idx < ctx->size; idx++) {
        struct arpc_ctx_t *item = &ctx->entries[idx];
        if(strcmp(item->ref_cfg->alias, alias) == 0) {
            chosen = item;
            break;
        }
    }
    return chosen;
} // end of apprpc_msgq_cfg_lookup

static arpc_cfg_bind_t *apprpc_msgq_bind_cfg_lookup(arpc_cfg_t *cfg, const char *routing_key)
{
    arpc_cfg_bind_t *chosen = NULL;
    size_t idx = 0;
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *item = &cfg->bindings.entries[idx];
        if(strcmp(item->routing_key, routing_key) == 0) {
            chosen = item;
            break;
        }
    }
    return chosen;
} // end of apprpc_msgq_bind_cfg_lookup


#define ARPC_MSGQ__ENSURE_RPC_REPLY_PARAM(app_status, reply_cfg, field, args, out, out_max_sz) \
{ \
    arpc_replyq_render_fn  fn = (reply_cfg)->field.render_fn; \
    const char *pattern = (reply_cfg)->field.name_pattern; \
    if(fn) { \
        app_status = fn(pattern, args, out, out_max_sz); \
    } else { \
        size_t exp_name_sz = strlen(pattern); \
        if(exp_name_sz < out_max_sz) { \
            memcpy(out, pattern, exp_name_sz); \
        } else { \
            app_status = APPRPC_RESP_MEMORY_ERROR; \
        } \
    } \
    if(app_status != APPRPC_RESP_OK) { \
        goto done; \
    } \
}

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *args)
{
    ARPC_STATUS_CODE  app_status = APPRPC_RESP_OK;
    if(!args || !args->job_id.bytes || (args->job_id.len == 0) || !args->msg_body.bytes
            || !args->routing_key || !args->conn || !args->alias) {
        app_status = APPRPC_RESP_ARG_ERROR;
        goto done;
    }
#define  RPC_MAX_REPLY_QNAME_SZ     128
    struct arpc_ctx_list_t *ctx = (struct arpc_ctx_list_t *) args->conn;
    struct arpc_ctx_t *mq_cfg = apprpc_msgq_cfg_lookup(ctx, args->alias);
    if(!mq_cfg || !mq_cfg->conn || !mq_cfg->sock) {
        app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR ;
        goto done;
    }
    arpc_cfg_bind_t *bind_cfg = apprpc_msgq_bind_cfg_lookup(mq_cfg->ref_cfg, args->routing_key);
    if(!bind_cfg) {
        app_status = APPRPC_RESP_ARG_ERROR;
        goto done;
    }
    // To create a responsive application, message broker has to return unroutable message
    // whenever the given routing key goes wrong.
    amqp_boolean_t mandatory = 1;
    // Should be OK if queue consumer does NOT immediately receive the published message
    amqp_boolean_t immediate = 0;
    args->_timestamp = (uint64_t) time(NULL); 
    char reply_req_queue[RPC_MAX_REPLY_QNAME_SZ] = {0};
    ARPC_MSGQ__ENSURE_RPC_REPLY_PARAM(app_status, &bind_cfg->reply, queue, args,
            &reply_req_queue[0], RPC_MAX_REPLY_QNAME_SZ);
    // job_id here should be the same as correlation ID within the message
    ARPC_MSGQ__ENSURE_RPC_REPLY_PARAM(app_status, &bind_cfg->reply, correlation_id, args,
             args->job_id.bytes, args->job_id.len);
    app_status = apprpc_ensure_reply_queue(mq_cfg->conn, &bind_cfg->reply, &reply_req_queue[0]);
    if(app_status != APPRPC_RESP_OK) {
        goto done;
    }
    amqp_basic_properties_t properties = {
            ._flags = AMQP_BASIC_CONTENT_TYPE_FLAG | AMQP_BASIC_DELIVERY_MODE_FLAG | AMQP_BASIC_CORRELATION_ID_FLAG
                | AMQP_BASIC_REPLY_TO_FLAG | AMQP_BASIC_TIMESTAMP_FLAG,
            .content_type = amqp_cstring_bytes("application/json"),  .reply_to = amqp_cstring_bytes(&reply_req_queue[0]),
            .correlation_id = {.bytes=args->job_id.bytes, .len=args->job_id.len},  .timestamp = args->_timestamp,
            .delivery_mode = 0x2, // defined in AMQP 0.9.1 without clear explanation
        };
    uint8_t is_done = 0, is_reconnected = 0;
    do { // publish message , if receiving heartbeat timeout , than login and open channel again
        // TODO: figure out how to use non-blocking API functions provided by librabbitmq.
        // Currently blocking API is used, there is only one channel to use for each
        // RabbitMQ connection.
        amqp_status_enum mq_status = amqp_basic_publish( mq_cfg->conn, APP_AMQP_CHANNEL_DEFAULT_ID,
                amqp_cstring_bytes(bind_cfg->exchange_name),  amqp_cstring_bytes(bind_cfg->routing_key),
                mandatory, immediate, (amqp_basic_properties_t const *)&properties,
                amqp_cstring_bytes(args->msg_body.bytes) );
        switch(mq_status) {
            case AMQP_STATUS_OK:
                app_status = APPRPC_RESP_ACCEPTED;
                is_done = 1;
                break;
            case AMQP_STATUS_TIMEOUT:
            case AMQP_STATUS_HEARTBEAT_TIMEOUT:
            case AMQP_STATUS_SOCKET_ERROR:
                if(is_reconnected) { // prevent endless loop at here
                    app_status = APPRPC_RESP_MSGQ_REMOTE_UNKNOWN_ERROR;
                    is_done = 1;
                    break;
                } else { // close connection without destroying it, then forward to next case ...
                    amqp_connection_close(mq_cfg->conn, AMQP_REPLY_SUCCESS);
                }
            case AMQP_STATUS_CONNECTION_CLOSED: // try connecting again
                if(apprpc_msgq_conn_init(mq_cfg) == AMQP_STATUS_OK) {
                    is_reconnected = 1;
                } else {
                    app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR ;
                    is_done = 1;
                }
                break;
            default:
                app_status = APPRPC_RESP_MSGQ_PUBLISH_ERROR;
                is_done = 1;
                break;
        } // end of switch statement
    } while(!is_done);
#undef   RPC_MAX_REPLY_QNAME_SZ
done:
    return app_status;
} // end of app_rpc_start


ARPC_STATUS_CODE app_rpc_get_reply(arpc_exe_arg_t *args)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_ACCEPTED;
    return status;
} // end of app_rpc_get_reply

