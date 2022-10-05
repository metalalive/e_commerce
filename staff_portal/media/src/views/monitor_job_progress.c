#include "utils.h"
#include "views.h"
#include "rpc/core.h"
#include "storage/cfg_parser.h"
#include "storage/localfs.h"

#define   PROGRESS_INFO_FILENAME  "progress_info"

#define   ASA_USRARG_INDEX__H2REQ        0
#define   ASA_USRARG_INDEX__H2HANDLER    1
#define   ASA_USRARG_INDEX__MIDDLEWARE   2
#define   ASA_USRARG_INDEX__QUERY_PARAM  3
#define   ASA_USRARG_INDEX__RESP_BODY    4
#define   NUM_USRARGS_ASA  (ASA_USRARG_INDEX__RESP_BODY + 1)

static void  _api_monitor_progress__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *qparam, json_t *res_body)
{
    size_t  nrequired = json_dumpb((const json_t *)res_body, NULL, 0, 0) + 1;
    char    body_raw[nrequired] ;
    size_t  nwrite = json_dumpb((const json_t *)res_body, &body_raw[0], nrequired, JSON_COMPACT);
    body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, &body_raw[0], strlen(&body_raw[0]));
    json_decref(qparam);
    json_decref(res_body);
    app_run_next_middleware(hdlr, req, node);
} // end of _api_monitor_progress__deinit_primitives

static void  _api_monitor_progress__deinit_asaobj (asa_op_base_cfg_t *asaobj)
{
    h2o_req_t     *req  = asaobj->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    h2o_handler_t *hdlr = asaobj->cb_args.entries[ASA_USRARG_INDEX__H2HANDLER];
    app_middleware_node_t *node = asaobj->cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE];
    json_t *qparam   = asaobj->cb_args.entries[ASA_USRARG_INDEX__QUERY_PARAM];
    json_t *res_body = asaobj->cb_args.entries[ASA_USRARG_INDEX__RESP_BODY];
    free(asaobj);
    _api_monitor_progress__deinit_primitives(req, hdlr, node, qparam, res_body);
}


static void _api_monitor_progress__closefile_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{  _api_monitor_progress__deinit_asaobj(asaobj); } // TODO , log error

static void  _api_monitor_progress__update_info_cb (const char *msg, size_t sz, arpc_exe_arg_t *arg)
{
    json_error_t  j_err = {0};
    json_t *new_reply = NULL;
    if(arg->job_id.len == 0 || !arg->job_id.bytes)
        goto done; // discard due to lack of job ID
    new_reply = json_loadb(msg, sz, JSON_REJECT_DUPLICATES, &j_err);
    if(j_err.line >= 0 || j_err.column >= 0) {
#if 1
        fprintf(stderr, "[API] monitor job progress, line:%d, junk data :%s \n", __LINE__, msg);
#endif
        goto done; // discard junk data
    }
    json_t *info = arg->usr_data;
    json_t *jobs_item = json_object_get(info, "jobs");
    json_t  *progress_item = json_object_get(new_reply, "progress");
    if(progress_item) {
        float percent_done = (float) json_real_value(progress_item);
        if(percent_done < 0.0f)
            goto done;
        json_t *job_item  = json_object();
        json_object_set_new(job_item, "percent_done", json_real(percent_done) );
        json_object_set_new(job_item, "timestamp", json_integer(arg->_timestamp) );
        json_object_deln(    jobs_item, arg->job_id.bytes, arg->job_id.len);
        json_object_setn_new(jobs_item, arg->job_id.bytes, arg->job_id.len, job_item);
    } else {
        json_object_deln(jobs_item, arg->job_id.bytes, arg->job_id.len);
        json_object_setn(jobs_item, arg->job_id.bytes, arg->job_id.len, new_reply);
    } // report error, error detail does not contain progress message
done:
    if(new_reply)
        json_decref(new_reply);
} // end of  _api_monitor_progress__update_info_cb


