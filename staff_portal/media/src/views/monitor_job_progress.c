#include "utils.h"
#include "views.h"
#include "storage/cfg_parser.h"
#include "storage/localfs.h"
#include "rpc/core.h"
#include "rpc/reply.h"

#define   NUM_USRARGS_ASA  (ASA_USRARG_INDEX__API_RPC_REPLY_DATA + 1)

typedef struct {
    h2o_req_t     *req;
    h2o_handler_t *hdlr;
    app_middleware_node_t  *node;
    json_t  *qparam;
    json_t *res_body;
    asa_op_base_cfg_t  *asaobj;
    void *rpcreply_ctx;
} api_usr_data_t;


static void  _api_monitor_progress__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *qparam, json_t *res_body)
{
    size_t  nrequired = json_dumpb((const json_t *)res_body, NULL, 0, 0) + 1;
    char    body_raw[nrequired] ;
    size_t  nwrite = json_dumpb((const json_t *)res_body, &body_raw[0], nrequired, JSON_COMPACT);
    body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    if(req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[monitor_job_progress] line:%d \n", __LINE__ );
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, &body_raw[0], strlen(&body_raw[0]));
    json_decref(qparam);
    json_decref(res_body);
    app_run_next_middleware(hdlr, req, node);
} // end of _api_monitor_progress__deinit_primitives


static void  _api_monitor_progress__deinit_asaobj (asa_op_base_cfg_t *asaobj)
{
    api_usr_data_t  *usrdata  = asaobj->cb_args.entries[ASA_USRARG_INDEX__APIUSRDATA];
    apprpc_reply_deinit_start(usrdata->rpcreply_ctx);
    _api_monitor_progress__deinit_primitives (usrdata->req, usrdata->hdlr, usrdata->node,
            usrdata->qparam, usrdata->res_body);
    free(asaobj);
    free(usrdata);
}


static void  _api_job_progressinfo_final_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ asaobj->deinit(asaobj); }

static void  _api_job_progress_fileopened_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    api_usr_data_t  *usrdata  = asaobj->cb_args.entries[ASA_USRARG_INDEX__APIUSRDATA];
    h2o_req_t  *req  = usrdata->req;
    json_t *qparam   = usrdata->qparam;
    json_t *res_body = usrdata->res_body;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_localfs_cfg_t * _asa_local = (asa_op_localfs_cfg_t *)asaobj;
        int fd = _asa_local->file.file;
        lseek(fd, 0, SEEK_SET);
        json_t *info =  json_loadfd(fd, JSON_REJECT_DUPLICATES, NULL);
        const char *req_job_id = json_string_value(json_object_get(qparam, "id"));
        json_t  *result_item = json_object_get(info, req_job_id);
        if(result_item) { // either progress or error detail
            req->res.status = 200;
            json_decref(res_body);
            json_incref(result_item);
            usrdata->res_body = result_item;
        } else {
            req->res.status = 404;
            json_object_set_new(res_body, "reason", json_string("job ID not found"));
        }
        json_decref(info);
    } else {
        req->res.status = 400;
        json_object_set_new(res_body, "reason", json_string("job queue not ready"));
    }
    asaobj->op.close.cb = _api_job_progressinfo_final_cb;
    result = asaobj->storage->ops.fn_close(asaobj);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[api][monitor_job_progress] line:%d, storage result:%d \n", __LINE__, result );
        asaobj->deinit(asaobj);
    }
} // end of _api_job_progress_fileopened_cb


static void  _api_job_progressinfo_saved_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    api_usr_data_t  *usrdata  = asaobj->cb_args.entries[ASA_USRARG_INDEX__APIUSRDATA];
    void *_out_ctx = apprpc_recv_reply_restart (usrdata->rpcreply_ctx);
    if(!_out_ctx) {
        usrdata->rpcreply_ctx = NULL;
        asaobj->deinit(asaobj);
    }
}


