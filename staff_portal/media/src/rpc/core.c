#include "rpc/core.h"

// Note: limitation of librabbitmq:
//       https://github.com/alanxz/rabbitmq-c#writing-applications-using-librabbitmq
//
// TODO
// (1) figure out how to make non-blocking call to all functions in librabbitmq
// (2) choose distinct channel ID for each thread, within a single TCP connection (to AMQP broker)
#define APP_AMQP_CHANNEL_DEFAULT_ID  1


static  ARPC_STATUS_CODE apprpc__translate_status_from_lowlvl_lib(amqp_rpc_reply_t *reply)
{
    ARPC_STATUS_CODE  app_status = APPRPC_RESP_OK;
    switch(reply->reply_type) {
        case AMQP_RESPONSE_NORMAL:
            break;
        case AMQP_RESPONSE_SERVER_EXCEPTION:
            switch (reply->reply.id) {
                case AMQP_CHANNEL_CLOSE_METHOD: // see detail usage of API  amqp_get_rpc_reply(...) in rabbitmq-c
                    {
                        amqp_channel_close_t *m = (amqp_channel_close_t *)reply->reply.decoded;
#if  0
                        fprintf(stderr, "[RPC][core] line:%d, reason: server channel error %u, class id:0x%x, method id:0x%x, message: %.*s \n",
                                __LINE__, m->reply_code, m->class_id, m->method_id, (int)m->reply_text.len,
                                (char *)m->reply_text.bytes);
#endif
                        if(m->reply_code < AMQP_CONTENT_TOO_LARGE) {
                            // pass
                        } else if(m->reply_code >= AMQP_CONTENT_TOO_LARGE && m->reply_code < AMQP_FRAME_ERROR) {
                            // the codes within the range means mostly config error
                            app_status = APPRPC_RESP_MSGQ_OPERATION_ERROR;
                        } else {
                            app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
                        }
                    }
                    break;
                case AMQP_CONNECTION_CLOSE_METHOD: // half-closed on remote server ?
                    app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
                    break;
                case AMQP_EXCHANGE_DECLARE_METHOD: 
                case AMQP_QUEUE_DECLARE_METHOD:
                case AMQP_BASIC_PUBLISH_METHOD:
                case AMQP_BASIC_CONSUME_METHOD:
                case AMQP_BASIC_CANCEL_METHOD:
                    app_status = APPRPC_RESP_MSGQ_OPERATION_ERROR;
                    break;
                default:
                    app_status = APPRPC_RESP_MSGQ_REMOTE_UNCLASSIFIED_ERROR;
                    break;
            }
            break;
        case AMQP_RESPONSE_LIBRARY_EXCEPTION:
            switch(reply->library_error) {
                case AMQP_STATUS_TIMER_FAILURE:
                    app_status = APPRPC_RESP_OS_ERROR;
                    break;
                case AMQP_STATUS_TIMEOUT:
                    app_status = APPRPC_RESP_MSGQ_OPERATION_TIMEOUT;
                    break;
                case AMQP_STATUS_HEARTBEAT_TIMEOUT:
                case AMQP_STATUS_SOCKET_ERROR:
                    app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
                    break;
                case AMQP_STATUS_SOCKET_CLOSED:
                case AMQP_STATUS_CONNECTION_CLOSED: // try connecting again
                    app_status = APPRPC_RESP_MSGQ_CONNECTION_CLOSED;
                    break;
                case AMQP_STATUS_INVALID_PARAMETER:
                    app_status = APPRPC_RESP_ARG_ERROR;
                    break;
                case AMQP_STATUS_BAD_AMQP_DATA:
                case AMQP_STATUS_TABLE_TOO_BIG:
                    app_status = APPRPC_RESP_MEMORY_ERROR;
                    break;
                default:
                    app_status = APPRPC_RESP_MSGQ_LOWLEVEL_LIB_ERROR;
                    break;
            }
            break;
        default:
            app_status = APPRPC_RESP_MSGQ_UNKNOWN_ERROR;
            break;
    } // end of error type check
    return app_status;
} // end of apprpc__translate_status_from_lowlvl_lib


