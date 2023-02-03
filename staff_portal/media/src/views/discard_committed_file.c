#include "utils.h"
#include "acl.h"
#include "base64.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"
#include "storage/cfg_parser.h"
#include "transcoder/video/common.h"
#include "transcoder/image/common.h"

#define  RESOURCE_PATH_PATTERN  "%s/%d/%08x"

#define  HTTPREQ_INDEX__IN_ASA_USRARG    (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  HTTPHDLR_INDEX__IN_ASA_USRARG   (ASAMAP_INDEX__IN_ASA_USRARG + 2)
#define  MIDDLEWARE_INDEX__IN_ASA_USRARG   (ASAMAP_INDEX__IN_ASA_USRARG + 3)
#define  NUM_USR_ARGS_ASA_OBJ   (MIDDLEWARE_INDEX__IN_ASA_USRARG + 1)

#define  PREP_STMT_LABEL__DELETE_TRANSCODE_METADATA  "app_media_transcode_metadata_delete"
#define  PREP_STMT_LABEL__DELETE_RESOURCE_FILE_ACL   "app_media_resource_file_acl_delete"
#define  PREP_STMT_LABEL__DELETE_RESOURCE_USER_ACL   "app_media_resource_user_acl_delete"
#define  PREP_STMT_LABEL__DELETE_RESOURCE_BASE       "app_media_resource_base_delete"
#define  PREP_STMT_LABEL__DELETE_UPLD_FILECHUNK      "app_media_upld_filechunk_delete"
#define  PREP_STMT_LABEL__DELETE_UPLD_REQUEST        "app_media_upld_request_delete"

#define  PREP_STMT_SQL__DELETE_TRANSCODE_METADATA  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_TRANSCODE_METADATA"` FROM 'DELETE FROM `%s` WHERE `file_id`=?';"
#define  PREP_STMT_SQL__DELETE_RESOURCE_FILE_ACL  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_RESOURCE_FILE_ACL"` FROM 'DELETE FROM `filelvl_access_ctrl` WHERE `file_id`=?';"
#define  PREP_STMT_SQL__DELETE_RESOURCE_USER_ACL  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_RESOURCE_USER_ACL"` FROM 'DELETE FROM `usrlvl_access_ctrl` WHERE `file_id`=?';"
#define  PREP_STMT_SQL__DELETE_RESOURCE_BASE  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_RESOURCE_BASE"` FROM 'DELETE FROM `uploaded_file` WHERE `id`=?';"
#define  PREP_STMT_SQL__DELETE_UPLD_FILECHUNK  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_UPLD_FILECHUNK"` FROM 'DELETE FROM `upload_filechunk` WHERE `usr_id`=? AND `req_id`=?';"
#define  PREP_STMT_SQL__DELETE_UPLD_REQUEST  \
    "PREPARE `"PREP_STMT_LABEL__DELETE_UPLD_REQUEST"` FROM 'DELETE FROM `upload_request` WHERE `usr_id`=? AND `req_id`=?';"

#define  EXE_PREP_STMT__DELETE_TRANSCODE_METADATA  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_TRANSCODE_METADATA"` USING FROM_BASE64('%s');"
#define  EXE_PREP_STMT__DELETE_RESOURCE_FILE_ACL  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_RESOURCE_FILE_ACL"` USING FROM_BASE64('%s');"
#define  EXE_PREP_STMT__DELETE_RESOURCE_USER_ACL  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_RESOURCE_USER_ACL"` USING FROM_BASE64('%s');"
#define  EXE_PREP_STMT__DELETE_RESOURCE_BASE  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_RESOURCE_BASE"` USING FROM_BASE64('%s');"
#define  EXE_PREP_STMT__DELETE_UPLD_FILECHUNK  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_UPLD_FILECHUNK"` USING %u,x'%08x';"
#define  EXE_PREP_STMT__DELETE_UPLD_REQUEST  \
    "EXECUTE `"PREP_STMT_LABEL__DELETE_UPLD_REQUEST"` USING %u,x'%08x';"