static void  _api_monitor_progress__openfile_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    asa_op_localfs_cfg_t * asa_local = (asa_op_localfs_cfg_t *)asaobj;
    h2o_req_t  *req  = asaobj->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    json_t *qparams  = asaobj->cb_args.entries[ASA_USRARG_INDEX__QUERY_PARAM];
    json_t *res_body = asaobj->cb_args.entries[ASA_USRARG_INDEX__RESP_BODY];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        int fd = asa_local->file.file;
        json_error_t  j_err = {0};   // load entire file, it shouldn't be that large in most cases
        json_t *info = json_loadfd(fd, JSON_REJECT_DUPLICATES, &j_err);
        if(!info)
            info = json_object();
        if(!json_object_get(info, "usr_id")) {
            app_middleware_node_t  *node = asaobj->cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE];
            json_t *jwt_claims = (json_t *) app_fetch_from_hashmap(node->data, "auth");
            uint32_t _usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
            json_object_set_new(info, "usr_id", json_integer(_usr_id));
        }
        if(!json_object_get(info, "jobs"))
            json_object_set_new(info, "jobs", json_object());
        arpc_exe_arg_t  rpc_arg = {.alias="app_mqbroker_1", .usr_data=(void *)info,
                  .conn=req->conn->ctx->storage.entries[1].data };
        ARPC_STATUS_CODE  arpc_res = app_rpc_fetch_all_reply_msg(&rpc_arg, _api_monitor_progress__update_info_cb);
        ftruncate(fd, (off_t)0);
        lseek(fd, 0, SEEK_SET);
        json_dumpfd((const json_t *)info, fd, JSON_COMPACT);
        if(arpc_res == APPRPC_RESP_OK) { // fetch progress field associated with given job ID
            const char *req_job_id = json_string_value(json_object_get(qparams, "id"));
            json_t  *jobs_item = json_object_get(info, "jobs");
            json_t  *result_item = json_object_get(jobs_item, req_job_id);
            if(result_item) { // either progress or error detail
                req->res.status = 200;
                json_object_set_new(res_body, req_job_id, result_item);
            } else {
                req->res.status = 404;
                json_object_set_new(res_body, "reason", json_string("job ID not found"));
            }
        } else if(arpc_res == APPRPC_RESP_MSGQ_OPERATION_ERROR || arpc_res == APPRPC_RESP_MSGQ_OPERATION_TIMEOUT) {
            req->res.status = 404;
            json_object_set_new(res_body, "reason", json_string("job queue not found"));
        } else {
#if  1
            fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, RPC result:%d \n",
                __LINE__, arpc_res);
#endif
            req->res.status = 503;
            req->res.reason = "message queue failure";
        }
        asaobj->op.close.cb = _api_monitor_progress__closefile_done_cb;
        result = asaobj->storage->ops.fn_close(asaobj);
        if(result != ASTORAGE_RESULT_ACCEPT)
            _api_monitor_progress__deinit_asaobj(asaobj);
    } else { // failed to open progress file
#if  1
        fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, storage result:%d \n",
                __LINE__, result );
#endif
        req->res.status = 503;
        req->res.reason = "open file failure";
        _api_monitor_progress__deinit_asaobj(asaobj);
    }
} // end of _api_monitor_progress__openfile_done_cb


static void  _api_monitor_progress__mkdir_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    h2o_req_t  *req  = asaobj->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        char *basepath = asaobj->op.open.dst_path ;
        assert(basepath[0] == 0x0);
        strncat(basepath, asaobj->op.mkdir.path.origin, strlen(asaobj->op.mkdir.path.origin));
        strncat(basepath, "/", 1);
        strncat(basepath, PROGRESS_INFO_FILENAME, strlen(PROGRESS_INFO_FILENAME));
        asaobj->op.open.cb = _api_monitor_progress__openfile_done_cb;
        asaobj->op.open.mode  = S_IRUSR | S_IWUSR;
        asaobj->op.open.flags = O_RDWR | O_CREAT;
        result = asaobj->storage->ops.fn_open(asaobj);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            req->res.status = 503;
            req->res.reason = "error on sending open file cmd";
            _api_monitor_progress__deinit_asaobj(asaobj);
        }
    } else {
#if  1
        fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, storage result:%d \n",
                __LINE__, result );
