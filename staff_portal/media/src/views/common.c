#include <ctype.h>
#include <errno.h>
#include <sys/file.h>
#include <openssl/sha.h>
#include <curl/curl.h>

#include "utils.h"
#include "acl.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"
#include "storage/cfg_parser.h"
#include "storage/localfs.h"
#include "rpc/datatypes.h"

#define  DB_OP__USRARG_IDX__HTTP_REQ     0
#define  DB_OP__USRARG_IDX__HTTP_HDLR    1
#define  DB_OP__USRARG_IDX__MIDDLEWARE_NODE  2
#define  DB_OP__USRARG_IDX__NB_ROWS_RD    3
#define  DB_OP__USRARG_IDX__SUCCESS_CB    4
#define  DB_OP__USRARG_IDX__FAILURE_CB    5
#define  DB_OP__USRARG_IDX__RES_OWNER_ID  6
#define  DB_OP__USRARG_IDX__UPLD_REQ_ID   7

#define   JOB_PROGRESS_INFO_FILENAME  "job_progress.json"

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail)
{ (void *)detail; }

static void  _app__upld_req_exist__row_fetch(db_query_t *target, db_query_result_t *rs)
{ // supposed to be invoked only once
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__NB_ROWS_RD];
#pragma GCC diagnostic pop
    target->cfg.usr_data.entry[DB_OP__USRARG_IDX__NB_ROWS_RD] = (void *) num_rows_read + 1;
} // end of _app__upld_req_exist__row_fetch


static void  _app__upld_req_exist__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *)     target->cfg.usr_data.entry[DB_OP__USRARG_IDX__HTTP_REQ];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__HTTP_HDLR];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__MIDDLEWARE_NODE];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__NB_ROWS_RD];
#pragma GCC diagnostic pop
    if (rs->app_result == DBA_RESULT_OK) {
        // check quota limit, estimate all uploaded chunks of the user
        app_middleware_fn  cb = NULL;
        if(num_rows_read == 0) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__FAILURE_CB];
            cb(self, req, node);
        } else if(num_rows_read == 1) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[DB_OP__USRARG_IDX__SUCCESS_CB];
            cb(self, req, node);
        } else {
            target->cfg.callbacks.error(target, rs);
        }
    } else {
        target->cfg.callbacks.error(target, rs);
    }
} // end of _app__upld_req_exist__rs_free


#define GET_USR_PROF_ID(node, usr_id) \
{ \
    usr_id  = 0; \
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth"); \
    if(jwt_claims) { \
        usr_id = (int) json_integer_value(json_object_get(jwt_claims, "profile")); \
    } \
    if(usr_id == 0) { \
        return DBA_RESULT_ERROR_ARG; \
    } \
}

DBA_RES_CODE  app_validate_uncommitted_upld_req(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node,
        const char *db_table, void (*err_cb)(db_query_t *, db_query_result_t *), app_middleware_fn success_cb,
        app_middleware_fn failure_cb)
{
    if(!self || !req || !node || !db_table || !err_cb || !failure_cb || !success_cb)
        return DBA_RESULT_ERROR_ARG;
    int usr_id  = 0;
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    int req_seq = (int)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
    GET_USR_PROF_ID(node, usr_id);
    // TODO, conditionally exclude committed / uncommitted requests
#define SQL_PATTERN "SELECT `usr_id` FROM `%s` WHERE `usr_id` = %u AND `req_id` = x'%08x' AND `time_committed` IS NULL;"
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(db_table) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(req_seq);
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, db_table, usr_id, req_seq);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define  NUM_USR_ARGS   (DB_OP__USRARG_IDX__FAILURE_CB + 1)
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node,
            (void *)0, (void *)success_cb, (void *)failure_cb };
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = _app__upld_req_exist__row_fetch,
            .result_free = _app__upld_req_exist__rs_free,
            .error = err_cb,
        }
    };
    return app_db_query_start(&cfg);
#undef NUM_USR_ARGS
} // end of  app_validate_uncommitted_upld_req