static  amqp_status_enum  apprpc_msgq_conn_auth(struct arpc_ctx_t *item)
{ // note all the low-level AMQP functions are NOT thread safe
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
    amqp_channel_open_ok_t *chn_res = amqp_channel_open(item->conn, item->curr_channel_id);
    if(!chn_res || !chn_res->channel_id.bytes) {
        fprintf(stderr, "[RPC] line:%d, failed to open default channel %s:%hu \n",
                __LINE__, cfg->credential.host, cfg->credential.port );
        status = AMQP_STATUS_NO_MEMORY;
    }
done:
    return status;
} // end of apprpc_msgq_conn_auth

static void apprpc_declare_q_report_error(amqp_rpc_reply_t *reply, char *q_name)
{ // TODO, deprecated, will be replaced by apprpc__translate_status_from_lowlvl_lib
    fprintf(stderr, "[RPC] fail to declare a queue : %s ", q_name);
    if(reply->reply_type == AMQP_RESPONSE_SERVER_EXCEPTION) {
        // TODO, separate channel error and connection error
        amqp_channel_close_t *m = (amqp_channel_close_t *)reply->reply.decoded;
        fprintf(stderr, ", reason: server channel error %hu, message: %.*s ",
             m->reply_code, (int)m->reply_text.len, (char *)m->reply_text.bytes);
    } else if(reply->reply_type == AMQP_RESPONSE_LIBRARY_EXCEPTION) {
        const char *errmsg = amqp_error_string2(reply->library_error);
        fprintf(stderr, ", reason: library error, %s ", errmsg);
    }
    fprintf(stderr, "\n");
} // end of apprpc_declare_q_report_error

static void apprpc_ensure_send_queue(struct arpc_ctx_t *item)
{
    size_t idx = 0;
    arpc_cfg_t *cfg = item->ref_cfg;
    amqp_rpc_reply_t _reply = {0};
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *bind_cfg = &cfg->bindings.entries[idx];
        if(bind_cfg->skip_declare) {
            fprintf(stderr, "[RPC][core] this app should NOT declare the queue:%s"
                    " where the other app will consume the message from \n",   bind_cfg->q_name);
            continue;
        }
        // AMQP broker does NOT allow  unsigned number as argument when declaring a queue ? find out the source(TODO)
        amqp_table_entry_t  q_arg_n_elms = {.key = amqp_cstring_bytes("x-max-length"),
                .value = {.kind = AMQP_FIELD_KIND_I32, .value = {.i32 = bind_cfg->max_msgs_pending}}};
        amqp_table_t  q_arg_table = {.num_entries=1, .entries=&q_arg_n_elms};
        amqp_queue_declare( item->conn,  item->curr_channel_id,
                amqp_cstring_bytes(bind_cfg->q_name), (amqp_boolean_t)bind_cfg->flags.passive,
                (amqp_boolean_t)bind_cfg->flags.durable, (amqp_boolean_t)bind_cfg->flags.exclusive,
                (amqp_boolean_t)bind_cfg->flags.auto_delete, q_arg_table );
        _reply = amqp_get_rpc_reply(item->conn);
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            apprpc_declare_q_report_error(&_reply, bind_cfg->q_name);
            continue;
        }
        amqp_queue_bind( item->conn, item->curr_channel_id,
                amqp_cstring_bytes(bind_cfg->q_name),  amqp_cstring_bytes(bind_cfg->exchange_name),
                amqp_cstring_bytes(bind_cfg->routing_key),  amqp_empty_table);
        _reply = amqp_get_rpc_reply(item->conn);
        if(_reply.reply_type != AMQP_RESPONSE_NORMAL) {
            fprintf(stderr, "[RPC] fail to bind the routing key (%s) with the queue (%s) \n",
                    bind_cfg->routing_key, bind_cfg->q_name);
        }
    } // end of loop
} // end of apprpc_ensure_send_queue