static uint8_t  _api_monitor_progress__rpcreply_update_cb (arpc_reply_cfg_t *cfg, json_t *info, ARPC_STATUS_CODE arpc_res)
{
    json_t  *reply_msgs = json_object_get(info, "rpc.media.transcode.corr_id.%s");
    uint8_t _updated = (reply_msgs != NULL) && (json_array_size(reply_msgs) > 0);
    uint8_t _continue = _updated;
    api_usr_data_t *usrdata = cfg->usr_data;
    asa_op_base_cfg_t  *_asaobj = usrdata->asaobj;
    uint8_t  _asa_exists = _asaobj != NULL;
    if(!_asa_exists) {
        h2o_req_t *req = usrdata->req;
        _asaobj =  api_job_progress_update__init_asaobj (req->conn->ctx->loop, cfg->usr_id, NUM_USRARGS_ASA);
        _asaobj->deinit = _api_monitor_progress__deinit_asaobj;
        _asaobj->cb_args.entries[ASA_USRARG_INDEX__APIUSRDATA] = usrdata;
        usrdata->asaobj = _asaobj;
    }
    ASA_RES_CODE  asa_result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    if(_updated) {
        _asaobj->op.close.cb = _api_job_progressinfo_saved_done_cb;
        if(_asa_exists) { // open the file then write again
            asa_result = _asaobj->storage->ops.fn_open(_asaobj);
        } else { // ensure local folder and progress file are ready
            asa_result = _asaobj->storage->ops.fn_mkdir(_asaobj, 1);
        }
        _continue = asa_result == ASTORAGE_RESULT_ACCEPT;
        if(_continue) {
            json_incref(reply_msgs);
            _asaobj->cb_args.entries[ASA_USRARG_INDEX__API_RPC_REPLY_DATA] = reply_msgs;
        }
    } else { // open the file then read job progress
        _asaobj->op.open.cb = _api_job_progress_fileopened_cb;
        asa_result = _asaobj->storage->ops.fn_open(_asaobj);
    }
    if(!_continue)
        usrdata->rpcreply_ctx = NULL;
    if(asa_result != ASTORAGE_RESULT_ACCEPT)
        _asaobj->deinit(_asaobj);
    return  _continue;
} // end of _api_monitor_progress__rpcreply_update_cb


static void  _api_monitor_progress__rpcreply_err_cb (arpc_reply_cfg_t  *cfg, ARPC_STATUS_CODE result)
{
    api_usr_data_t *usrdata = cfg->usr_data;
    asa_op_base_cfg_t  *_asaobj = usrdata->asaobj;
    usrdata->req ->res.status = cfg->flags.replyq_nonexist ? 400: 503;
    usrdata->rpcreply_ctx = NULL;
    if(_asaobj) {
        _asaobj->deinit(_asaobj);
    } else {
        _api_monitor_progress__deinit_primitives(usrdata->req, usrdata->hdlr,
                usrdata->node, usrdata->qparam, usrdata->res_body);
        free(usrdata);
    }
} // end of _api_monitor_progress__rpcreply_err_cb



RESTAPI_ENDPOINT_HANDLER(monitor_job_progress, GET, hdlr, req)
{
    json_t *res_body = json_object();
    json_t *qparam = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparam);
    const char *job_id = json_string_value(json_object_get(qparam, "id"));
    size_t  job_id_actual_sz = strlen(job_id);
    req->res.status = 0;
    if(job_id_actual_sz >= MAX_BYTES_JOB_ID)
        json_object_set_new(res_body, "id", json_string("exceeding max limit"));
    if(json_object_size(res_body) == 0) {
        int err = app_verify_printable_string(job_id, job_id_actual_sz);
        if(err)
            json_object_set_new(res_body, "id", json_string("contains non-printable charater"));
    }
    if(!req->conn->ctx->storage.entries || req->conn->ctx->storage.size < 2) {
        req->res.status = 503;
        req->res.reason = "missing rpc context";
        json_object_set_new(res_body, "reason", json_string("essential service not available"));
#if  1
        fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, ctx storage size:%ld \n",
                __LINE__, req->conn->ctx->storage.size );
#endif
    } // missing rpc context object
    if(json_object_size(res_body) == 0) {
        api_usr_data_t *usrdata = calloc(1, sizeof(api_usr_data_t));
        *usrdata = (api_usr_data_t) {.req=req, .hdlr=hdlr, .node=node,
             .qparam=qparam, .res_body=res_body };
        json_t *jwt_claims = (json_t *) app_fetch_from_hashmap(node->data, "auth");
        arpc_reply_cfg_t   rpc_cfg = {
            .usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile")),
            .loop=req->conn->ctx->loop,   .conn=req->conn->ctx->storage.entries[1].data,
            .on_error=_api_monitor_progress__rpcreply_err_cb,
            .on_update=_api_monitor_progress__rpcreply_update_cb,
            .get_reply_fn=app_rpc_fetch_replies, .timeout_ms=8,
            .max_num_msgs_fetched=3,  .usr_data=usrdata
        };
        void *rpc_reply_ctx = apprpc_recv_reply_start (&rpc_cfg);
        if(rpc_reply_ctx) {
            usrdata->rpcreply_ctx = rpc_reply_ctx;
        } else {
            free(usrdata);
            json_object_set_new(res_body, "reason", json_string("essential service not available"));
        }
    } else { // invalid job ID
        req->res.status = 400;
    }
    if(json_object_size(res_body) > 0)
        _api_monitor_progress__deinit_primitives(req, hdlr, node, qparam, res_body);
    return 0;
} // end of monitor_job_progress
