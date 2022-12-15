#include <uv.h>

#include "app_cfg.h"
#include "rpc/core.h"
#include "rpc/reply.h"
#include "storage/cfg_parser.h"
#include "storage/localfs.h"

#define   TIMER_DONT_REPEAT  0

typedef struct {
    arpc_reply_cfg_t  super;
    uv_timer_t   timer;
} _arpc_reply_ctx_t;


static void _apprpc_reply_deinit_timer_closed_cb (uv_handle_t* handle)
{
    _arpc_reply_ctx_t  *_rpc_ctx = handle->data;
    free(_rpc_ctx);
}

static void  _apprpc_reply_deinit_start (_arpc_reply_ctx_t  *_rpc_ctx)
{
    if(!_rpc_ctx)
        return;
    assert(!uv_is_closing((const uv_handle_t *) &_rpc_ctx->timer));
    uv_close((uv_handle_t *)&_rpc_ctx->timer, _apprpc_reply_deinit_timer_closed_cb);
}

void  apprpc_reply_deinit_start (void *ctx)
{ _apprpc_reply_deinit_start((_arpc_reply_ctx_t *)ctx); }


static  void  _apprpc_replyq_identify_corr_id (arpc_cfg_t *cfg, arpc_exe_arg_t *arg)
{
    if(arg->job_id.len == 0 || !arg->job_id.bytes)
        return; // discard due to lack of job ID
    // NOTE, in this app, the prefix of name pattern of a reply queue has to be identifiable by each user
    // the prefix of name pattern of correlation ID has to be identifiable by each RPC function
#define  FILTER_CODE(s1, _pattern) { \
    char *ptr = NULL; size_t prefix_sz = 0; int ret = 0; \
    ptr = strchr(_pattern, '%'); \
    prefix_sz = (ptr) ? ((size_t)ptr - (size_t)_pattern): strlen(_pattern); \
    ret = memcmp(s1, _pattern, prefix_sz); \
    if(ret != 0) \
        continue; \
}
    int idx = 0;
    json_t  *collected = arg->usr_data;
    for(idx = 0; idx < cfg->bindings.size; idx++) {
        arpc_cfg_bind_t  *bind_cfg = & cfg->bindings.entries[idx];
        const char *q_name_patt  = bind_cfg->reply.queue.name_pattern;
        const char *corr_id_patt = bind_cfg->reply.correlation_id.name_pattern;
        FILTER_CODE(arg->routing_key, q_name_patt)
        FILTER_CODE(arg->job_id.bytes, corr_id_patt)
        json_t  *reply_msgs = json_object_get(collected, corr_id_patt);
        if(!reply_msgs) {
            reply_msgs = json_array();
            json_object_set_new(collected, corr_id_patt, reply_msgs);
        }
        json_t *_packed = json_object(), *corr_id_item = json_object(), *msg_item = json_object();
        json_object_set_new(corr_id_item, "size", json_integer(arg->job_id.len));
        json_object_set_new(corr_id_item, "data", json_stringn(arg->job_id.bytes, arg->job_id.len));
        json_object_set_new(msg_item, "size", json_integer(arg->msg_body.len));
        json_object_set_new(msg_item, "data", json_stringn(arg->msg_body.bytes, arg->msg_body.len));
        json_object_set_new(_packed, "corr_id", corr_id_item);
        json_object_set_new(_packed, "msg", msg_item);
        json_object_set_new(_packed, "timestamp", json_integer(arg->_timestamp));
        json_array_append_new(reply_msgs, _packed);
        break;
    } // end of loop
    if(idx == cfg->bindings.size) {
        fprintf(stderr, "[rpc][reply] line:%d, discard reply message, queue:%s, corr_id:%s \r\n",
                __LINE__, arg->routing_key, arg->job_id.bytes);
    }
#undef   FILTER_CODE
} // end of  _apprpc_replyq_identify_corr_id