static ARPC_STATUS_CODE  apprpc_ensure_reply_queue(struct arpc_ctx_t *_ctx,
        arpc_cfg_bind_reply_t *reply_cfg, char *q_name) {
    // TODO:
    // * Currently, reply queue is always bound with default exchange (specify empty
    //   exchange name) , in case some AMQP brokers sends return value back to the reply
    //   queue ONLY using default exchange. ( switch to non-default exchange)
    // * add  `x-message-ttl` attribute to all reply queues, the base time unit is milliseconds
    amqp_queue_declare( _ctx->conn, _ctx->curr_channel_id,
            amqp_cstring_bytes(q_name), (amqp_boolean_t)reply_cfg->flags.passive,
            (amqp_boolean_t)reply_cfg->flags.durable, (amqp_boolean_t)reply_cfg->flags.exclusive,
            (amqp_boolean_t)reply_cfg->flags.auto_delete, amqp_empty_table );
    amqp_rpc_reply_t _reply = amqp_get_rpc_reply(_ctx->conn);
    if(_reply.reply_type != AMQP_RESPONSE_NORMAL)
        apprpc_declare_q_report_error(&_reply, q_name);
    return  apprpc__translate_status_from_lowlvl_lib(&_reply);
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
    if(!cfgs || nitem == 0)
        return NULL;
    size_t ctx_sz = sizeof(struct arpc_ctx_list_t) + nitem * sizeof(struct arpc_ctx_t);
    struct arpc_ctx_list_t *ctx = (struct arpc_ctx_list_t *) calloc(1, ctx_sz);
    size_t idx = 0;
    ctx->size = nitem;
    ctx->entries = (struct arpc_ctx_t *) ((char *)ctx + sizeof(struct arpc_ctx_list_t));
    for(idx = 0; idx < nitem; idx++) {
        struct arpc_ctx_t *item = &ctx->entries[idx];
        item->ref_cfg = &cfgs[idx];
        ARPC_STATUS_CODE  res = app_rpc_open_connection((void *)item);
        if(res != APPRPC_RESP_OK)
            continue;
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
        if(item->conn)
            amqp_connection_close(item->conn, AMQP_REPLY_SUCCESS);
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

static arpc_cfg_bind_t *apprpc_msgq_bind_cfg_lookup(arpc_cfg_t *cfg, amqp_bytes_t *route_key)
{
    arpc_cfg_bind_t *chosen = NULL;
    size_t idx = 0;
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *item = &cfg->bindings.entries[idx];
        if(strncmp(item->routing_key, (char *)route_key->bytes, route_key->len) == 0)
        {
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
    struct arpc_ctx_list_t *ctxes = (struct arpc_ctx_list_t *) args->conn;
    struct arpc_ctx_t *mq_ctx = apprpc_msgq_cfg_lookup(ctxes, args->alias);
    if(!mq_ctx || !mq_ctx->conn || !mq_ctx->sock) {
        app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR ;
        goto done;
    }
    amqp_bytes_t routekey = amqp_cstring_bytes(args->routing_key);
    arpc_cfg_bind_t *bind_cfg = apprpc_msgq_bind_cfg_lookup(mq_ctx->ref_cfg, &routekey);
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
    if(app_status != APPRPC_RESP_OK)
        goto done;
    // job_id here should be the same as correlation ID within the message
    ARPC_MSGQ__ENSURE_RPC_REPLY_PARAM(app_status, &bind_cfg->reply, correlation_id, args,
             args->job_id.bytes, args->job_id.len);
    if(app_status != APPRPC_RESP_OK)
        goto done;
    size_t _job_id_fit_sz = strlen(args->job_id.bytes);
    assert(_job_id_fit_sz < args->job_id.len);
    app_status = apprpc_ensure_reply_queue(mq_ctx, &bind_cfg->reply, &reply_req_queue[0]);
    switch(app_status) {
        case APPRPC_RESP_OK:
            break;
        case APPRPC_RESP_MSGQ_CONNECTION_ERROR: // try reconnecting
            app_rpc_close_connection((void *)mq_ctx);
        case APPRPC_RESP_MSGQ_CONNECTION_CLOSED:
            app_status = app_rpc_open_connection((void *)mq_ctx);
            if(app_status == APPRPC_RESP_OK)
                app_status = apprpc_ensure_reply_queue(mq_ctx, &bind_cfg->reply, &reply_req_queue[0]);
            if(app_status == APPRPC_RESP_OK) {
                break;
            } else {
                goto done;
            }
        default:
            goto done;
    }
    amqp_basic_properties_t properties = {
            ._flags = AMQP_BASIC_CONTENT_TYPE_FLAG | AMQP_BASIC_DELIVERY_MODE_FLAG | AMQP_BASIC_CORRELATION_ID_FLAG
                | AMQP_BASIC_REPLY_TO_FLAG | AMQP_BASIC_TIMESTAMP_FLAG,
            .content_type = amqp_cstring_bytes("application/json"),  .reply_to = amqp_cstring_bytes(&reply_req_queue[0]),
            .correlation_id = {.bytes=args->job_id.bytes, .len=_job_id_fit_sz},  .timestamp = args->_timestamp,
            .delivery_mode = 0x2, // defined in AMQP 0.9.1 without clear explanation
        };
    amqp_status_enum mq_status = amqp_basic_publish( mq_ctx->conn,  mq_ctx->curr_channel_id,
            amqp_cstring_bytes(bind_cfg->exchange_name),  routekey, mandatory, immediate,
            (amqp_basic_properties_t const *)&properties, amqp_cstring_bytes(args->msg_body.bytes) );
    // TODO: figure out how to use non-blocking API functions provided by librabbitmq.
    // Currently blocking API is used, there is only one channel to use for each
    // RabbitMQ connection.
    if(mq_status == AMQP_STATUS_OK) {
        app_status = APPRPC_RESP_ACCEPTED;
    } else {
        amqp_rpc_reply_t _reply = amqp_get_rpc_reply(mq_ctx->conn);
        app_status = apprpc__translate_status_from_lowlvl_lib(&_reply);
    }
#undef   RPC_MAX_REPLY_QNAME_SZ
done:
    return app_status;
} // end of app_rpc_start


static ARPC_STATUS_CODE apprpc_consumer_set_read_queues(struct arpc_ctx_t *_ctx)
{
    ARPC_STATUS_CODE res = APPRPC_RESP_OK;
    arpc_cfg_t *rpc_cfg = _ctx->ref_cfg;
    size_t idx = 0;
    amqp_boolean_t no_local = 0, exclusive = 0;
    amqp_boolean_t no_ack = 1; // automatically send ack back to broker
    for(idx = 0; idx < rpc_cfg->bindings.size; idx++) {
        arpc_cfg_bind_t *bindcfg = &rpc_cfg->bindings.entries[idx];
        if(bindcfg->skip_declare) {
            fprintf(stderr, "[RPC][core] this app should NOT consume message from the queue:%s \n",
                    bindcfg->q_name);
            continue;
        }
        amqp_bytes_t queue = amqp_cstring_bytes(bindcfg->q_name);
        amqp_basic_consume( _ctx->conn, _ctx->curr_channel_id, queue, amqp_empty_bytes,
                no_local, no_ack, exclusive, amqp_empty_table) ;
        amqp_rpc_reply_t  reply = amqp_get_rpc_reply(_ctx->conn);
        res = apprpc__translate_status_from_lowlvl_lib(&reply);
        if(res != APPRPC_RESP_OK) { break; }
    }
    return res;
} // end of apprpc_consumer_set_read_queues


static ARPC_STATUS_CODE _apprpc_consume_handler__send2reply_q(arpc_receipt_t *r, char *out, size_t out_sz)
{
    ARPC_STATUS_CODE app_status = APPRPC_RESP_MSGQ_UNKNOWN_ERROR;
    struct arpc_ctx_t * _ctx = (struct arpc_ctx_t *)r->ctx;
    amqp_envelope_t  *evp = (amqp_envelope_t *)r->_msg_obj;
    // Note this function doesn't ensure existence of the reply queue, it reports error
    // if reply queue is absent.
    amqp_basic_properties_t properties = {
        ._flags = AMQP_BASIC_CONTENT_TYPE_FLAG | AMQP_BASIC_DELIVERY_MODE_FLAG |
            AMQP_BASIC_CORRELATION_ID_FLAG | AMQP_BASIC_TIMESTAMP_FLAG,
        .content_type = amqp_cstring_bytes("application/json"), .timestamp = (uint64_t)time(NULL),
        .correlation_id = {.bytes=r->job_id.bytes, .len=r->job_id.len}, .delivery_mode = 0x2, // magic number
    }; // TODO, the value in `r->_timestamp` should be reserved for profiling purpose
    // send return value to reply queue
    amqp_bytes_t body = {.len=out_sz , .bytes=out};
    amqp_boolean_t mandatory = 1;
    amqp_boolean_t immediate = 0;
    amqp_status_enum  mq_status = AMQP_STATUS_OK;
    // it has to be anon-exchange for publishing message to RPC reply queue, such exchange
    // in AMQP broker maps routing key directly to the queue with exact name.
#define AMQP_ANON_EXCHANGE amqp_empty_bytes
#define RUN_PUBLISH_CMD \
    amqp_basic_publish( _ctx->conn, _ctx->curr_channel_id,  AMQP_ANON_EXCHANGE, \
            evp->message.properties.reply_to, mandatory, immediate, \
            (amqp_basic_properties_t const *)&properties, body );
    mq_status = RUN_PUBLISH_CMD;
    if(mq_status == AMQP_STATUS_OK) {
        app_status = APPRPC_RESP_OK;
    } else {
        amqp_rpc_reply_t _reply = amqp_get_rpc_reply(_ctx->conn);
        app_status = apprpc__translate_status_from_lowlvl_lib(&_reply);
        switch(app_status) {
            case APPRPC_RESP_MSGQ_CONNECTION_ERROR: // try reconnecting
                app_rpc_close_connection((void *)_ctx);
            case APPRPC_RESP_MSGQ_CONNECTION_CLOSED:
                app_status = app_rpc_open_connection((void *)_ctx);
                if(app_status == APPRPC_RESP_OK) {
                    mq_status = RUN_PUBLISH_CMD;
                    if(mq_status != AMQP_STATUS_OK) {
                        _reply = amqp_get_rpc_reply(_ctx->conn);
                        app_status = apprpc__translate_status_from_lowlvl_lib(&_reply);
                        fprintf(stderr, "[RPC][consumer] failed to send message to reply queue\n");
                    } // TODO, logging more error detail
                } else {
                    fprintf(stderr, "[RPC][consumer] failed to reconnect when sending to reply queue\n");
                    app_status = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
                }
                break;
            case APPRPC_RESP_OK:
            default:
                fprintf(stderr, "[RPC][consumer] unclassified error (%d) when returning value to reply queue\n", app_status);
                break;
        } // error handling if connection closed unexpectedly
    }
    return app_status;
#undef RUN_PUBLISH_CMD
#undef AMQP_ANON_EXCHANGE
} // end of _apprpc_consume_handler__send2reply_q

static ARPC_STATUS_CODE apprpc_consume_handler_finalize(arpc_receipt_t *r, char *out, size_t out_sz)
{
    amqp_envelope_t  *evp = (amqp_envelope_t *)r->_msg_obj;
    ARPC_STATUS_CODE  status = _apprpc_consume_handler__send2reply_q(r, out, out_sz);
    amqp_destroy_envelope(evp);
    free(evp);
    free(r);
    return status;
} // end of apprpc_consume_handler_finalize


ARPC_STATUS_CODE  app_rpc_consume_message(void *ctx, void *loop)
{ // consume one message at a time in non-blocking manner
    ARPC_STATUS_CODE res = APPRPC_RESP_OK;
    struct arpc_ctx_t *_ctx = (struct arpc_ctx_t *)ctx;
    struct timeval  timeout = {0}; // immediately return if all the queues are empty.
    amqp_envelope_t envelope = {0};
    if(!_ctx || !_ctx->conn || !_ctx->ref_cfg || !loop)
        return APPRPC_RESP_ARG_ERROR;
    if(!_ctx->consumer_setup_done) {
        res = apprpc_consumer_set_read_queues(_ctx);
        if(res != APPRPC_RESP_OK) { goto done; }
        _ctx->consumer_setup_done = 1;
    }
    amqp_maybe_release_buffers(_ctx->conn);    
    amqp_rpc_reply_t  reply = amqp_consume_message(_ctx->conn, &envelope, (const struct timeval *)&timeout, 0);
    res = apprpc__translate_status_from_lowlvl_lib(&reply);
    if(res != APPRPC_RESP_OK) {
        goto done;
    } // this is non-blocking function, don't treat operation timeout as error.
    arpc_cfg_bind_t *bind_cfg = apprpc_msgq_bind_cfg_lookup(_ctx->ref_cfg, &envelope.routing_key);
    if(!bind_cfg) {
        fprintf(stderr, "[RPC consumer] unknown routing key (%s) within the RPC message\n",
                (char *)envelope.routing_key.bytes);
        res = APPRPC_RESP_MSGQ_OPERATION_ERROR;
        goto done;
    }
    arpc_task_handler_fn  entry_fn = bind_cfg->reply.task_handler;
    if(entry_fn) {
        arpc_receipt_t *r = malloc(sizeof(arpc_receipt_t));
        amqp_bytes_t *corr_id = &envelope.message.properties.correlation_id;
        amqp_bytes_t *body    = &envelope.message.body;
        *r = (arpc_receipt_t) {.ctx=ctx, .loop=loop, .return_fn=apprpc_consume_handler_finalize,
            .send_fn=_apprpc_consume_handler__send2reply_q,
            ._msg_obj=malloc(sizeof(amqp_envelope_t)), .routing_key=envelope.routing_key.bytes,
            .job_id={.len=corr_id->len, .bytes=corr_id->bytes}, ._timestamp=(uint64_t)time(NULL),
            .msg_body={.len=body->len, .bytes=body->bytes},
        };
        *(amqp_envelope_t *)r->_msg_obj = envelope;
        entry_fn(r); // user-defined handlers must invoke return function at the end of the long-running task
    } else {
        fprintf(stderr, "[RPC consumer] missing task handler (%s) that wasn't found at parsing phase \n",
                (char *)envelope.routing_key.bytes);
        // TODO, log error, received message not handled
        res = APPRPC_RESP_MSGQ_OPERATION_ERROR;
    }
done:
    if(res != APPRPC_RESP_OK)
        amqp_destroy_envelope(&envelope);
    return res;
} // end of app_rpc_consume_message


#define  MSGQ__RECONNECT_THEN_RUN_OPERATION(_mq_ctx, _res, _cmd) { \
    _cmd(_res); \
    switch(_res) { \
        case APPRPC_RESP_MSGQ_CONNECTION_ERROR: \
            app_rpc_close_connection((void *)_mq_ctx); \
        case APPRPC_RESP_MSGQ_CONNECTION_CLOSED: \
            _res = app_rpc_open_connection((void *)_mq_ctx); \
            if(_res == APPRPC_RESP_OK) \
                _cmd(_res); \
        case APPRPC_RESP_OK: \
        default: \
            break; \
    } \
}

ARPC_STATUS_CODE  app_rpc_fetch_replies (arpc_exe_arg_t *args, size_t max_nread, arpc_reply_corr_identify_fn  cb)
{
#define  RPC_MAX_REPLY_QNAME_SZ     128
    if(!args || !args->conn || !args->alias  || !cb || max_nread == 0)
        return APPRPC_RESP_ARG_ERROR;
    struct arpc_ctx_t  *mq_ctx = app_rpc_context_lookup(args->conn, args->alias);
    if(!mq_ctx || !mq_ctx->ref_cfg || !mq_ctx->ref_cfg->bindings.entries ||
            (mq_ctx->ref_cfg->bindings.size == 0))
        return APPRPC_RESP_ARG_ERROR;
    ARPC_STATUS_CODE  res = APPRPC_RESP_OK;
    amqp_boolean_t  no_local = 0, exclusive = 0, no_ack = 1; // automatically send ack back to broker
    amqp_rpc_reply_t  reply = {0};
    for(int idx = 0; (max_nread > 0) && (idx < mq_ctx->ref_cfg->bindings.size); idx++) 
    {
        arpc_cfg_bind_t  *bind_cfg = & mq_ctx->ref_cfg->bindings.entries[idx];
        char reply_q_name[RPC_MAX_REPLY_QNAME_SZ] = {0};
        ARPC_MSGQ__ENSURE_RPC_REPLY_PARAM(res, &bind_cfg->reply, queue, args,
              &reply_q_name[0], RPC_MAX_REPLY_QNAME_SZ);
        if(res != APPRPC_RESP_OK)
            continue;  // -------------
        amqp_bytes_t _queue   = amqp_cstring_bytes(&reply_q_name[0]);
        amqp_bytes_t _con_tag = _queue;
#define  CMD(_res) { \
    amqp_basic_consume(mq_ctx->conn, mq_ctx->curr_channel_id, _queue, _con_tag, \
            no_local, no_ack, exclusive, amqp_empty_table) ; \
    reply = amqp_get_rpc_reply(mq_ctx->conn); \
    _res = apprpc__translate_status_from_lowlvl_lib(&reply); \
}
        MSGQ__RECONNECT_THEN_RUN_OPERATION(mq_ctx, res, CMD);
#undef  CMD
        if(res == APPRPC_RESP_OK) {
            // pass
        } else if(res == APPRPC_RESP_MSGQ_OPERATION_ERROR || res == APPRPC_RESP_MSGQ_OPERATION_TIMEOUT) {
            fprintf(stderr, "[RPC] failed to consume reply queue (%s) \n", &reply_q_name[0]);
            uint8_t channel_closed = reply.reply_type == AMQP_RESPONSE_SERVER_EXCEPTION && 
                        reply.reply.id == AMQP_CHANNEL_CLOSE_METHOD;
            if(channel_closed) { // RPC reply queue for current user does not exist
                // TODO, (1) force to reconnect if channal number reaches its max limit,
                //  (2) security log, prevent DDoS attack
                reply = amqp_channel_close(mq_ctx->conn, mq_ctx->curr_channel_id++, AMQP_NOT_FOUND);
                amqp_channel_open_ok_t *chn_res = amqp_channel_open(mq_ctx->conn, mq_ctx->curr_channel_id);
                if(!chn_res || !chn_res->channel_id.bytes) {
                    fprintf(stderr, "[RPC] line:%d, failed to open channel:%hu \n", __LINE__,  mq_ctx->curr_channel_id);
                    res = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
                    break;
                }
            } // can perform basic-cancel ?
            args->flags.replyq_nonexist = 1;
            continue; 
        } else {
            fprintf(stderr, "[RPC] failed to reconnect to broker (%d) \n", res);
            break;
        }
        struct timeval   timeout = {0}; // immediately return if all the queues are empty.
        args->routing_key = &reply_q_name[0];
        do {
            amqp_envelope_t  envelope = {0};
            amqp_maybe_release_buffers(mq_ctx->conn);    
            reply = amqp_consume_message(mq_ctx->conn, &envelope, (const struct timeval *)&timeout, 0); // non-blocking
            res = apprpc__translate_status_from_lowlvl_lib(&reply);
            if(res == APPRPC_RESP_OK) {
                amqp_bytes_t *corr_id = &envelope.message.properties.correlation_id;
                amqp_bytes_t *msgbody = &envelope.message.body;
                args->job_id.len   = corr_id->len;
                args->job_id.bytes = corr_id->bytes;
                args->msg_body.len   = msgbody->len;
                args->msg_body.bytes = msgbody->bytes;
                args->_timestamp = (uint64_t) envelope.message.properties.timestamp;
                cb(mq_ctx->ref_cfg, args);
                max_nread--;
                amqp_destroy_envelope(&envelope);
            } // TODO, figure out the way of identifying empty queue
        } while ((res == APPRPC_RESP_OK) && (max_nread > 0));
        if (res == APPRPC_RESP_MSGQ_OPERATION_TIMEOUT)
            res = APPRPC_RESP_OK; // means empty reply queue
        amqp_basic_cancel(mq_ctx->conn, mq_ctx->curr_channel_id, _con_tag);
    } // end of msg queue setup iteration
    args->routing_key = NULL;
    args->job_id.len   = 0;
    args->job_id.bytes = NULL;
    args->msg_body.len   = 0;
    args->msg_body.bytes = NULL;
    return res;
#undef   RPC_MAX_REPLY_QNAME_SZ
}  // ned of app_rpc_fetch_replies


void app_rpc_task_send_reply (arpc_receipt_t *receipt, json_t *res_body, uint8_t _final)
{
    size_t  nrequired = json_dumpb((const json_t *)res_body, NULL, 0, 0); // + 1;
    char    body_raw[nrequired];
    size_t  nwrite = json_dumpb((const json_t *)res_body, &body_raw[0], nrequired, JSON_COMPACT);
    // body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    if(_final) {
        receipt->return_fn(receipt, &body_raw[0], nwrite);
    } else {
        receipt->send_fn(receipt, &body_raw[0], nwrite);
    }
} // end of app_rpc_task_send_reply