#define  SQL_PATTERN \
    "BEGIN NOT ATOMIC" \
    "  "PREP_STMT_SQL__DELETE_TRANSCODE_METADATA \
    "  "PREP_STMT_SQL__DELETE_RESOURCE_FILE_ACL \
    "  "PREP_STMT_SQL__DELETE_RESOURCE_USER_ACL \
    "  "PREP_STMT_SQL__DELETE_RESOURCE_BASE  \
    "  "PREP_STMT_SQL__DELETE_UPLD_FILECHUNK \
    "  "PREP_STMT_SQL__DELETE_UPLD_REQUEST   \
    "  START TRANSACTION;" \
    "    "EXE_PREP_STMT__DELETE_TRANSCODE_METADATA  \
    "    "EXE_PREP_STMT__DELETE_RESOURCE_FILE_ACL  \
    "    "EXE_PREP_STMT__DELETE_RESOURCE_USER_ACL  \
    "    "EXE_PREP_STMT__DELETE_RESOURCE_BASE   \
    "    "EXE_PREP_STMT__DELETE_UPLD_FILECHUNK  \
    "    "EXE_PREP_STMT__DELETE_UPLD_REQUEST  \
    "  COMMIT;" \
    "END;"


static void api_discard_committedfile__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info)
{
    size_t  nrequired = json_dumpb((const json_t *)err_info, NULL, 0, 0) + 1;
    char    body_raw[nrequired] ;
    size_t  nwrite = json_dumpb((const json_t *)err_info, &body_raw[0], nrequired, JSON_COMPACT);
    body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    if(req->res.status == 0)
        req->res.status = 500;
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, &body_raw[0], strlen(&body_raw[0]));
    json_decref(spec);
    json_decref(err_info);
    app_run_next_middleware(hdlr, req, node);
}

static void  api_discard_committedfile__deinit_asaobj (asa_op_base_cfg_t *asaobj)
{
    h2o_req_t     *req  = asaobj->cb_args.entries[HTTPREQ_INDEX__IN_ASA_USRARG];
    h2o_handler_t *hdlr = asaobj->cb_args.entries[HTTPHDLR_INDEX__IN_ASA_USRARG];
    app_middleware_node_t *node = asaobj->cb_args.entries[MIDDLEWARE_INDEX__IN_ASA_USRARG];
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *qparams = processor->data.spec, *err_info = processor->data.error;
    api_discard_committedfile__deinit_primitives (req, hdlr, node, qparams, err_info);
    free(asaobj);
    free(processor);
}


static void  _api_discardfile__clean_resource_metadata_done(db_query_t *target, db_query_result_t *rs)
{
    assert(rs->_final);
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    h2o_req_t  *req  = asa_remote->cb_args.entries[HTTPREQ_INDEX__IN_ASA_USRARG];
    if(json_object_size(err_info) == 0)
        req->res.status = 204;
    api_discard_committedfile__deinit_asaobj (asa_remote);
}

static void  _api_discardfile__clean_resource_metadata__db_err(db_query_t *target, db_query_result_t *rs)
{
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    h2o_req_t  *req  = asa_remote->cb_args.entries[HTTPREQ_INDEX__IN_ASA_USRARG];
    req->res.status = 503;
    fprintf(stderr, "[api][discard_file][atfp] line:%d, remote database error, usr:%u, res_id:%u, alias:%s"
            ", conn-state:%d, conn_result:%d \r\n", __LINE__, processor->data.usr_id,  processor->data.upld_req_id
            , rs->conn.alias, rs->conn.state, rs->app_result);
    api_discard_committedfile__deinit_asaobj (asa_remote);
}