const char *app_resource_id__url_decode(json_t *spec, json_t *err_info)
{
    const char *resource_id = json_string_value(json_object_get(spec, "res_id")); // URL-encoded
    if(resource_id) {
        size_t res_id_sz = strlen(resource_id);
        size_t  max_res_id_sz = APP_RESOURCE_ID_SIZE * 3; // consider it is URL-encoded
        if(res_id_sz > max_res_id_sz) {
            json_object_set_new(err_info, "resource_id", json_string("exceeding max limit"));
        } else { // resource ID from frontend client should always be URL-encoded
            int   out_len  = 0;
            char *res_id_uri_decoded = curl_easy_unescape(NULL, resource_id, (int)res_id_sz, &out_len);
            json_object_set_new(spec, "res_id", json_string(res_id_uri_decoded));
            free(res_id_uri_decoded);
            resource_id = json_string_value(json_object_get(spec, "res_id"));
            res_id_sz = strlen(resource_id);
            int err = app_verify_printable_string(resource_id, res_id_sz);
            if(err)
                json_object_set_new(err_info, "resource_id", json_string("contains non-printable charater"));
        }
    } else {
        json_object_set_new(err_info, "query", json_string("missing resource id in URL"));
    }
    return resource_id;
} // end of  app_resource_id__url_decode


int  api_http_resp_status__verify_resource_id (aacl_result_t *result, json_t *err_info)
{
    int resp_status = 0;
    if(result->flag.error || result->flag.res_id_dup) {
        h2o_error_printf("[api][common] line:%d, err=%u, dup=%u \n", __LINE__,
                result->flag.error, result->flag.res_id_dup);
        json_object_set_new(err_info, "res_id", json_string("internal error"));
        resp_status = 500;
    } else if (!result->flag.res_id_exists) {
        json_object_set_new(err_info, "res_id", json_string("not exists"));
        resp_status = 404;
    }
    return resp_status;
} // end of  api_http_resp_status__verify_resource_id



int  app_verify_printable_string(const char *str, size_t limit_sz)
{  // Note in this application,  this function does not allow whitespace
   // and does NOT prevent SQL injection
    int err = 0;
    if(!str || limit_sz == 0) {
        err = 1;
        goto done;
    }
    size_t actual_sz = strlen(str);
    if(actual_sz == 0 || actual_sz > limit_sz) {
        err = 2;
        goto done;
    }
    for(size_t idx = 0; (!err) && (idx < actual_sz); idx++) {
        int c = (int)str[idx];
        err = (isprint(c) == 0) || isspace(c);
    }
done:
    return err;
} // end of app_verify_printable_string