#endif
        req->res.status = 503;
        req->res.reason = "mkdir failure";
        _api_monitor_progress__deinit_asaobj(asaobj);
    }
} // end of _api_monitor_progress__mkdir_done_cb


static  asa_op_localfs_cfg_t * _api_monitor_progress__init_asa_obj (h2o_req_t *req, h2o_handler_t *hdlr,
            app_middleware_node_t *node, json_t *qparam, json_t *res_body)
{
    app_cfg_t *app_cfg = app_get_global_cfg();
    asa_cfg_t *storage =  app_storage_cfg_lookup("localfs");
    size_t  mkdir_path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1; // include NULL-terminated byte
    size_t  openf_path_sz = mkdir_path_sz + sizeof(PROGRESS_INFO_FILENAME) + 1;
    size_t  cb_args_sz = NUM_USRARGS_ASA * sizeof(void *);
    size_t  asaobj_base_sz = storage->ops.fn_typesize();
    size_t  asaobj_tot_sz  = asaobj_base_sz + cb_args_sz + openf_path_sz + (mkdir_path_sz << 1);
    asa_op_localfs_cfg_t *out = calloc(1, asaobj_tot_sz);
    char *ptr = (char *)out + asaobj_base_sz;
    out->super.cb_args.size = NUM_USRARGS_ASA;
    out->super.cb_args.entries = (void **) ptr;
    out->super.cb_args.entries[ASA_USRARG_INDEX__H2REQ] = req;
    out->super.cb_args.entries[ASA_USRARG_INDEX__H2HANDLER] = hdlr;
    out->super.cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE]  = node;
    out->super.cb_args.entries[ASA_USRARG_INDEX__QUERY_PARAM] = qparam;
    out->super.cb_args.entries[ASA_USRARG_INDEX__RESP_BODY]   = res_body;
    out->super.storage = storage;
    out->file.file = -1;
    out->loop = req->conn->ctx->loop;
    ptr += cb_args_sz;
    out->super.op.open.dst_path = ptr;
    ptr += openf_path_sz;
    out->super.op.mkdir.path.origin = ptr;
    ptr += mkdir_path_sz;
    out->super.op.mkdir.path.curr_parent = ptr;
    ptr += mkdir_path_sz;
    assert((size_t)(ptr - (char *)out) == asaobj_tot_sz);
    { // mkdir setup
        json_t *jwt_claims = (json_t *) app_fetch_from_hashmap(node->data, "auth");
        uint32_t _usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
        char *basepath = out->super.op.mkdir.path.origin;
        size_t nwrite = snprintf(basepath, mkdir_path_sz, "%s/%d", app_cfg->tmp_buf.path, _usr_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        assert(nwrite <= mkdir_path_sz);
        out->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        out->super.op.mkdir.cb = _api_monitor_progress__mkdir_done_cb;
        assert(out->super.op.mkdir.path.curr_parent[0] == 0x0);
    }
    return  out;
} // end of _api_monitor_progress__init_asa_obj


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
        fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, ctx storage size:%d \n",
                __LINE__, req->conn->ctx->storage.size );
#endif
    } // missing rpc context object
    if(json_object_size(res_body) == 0) {
        asa_op_localfs_cfg_t *asa_local = _api_monitor_progress__init_asa_obj (req, hdlr, node, qparam, res_body);
        // ensure local folder and progress file are ready
        ASA_RES_CODE  result = asa_local->super.storage->ops.fn_mkdir(&asa_local->super, 1);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            req->res.status = 503;
            req->res.reason = "error when issuing mkdir cmd to storage";
            free(asa_local);
#if  1
            fprintf(stderr, "[monitor_job_progress] http_resp:503, line:%d, storage error code:%d \n",
                __LINE__, result );
#endif
        }
    } else {
        req->res.status = 400;
        req->res.reason = "invalid job ID";
    }
    if(req->res.status >= 400)
        _api_monitor_progress__deinit_primitives(req, hdlr, node, qparam, res_body);
    return 0;
} // end of monitor_job_progress