static void  _api_discardfile__clean_resource_metadata_start (atfp_t *processor, uv_loop_t *loop)
{
    json_t *err_info = processor->data.error, *spec = processor->data.spec;
    const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
    const char *meta_dbtable = json_string_value(json_object_get(spec, "transcoded_metadata_db_table"));
    uint32_t _usr_id = processor->data.usr_id;
    uint32_t last_upld_req = processor->data.upld_req_id;
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(meta_dbtable) + strlen(_res_id_encoded) * 4 +
        USR_ID_STR_SIZE * 2 + UPLOAD_INT2HEX_SIZE(last_upld_req) * 2;
    char raw_sql[raw_sql_sz];
    size_t nwrite = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, meta_dbtable, _res_id_encoded,
            _res_id_encoded, _res_id_encoded, _res_id_encoded, _usr_id, last_upld_req, _usr_id,
            last_upld_req);
    raw_sql[nwrite] = 0;
    assert(nwrite < raw_sql_sz);
    void *_usr_data[1] = {(void *)processor,};
    db_query_cfg_t  cfg = {.pool=app_db_pool_get_pool("db_server_1"), .loop=loop,
        .statements={.entry=&raw_sql[0], .num_rs=1},  .usr_data={.entry=(void **)&_usr_data, .len=1},
        .callbacks = {.result_rdy=_api_discardfile__clean_resource_metadata_done,
            .error=_api_discardfile__clean_resource_metadata__db_err,
            .row_fetched=app_db_async_dummy_cb, .result_free=app_db_async_dummy_cb,
        }};
    if(app_db_query_start(&cfg) != DBA_RESULT_OK) {
        json_object_set_new(err_info, "reason", json_string("internal error"));
        fprintf(stderr, "[api][discard_file][atfp] line:%d, failed to issue query to database"
           ", usr:%u, res_id:%u \r\n", __LINE__, processor->data.usr_id, processor->data.upld_req_id);
    }
} // end of  _api_discardfile__clean_resource_metadata_start


static void _api_discardfile__rm_upld_fchunks_done(atfp_t *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    if(json_object_size(err_info) == 0) {
        h2o_req_t  *req  = asa_remote->cb_args.entries[HTTPREQ_INDEX__IN_ASA_USRARG];
        _api_discardfile__clean_resource_metadata_start (processor, req->conn->ctx->loop);
    } else { // TODO, further check whether the error comes from scandir result
        fprintf(stderr, "[api][discard_file][atfp] line:%d, failed to remove entire"
                " folders, usr:%u, res_id:%u \r\n", __LINE__, processor->data.usr_id,
                processor->data.upld_req_id);
    }
    if(json_object_size(err_info) > 0)
        api_discard_committedfile__deinit_asaobj (asa_remote);
} // end of  _api_discardfile__rm_upld_fchunks_done

static void _api_discardfile__rm_transcoded_done(atfp_t *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    if(json_object_size(err_info) == 0) {
        uint32_t _usr_id = processor->data.usr_id;
        uint32_t _upld_req_id = processor->data.upld_req_id;
        size_t  fullpath_sz = sizeof(RESOURCE_PATH_PATTERN) + strlen(asa_remote->storage->base_path) +
            USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id);
        char fullpath[fullpath_sz];
        size_t nwrite = snprintf(&fullpath[0], fullpath_sz, RESOURCE_PATH_PATTERN,
                asa_remote->storage->base_path, _usr_id, _upld_req_id);
        fullpath[nwrite++] = 0x0;
        assert(nwrite <= fullpath_sz);
        processor->data.callback = _api_discardfile__rm_upld_fchunks_done;
        atfp_remote_rmdir_generic (processor, &fullpath[0]);
    }
    if(json_object_size(err_info) > 0) {
        fprintf(stderr, "[api][discard_file][atfp] line:%d, failed to remove entire"
                " folders\r\n", __LINE__);
        api_discard_committedfile__deinit_asaobj (asa_remote);
    }
} // end of  _api_discardfile__rm_transcoded_done