ARPC_STATUS_CODE api__render_rpc_reply_qname(
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    size_t tot_wr_sz = strlen(name_pattern) + USR_ID_STR_SIZE;
    if(tot_wr_sz > wr_sz) {
        fprintf(stderr, "[api][common] line:%d, insufficient buffer, required:%ld,actual:%ld \n",
                __LINE__, tot_wr_sz, wr_sz );
        return APPRPC_RESP_MEMORY_ERROR;
    }
    json_t *_usr_data = (json_t *)args->usr_data;
    uint32_t usr_prof_id = (uint32_t) json_integer_value(json_object_get(_usr_data,"usr_id"));
    if(usr_prof_id > 0) {
        snprintf(wr_buf, wr_sz, name_pattern, usr_prof_id);
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api__render_rpc_reply_qname


ARPC_STATUS_CODE api__default__render_rpc_corr_id (
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    size_t md_hex_sz = (SHA_DIGEST_LENGTH << 1) + 1;
    size_t tot_wr_sz = strlen(name_pattern) + md_hex_sz;
    if(tot_wr_sz > wr_sz) {
        fprintf(stderr, "[api][common] line:%d, insufficient buffer, required:%ld,actual:%ld \n",
                __LINE__, tot_wr_sz, wr_sz );
        return APPRPC_RESP_MEMORY_ERROR;
    }
    json_t *_usr_data = (json_t *)args->usr_data;
    uint32_t usr_prof_id = (uint32_t) json_integer_value(json_object_get(_usr_data,"usr_id"));
    if(usr_prof_id > 0) {
        SHA_CTX  sha_ctx = {0};
        SHA1_Init(&sha_ctx);
        SHA1_Update(&sha_ctx, (const char *)&usr_prof_id, sizeof(usr_prof_id));
        SHA1_Update(&sha_ctx, (const char *)&args->_timestamp, sizeof(args->_timestamp));
        char md[SHA_DIGEST_LENGTH] = {0};
        char md_hex[md_hex_sz];
        SHA1_Final((unsigned char *)&md[0], &sha_ctx);
        app_chararray_to_hexstr(&md_hex[0], md_hex_sz - 1, &md[0], SHA_DIGEST_LENGTH);
        md_hex[md_hex_sz - 1] = 0x0;
        size_t nwrite = snprintf(wr_buf, wr_sz, name_pattern, &md_hex[0]);
        if(nwrite >= wr_sz)
            status = APPRPC_RESP_MEMORY_ERROR;
        OPENSSL_cleanse(&sha_ctx, sizeof(SHA_CTX));
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api__default__render_rpc_corr_id



static void _api_progressinfo_update (int fd, json_t *reply_msgs)
{
    json_error_t  j_err = {0};   // load entire file, it shouldn't be that large in most cases
    json_t *info = NULL, *_packed = NULL;
    int idx = 0;
    lseek(fd, 0, SEEK_SET);
    info = json_loadfd(fd, JSON_REJECT_DUPLICATES, &j_err);
    if(!info)
        info = json_object();
    json_array_foreach(reply_msgs, idx, _packed) {
        json_t *corr_id_item = json_object_get(_packed, "corr_id");
        json_t *msg_item = json_object_get(_packed, "msg");
        uint64_t  ts_done = json_integer_value(json_object_get(_packed, "timestamp"));
        const char * corr_id = json_string_value(json_object_get(corr_id_item, "data"));
        size_t  corr_id_sz   = json_integer_value(json_object_get(corr_id_item, "size"));
        const char *msg = json_string_value(json_object_get(msg_item, "data"));
        size_t  msg_sz  = json_integer_value(json_object_get(msg_item, "size"));
        j_err = (json_error_t){0};
        json_t *_reply = json_loadb(msg, msg_sz, JSON_REJECT_DUPLICATES, &j_err);
        if(j_err.line >= 0 || j_err.column >= 0) { //  discard junk data
            fprintf(stderr, "[api][common] line:%d, corr_id:%s, msg:%s \n", __LINE__, corr_id, msg);
        } else { // -----------------------------
            json_t  *job_item  = json_object_getn(info, corr_id, corr_id_sz);
            uint8_t _new_item_add = !job_item;
            if(_new_item_add)
                job_item = json_object();
            json_t  *err_info_item = json_object_get(_reply, "error");
            json_t  *progress_item = json_object_get(_reply, "progress");
            if(progress_item) {
                float _percent_done = (float) json_real_value(progress_item);
                json_object_set_new(job_item, "percent_done", json_real(_percent_done));
            } // report error, error detail does not contain progress message
            if(err_info_item)
                json_object_set(job_item, "error", err_info_item);
            json_object_set_new(job_item, "timestamp", json_integer(ts_done) );
            if(_new_item_add)
                json_object_setn_new(info, corr_id, corr_id_sz, job_item);
            json_decref(_reply);
        } // ---------------------------
    } // end of reply message iteration
    ftruncate(fd, (off_t)0);
    lseek(fd, 0, SEEK_SET);
    json_dumpfd((const json_t *)info, fd, JSON_COMPACT); // will call low-level write() without buffering this
    json_decref(info);
} // end of  _api_progressinfo_update


static void  _api_job_progress_fileopened_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    asa_op_localfs_cfg_t * _asa_local = (asa_op_localfs_cfg_t *)asaobj;
    json_t *reply_msgs = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_RPC_REPLY_DATA];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        int fd = _asa_local->file.file;
        int ret = flock(fd, LOCK_EX); //  | LOCK_NB
        if(ret == 0) {
            _api_progressinfo_update (fd, reply_msgs);
            flock(fd, LOCK_UN); //  | LOCK_NB
        } else {
            fprintf(stderr, "[api][common] line:%d, error (%d) when locking file \r\n", __LINE__, errno);
        }
        result = asaobj->storage->ops.fn_close(asaobj);
    }
    json_decref(reply_msgs);
    asaobj->cb_args.entries[ASA_USRARG_INDEX__API_RPC_REPLY_DATA] = NULL;
    if(result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[api][common] line:%d, storage result:%d \n", __LINE__, result );
        asaobj->deinit(asaobj);
    }
} // end of _api_job_progress_fileopened_cb


static void  _api_ensure_progress_update_filepath_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asaobj->op.open.cb = _api_job_progress_fileopened_cb;
        asaobj->op.open.mode  = S_IRUSR | S_IWUSR;
        asaobj->op.open.flags = O_RDWR | O_CREAT;
        result = asaobj->storage->ops.fn_open(asaobj);
    }
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_t *reply_msgs = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_RPC_REPLY_DATA];
        json_decref(reply_msgs);
        asaobj->cb_args.entries[ASA_USRARG_INDEX__API_RPC_REPLY_DATA] = NULL;
        fprintf(stderr, "[api][common] line:%d, storage result:%d \n", __LINE__, result );
        asaobj->deinit(asaobj);
    }
} // end of _api_ensure_progress_update_filepath_cb