ARPC_STATUS_CODE app_rpc_close_connection(void *ctx)
{ // TODO, find better approach, without re-init connection object
    ARPC_STATUS_CODE res = APPRPC_RESP_OK;
    struct arpc_ctx_t *_ctx = (struct arpc_ctx_t *)ctx;
    if(_ctx && _ctx->conn && _ctx->sock) {
        amqp_connection_close(_ctx->conn, AMQP_REPLY_SUCCESS);
        apprpc_conn_deinit__per_item(_ctx); 
    } else {
        res = APPRPC_RESP_MEMORY_ERROR;
    }
    return res;
}

ARPC_STATUS_CODE app_rpc_open_connection(void *ctx)
{
    ARPC_STATUS_CODE res = APPRPC_RESP_OK;
    struct arpc_ctx_t *_ctx = (struct arpc_ctx_t *)ctx;
    if(!_ctx)
        return APPRPC_RESP_ARG_ERROR;
    if(_ctx->conn || _ctx->sock)
        return APPRPC_RESP_MEMORY_ERROR;
    _ctx->conn = amqp_new_connection();
    if(!_ctx->conn) {
        fprintf(stderr, "[RPC][init] memory allocation error on connection object\n");
        res = APPRPC_RESP_MEMORY_ERROR;
        goto done;
    }
    _ctx->sock = amqp_tcp_socket_new(_ctx->conn); // not thread-safe
    if(!_ctx->sock) {
        fprintf(stderr, "[RPC][init] memory allocation error on TCP socket\n");
        res = APPRPC_RESP_MEMORY_ERROR;
        goto done;
    } // socket will be deleted once connection is closed
    _ctx->curr_channel_id = APP_AMQP_CHANNEL_DEFAULT_ID;
    if(apprpc_msgq_conn_auth(_ctx) != AMQP_STATUS_OK) {
        res = APPRPC_RESP_MSGQ_CONNECTION_ERROR;
        goto done; 
    }
    _ctx->consumer_setup_done = 0;
done:
    if(res != APPRPC_RESP_OK) {
        if(_ctx->conn)
            amqp_connection_close(_ctx->conn, AMQP_REPLY_SUCCESS);
        apprpc_conn_deinit__per_item(_ctx); 
    }
    return res;
} // end of app_rpc_open_connection


void *app_rpc_context_lookup(void *ctxes, const char *alias)
{
    struct arpc_ctx_list_t *ctx_list = (struct arpc_ctx_list_t *)ctxes;
    return (void *)apprpc_msgq_cfg_lookup(ctx_list, alias);
}

int app_rpc_get_sockfd(void *ctx)
{
    int fd = -1;
    struct arpc_ctx_t *_ctx = (struct arpc_ctx_t *)ctx;
    if(_ctx && _ctx->conn)
        fd = amqp_get_sockfd(_ctx->conn);
    return fd;
}

arpc_cfg_t *app_rpc_get_config(void *ctx)
{
    arpc_cfg_t *cfg =  NULL;
    struct arpc_ctx_t *_ctx = (struct arpc_ctx_t *)ctx;
    if(_ctx && _ctx->ref_cfg)
        cfg = _ctx->ref_cfg;
    return cfg;
}