static void  _apprpc_reply_update_timeout_cb (uv_timer_t *handle)
{
    _arpc_reply_ctx_t  *_rpc_ctx = handle->data;
    arpc_reply_cfg_t   *cfg = &_rpc_ctx->super;
    uint8_t  _continue = 0;
    json_t  *info = json_object();
    if(!json_object_get(info, "usr_id")) // to build name of  RPC reply queue
        json_object_set_new(info, "usr_id", json_integer(cfg->usr_id));
    arpc_exe_arg_t  arg = {.alias="app_mqbroker_1", .usr_data=(void *)info,
        .conn=cfg->conn,  .flags={.replyq_nonexist=0} };
    ARPC_STATUS_CODE  arpc_res =  cfg->get_reply_fn(&arg, cfg->max_num_msgs_fetched,
            _apprpc_replyq_identify_corr_id);
    if(arpc_res == APPRPC_RESP_OK) {
        _continue = cfg->on_update(cfg, info, arpc_res);
    } else {
        fprintf(stderr, "[rpc][reply] line:%d, error (%d) when fetching RPC reply \r\n", __LINE__, arpc_res);
        cfg->flags.replyq_nonexist = arg.flags.replyq_nonexist;
        cfg->on_error(cfg, arpc_res);
    }
    json_decref(info);
    if(!_continue)
        _apprpc_reply_deinit_start(_rpc_ctx);
} // end of  _apprpc_reply_update_timeout_cb


void * apprpc_recv_reply_start (arpc_reply_cfg_t *cfg)
{
    _arpc_reply_ctx_t  *_rpc_ctx = calloc(1, sizeof(_arpc_reply_ctx_t));
    _rpc_ctx->super = *cfg;
    _rpc_ctx->timer.data = _rpc_ctx;
    int ret = uv_timer_init(_rpc_ctx->super.loop, &_rpc_ctx->timer);
    if(ret == 0) {
        _rpc_ctx =  apprpc_recv_reply_restart (_rpc_ctx);
    } else {
        _apprpc_reply_deinit_timer_closed_cb ((uv_handle_t *)&_rpc_ctx->timer);
        _rpc_ctx = NULL;
    }
    return  (void *)_rpc_ctx;
} // end of  apprpc_recv_reply_start


void * apprpc_recv_reply_restart (void *in_ctx)
{
    _arpc_reply_ctx_t *_rpc_ctx = (_arpc_reply_ctx_t *)in_ctx;
    int ret = uv_timer_start(&_rpc_ctx->timer, _apprpc_reply_update_timeout_cb,
         _rpc_ctx->super.timeout_ms , TIMER_DONT_REPEAT );
    if(ret != 0) {
        _apprpc_reply_deinit_start(_rpc_ctx);
        in_ctx = NULL;
        fprintf(stderr,"[rpc][reply] line:%d, failed to restart: %d \n", __LINE__, ret);
    }
    return in_ctx;
}


ARPC_STATUS_CODE  app_rpc__pycelery_extract_replies(json_t *in_msgs, json_t **out)
{
    json_t  *_packed = NULL;
    ARPC_STATUS_CODE  result = APPRPC_RESP_OK;
    int idx = 0;
    // Python celery consumer sends 2 messages back, first one means the consumer started handling
    //  the request, the second one indicates either error or completion 
    json_array_foreach(in_msgs, idx, _packed) {
        json_t *msg_item = json_object_get(_packed, "msg");
        const char *msg = json_string_value(json_object_get(msg_item, "data"));
        size_t  msg_sz  = json_integer_value(json_object_get(msg_item, "size"));
        json_t *_reply = json_loadb(msg, msg_sz, JSON_REJECT_DUPLICATES, NULL);
        if(_reply) {
            const char *rpc_status  = json_string_value(json_object_get(_reply, "status"));
            if(rpc_status && !strncmp(rpc_status, "SUCCESS", 7)) {
                json_t  *rpc_result  = json_object_get(_reply, "result");
                if(rpc_result != NULL) {
                    json_incref(rpc_result);
                    *out = rpc_result;
                } else {
                    fprintf(stderr, "[rpc][reply] line:%d, invalid result \n", __LINE__);
                    result = APPRPC_RESP_ARG_ERROR;
                }
            } else if(rpc_status && !strncmp(rpc_status, "STARTED", 7)) {
                // pass, discard the result
            } else {
                fprintf(stderr, "[rpc][reply] line:%d, unknown state:%s \n", __LINE__, rpc_status);
                result = APPRPC_RESP_ARG_ERROR;
            }
            json_decref(_reply);
        } else {
            json_t *corr_id_item = json_object_get(_packed, "corr_id");
            const char * corr_id = json_string_value(json_object_get(corr_id_item, "data"));
            fprintf(stderr, "[rpc][reply] line:%d, corr_id:%s, msg:%s \n", __LINE__, corr_id, msg);
            result = APPRPC_RESP_ARG_ERROR;
        }
        if((result != APPRPC_RESP_OK) || *out)
            break;
    } // end of loop
    return  result;
} // end of  app_rpc__pycelery_extract_replies