asa_op_base_cfg_t * api_job_progress_update__init_asaobj (void *loop, uint32_t usr_id, size_t num_usr_args)
{
    asa_cfg_t *storage =  app_storage_cfg_lookup("localfs");
    app_cfg_t *app_cfg = app_get_global_cfg();
    size_t  mkdir_path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1; // include NULL-terminated byte
    size_t  openf_path_sz = mkdir_path_sz + sizeof(JOB_PROGRESS_INFO_FILENAME) + 1;
    size_t  cb_args_sz = num_usr_args * sizeof(void *);
    size_t  asaobj_base_sz = storage->ops.fn_typesize();
    size_t  asaobj_tot_sz  = asaobj_base_sz + cb_args_sz + (mkdir_path_sz << 1) + openf_path_sz;
    asa_op_localfs_cfg_t *asa_local = calloc(1, asaobj_tot_sz);
    asa_op_base_cfg_t *asaobj = &asa_local->super;
    char *ptr = (char *)asaobj + asaobj_base_sz;
    asaobj->cb_args.size = num_usr_args;
    asaobj->cb_args.entries = (void **) ptr;
    ptr += cb_args_sz;
    asaobj->op.mkdir.path.origin = ptr;
    ptr += mkdir_path_sz;
    asaobj->op.mkdir.path.curr_parent = ptr;
    ptr += mkdir_path_sz;
    asaobj->op.open.dst_path = ptr;
    ptr += openf_path_sz;
    assert((size_t)(ptr - (char *)asaobj) == asaobj_tot_sz);
    asaobj->storage = storage;
    asa_local->file.file = -1;
    asa_local->loop = loop;
    asaobj->deinit = NULL;
    { // mkdir setup
        char *basepath = asaobj->op.mkdir.path.origin;
        size_t nwrite = snprintf(basepath, mkdir_path_sz, "%s/%d", app_cfg->tmp_buf.path, usr_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        assert(nwrite <= mkdir_path_sz);
        asaobj->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asaobj->op.mkdir.cb = _api_ensure_progress_update_filepath_cb;
        assert(asaobj->op.mkdir.path.curr_parent[0] == 0x0);
    } { // build file path in advance
        char *basepath = asaobj->op.open.dst_path ;
        size_t nwrite = snprintf( basepath, openf_path_sz, "%s/%s", asaobj->op.mkdir.path.origin,
                JOB_PROGRESS_INFO_FILENAME );
        basepath[nwrite++] = 0x0; // NULL-terminated
        assert(nwrite <= openf_path_sz);
    }
    return  asaobj;
} // end of api_job_progress_update__init_asaobj