static void  _api_discardfile__remove_transcoded_start(h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info)
{
    atfp_t *processor = calloc(1, sizeof(atfp_t));
    asa_cfg_t *storage = app_storage_cfg_lookup("localfs");
    asa_op_base_cfg_t *asa_remote = app_storage__init_asaobj_helper (storage, NUM_USR_ARGS_ASA_OBJ, 0, 0);
    ((asa_op_localfs_cfg_t *)asa_remote)->loop = req->conn->ctx->loop; // TODO
    asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = processor;
    asa_remote->cb_args.entries[HTTPREQ_INDEX__IN_ASA_USRARG] = req;
    asa_remote->cb_args.entries[HTTPHDLR_INDEX__IN_ASA_USRARG] = hdlr;
    asa_remote->cb_args.entries[MIDDLEWARE_INDEX__IN_ASA_USRARG] = node;
    processor->data = (atfp_data_t) {.spec=spec,  .error=err_info, .storage={.handle=asa_remote},
        .usr_id=(uint32_t)json_integer_value(json_object_get(spec, "resource_owner_id")),
        .upld_req_id=(uint32_t)json_integer_value(json_object_get(spec, "last_upld_req"))
    };
    void (*_rm_ver_fn)(atfp_t *, const char *) = NULL;
    const char *dbtable = NULL;
    const char *res_type = json_string_value(json_object_get(spec, "resource_file_type"));
    int ret = strncmp(res_type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) {
        _rm_ver_fn = atfp_storage_video_remove_version;
        dbtable = atfp_video__metadata_dbtable_name();
    } else {
        ret = strncmp(res_type, APP_FILETYPE_LABEL_IMAGE, sizeof(APP_FILETYPE_LABEL_IMAGE) - 1);
        if(ret == 0) {
            _rm_ver_fn = atfp_storage_image_remove_version;
            dbtable = atfp_image__metadata_dbtable_name();
        }
    }
    if(_rm_ver_fn && dbtable) {
        json_object_set_new(spec, "transcoded_metadata_db_table", json_string(dbtable));
        atfp_discard_transcoded(processor, _rm_ver_fn, _api_discardfile__rm_transcoded_done);
    } else {
        req->res.status = 400;
        json_object_set_new(err_info, "reason", json_string("unknown resource type"));
    }
    if(json_object_size(err_info) > 0)
        api_discard_committedfile__deinit_asaobj (asa_remote);
} // end of  _api_discardfile__remove_transcoded_start


static void _api_discardfile__verify_resource_owner (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *qparams  = usr_args[3];
    json_t *err_info = usr_args[4];
    int _resp_status =  api_http_resp_status__verify_resource_id (result, err_info);
    if(json_object_size(err_info) == 0) {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
        if(curr_usr_id == result->owner_usr_id) {
            json_object_set_new(qparams, "last_upld_req", json_integer(result->upld_req));
            json_object_set_new(qparams, "resource_owner_id", json_integer(result->owner_usr_id));
            json_object_set_new(qparams, "resource_file_type", json_string(&result->type[0]));
            _api_discardfile__remove_transcoded_start(req, hdlr, node, qparams, err_info);
        } else {
            req->res.status = 403;
            json_object_set_new(err_info, "reason", json_string("not allowed to delete resource"));
            api_discard_committedfile__deinit_primitives (req, hdlr, node, qparams, err_info);
        }
    } else {
        req->res.status = _resp_status;
        api_discard_committedfile__deinit_primitives (req, hdlr, node, qparams, err_info);
    }
} // end of  _api_discardfile__verify_resource_owner


RESTAPI_ENDPOINT_HANDLER(discard_committed_file, DELETE, hdlr, req)
{
    json_t *err_info = json_object(),  *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = app_resource_id__url_decode(qparams, err_info);
    if(!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t out_len = 0;
        char *_res_id_encoded = (char *) base64_encode((const unsigned char *)resource_id,
                  strlen(resource_id), &out_len);
        json_object_set_new(qparams, "res_id_encoded", json_string(_res_id_encoded));
        free(_res_id_encoded);
        _res_id_encoded = (char *)json_string_value(json_object_get(qparams, "res_id_encoded"));
        void *usr_args[5] = {req, hdlr, node, qparams, err_info};
        aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=5}, .resource_id=_res_id_encoded,
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                .fetch_acl=0, .callback=_api_discardfile__verify_resource_owner };
        int err = app_acl_verify_resource_id (&cfg);
        if(err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if(json_object_size(err_info) > 0)
        api_discard_committedfile__deinit_primitives (req, hdlr, node, qparams, err_info);
    return 0;
} // end of  discard_committed_file
