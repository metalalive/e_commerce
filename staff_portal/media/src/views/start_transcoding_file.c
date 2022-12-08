#include <openssl/sha.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>

#include "utils.h"
#include "base64.h"
#include "app_cfg.h"
#include "views.h"

#include "models/pool.h"
#include "models/query.h"
#include "rpc/core.h"
#include "storage/cfg_parser.h"

static void api__dealloc_req_hashmap (app_middleware_node_t *node) {
    char *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t *res_body_json = app_fetch_from_hashmap(node->data, "res_body_json");
    json_t *req_body_json = app_fetch_from_hashmap(node->data, "req_body_json");
    if(res_id_encoded) {
        free(res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    if(res_body_json) {
        json_decref(res_body_json);
        app_save_ptr_to_hashmap(node->data, "res_body_json", (void *)NULL);
    }
    if(req_body_json) {
        json_decref(req_body_json);
        app_save_ptr_to_hashmap(node->data, "req_body_json", (void *)NULL);
    }
} // end of api__dealloc_req_hashmap


static void  _api_start_transcode__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node)
{
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    if(req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[api][transcode] line:%d \n", __LINE__ );
    }
    // h2o_send_error_503(req, "server temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    json_t *_resp_body = app_fetch_from_hashmap(node->data, "res_body_json");
    size_t  nb_required = json_dumpb(_resp_body, NULL, 0, 0);
    if(nb_required > 0) {
        char  body[nb_required + 1];
        size_t  nwrite = json_dumpb(_resp_body, &body[0], nb_required, JSON_COMPACT);
        body[nwrite++] = 0x0;
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, strlen(&body[0]));
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    api__dealloc_req_hashmap(node);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_start_transcode__deinit_primitives


static void  api__start_transcoding__db_async_err(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    req->res.status = 503;
    _api_start_transcode__deinit_primitives (req, self, node);
} // end of api__start_transcoding__db_async_err


ARPC_STATUS_CODE api__start_transcoding__render_rpc_corr_id (
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    size_t md_hex_sz = (SHA_DIGEST_LENGTH << 1) + 1;
    size_t tot_wr_sz = strlen(name_pattern) + md_hex_sz;
    if(tot_wr_sz > wr_sz) 
        return APPRPC_RESP_MEMORY_ERROR;
    json_t *_usr_data = (json_t *)args->usr_data;
    uint32_t usr_prof_id = (uint32_t) json_integer_value(json_object_get(_usr_data,"usr_id"));
    json_t *outputs = json_object_get(_usr_data, "outputs");
    if(usr_prof_id > 0 && outputs) {
        SHA_CTX  sha_ctx = {0};
        SHA1_Init(&sha_ctx);
        SHA1_Update(&sha_ctx, (const char *)&usr_prof_id, sizeof(usr_prof_id));
        SHA1_Update(&sha_ctx, (const char *)&args->_timestamp, sizeof(args->_timestamp));
        const char *version = NULL;
        json_t *req_output = NULL;
        json_object_foreach(outputs, version, req_output) {
            SHA1_Update(&sha_ctx, version, APP_TRANSCODED_VERSION_SIZE);
        }
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
} // end of api__start_transcoding__render_rpc_corr_id



static __attribute__((optimize("O0"))) void api__transcoding_file__send_async_jobs(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{ // create async job send it to message queue, since it takes time to transcode media file
    char *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t *res_body_json = app_fetch_from_hashmap(node->data, "res_body_json");
    json_t *req_body_json = app_fetch_from_hashmap(node->data, "req_body_json");
    json_t *req_outputs = json_object_get(req_body_json, "outputs");
    json_t *elm_streams = json_object_get(req_body_json, "elementary_streams");
    json_t *parts_size  = json_object_get(req_body_json, "parts_size");
    json_t *res_id_item = json_object_get(req_body_json, "resource_id");
    json_t *usr_id_item   = json_object_get(req_body_json, "usr_id"); // resource owner, not current authorized user
    json_t *upld_req_item = json_object_get(req_body_json, "last_upld_req");
    json_object_set(res_body_json, "resource_id", res_id_item);
    // determine source and destination storage for RPC consumers, TODO, scalability
    asa_cfg_t *src_storage =  app_storage_cfg_lookup("localfs");
    asa_cfg_t *dst_storage =  app_storage_cfg_lookup("localfs");
    // TODO, improve transcoding function by following design straategies:
    // (1) reduce the redundant decode stages in RPC consumers. For the same source
    //     file transcoding to different variants, all decoders in the RPC consumers
    //     are identical, which means they generate the same decoded frames in
    //     different consumer servers. It is possible to save computational power by
    //     using only one consumer and then distribute each decoded frame to next stage
    //     scaling for different variants. (e.g. you may apply 2-level consumer scheme, the
    //     first-level consumer decodes packet and sends decoded frame to the next-level
    //     consumer for scaling and encoding)
    // (2) buffer scaled frames, which saves time for transcoding variants in one go.
    //     For example, a video transcoding to 2 variants 1024p720 and 512p360, the
    //     function can first scale a frame to 1024p720, keep the frame, use it when
    //     scaling to 512p360 (extra storage required, so this will be considered for
    //      scalability once the application grows).
    json_t *msgq_body_item = json_object();
    { // start of construct message body
        json_t *req_output = NULL;
        const char *version = NULL;
        json_object_set(msgq_body_item, "parts_size", parts_size);
        json_object_set(msgq_body_item, "resource_id", res_id_item);
        json_object_set_new(msgq_body_item, "res_id_encoded", json_string(res_id_encoded));
        json_object_set_new(msgq_body_item, "metadata_db", json_string("db_server_1"));
        json_object_set_new(msgq_body_item, "storage_alias", json_string(src_storage->alias));
        json_object_set(msgq_body_item, "usr_id", usr_id_item);
        json_object_set(msgq_body_item, "last_upld_req", upld_req_item);
        json_object_set(msgq_body_item, "elementary_streams", elm_streams);
        json_object_set(msgq_body_item, "outputs", req_outputs);
        json_object_foreach(req_outputs, version, req_output) {
            json_object_set_new(req_output, "storage_alias", json_string(dst_storage->alias));
        } // each output version may be in different storage
        size_t nb_required = json_dumpb(msgq_body_item, NULL, 0, 0);
        char *msg_body_raw = calloc(nb_required, sizeof(char));
        size_t nwrite = json_dumpb(msgq_body_item, msg_body_raw, nb_required, JSON_COMPACT);
        assert(nwrite <= nb_required);
        char job_id_raw[MAX_BYTES_JOB_ID] = {0};
        arpc_exe_arg_t  rpc_arg = {
            .conn = req->conn->ctx->storage.entries[1].data,  .job_id = {.bytes=&job_id_raw[0],
                .len=MAX_BYTES_JOB_ID }, .msg_body = {.len=nwrite, .bytes=msg_body_raw},
            .alias = "app_mqbroker_1",  .routing_key = "rpc.media.transcode_video_file",
            .usr_data = (void *)msgq_body_item,
        }; // will start a new job and transcode asynchronously
        if(app_rpc_start(&rpc_arg) == APPRPC_RESP_ACCEPTED) {
            req->res.status = 202;
            json_object_set_new(res_body_json, "job_id", json_string(&job_id_raw[0]));
        } else {
            req->res.status = 503;
            json_object_set_new(res_body_json, "job_id", json_null());
        }
        free(msg_body_raw);
    } // end of construct message body
    json_decref(msgq_body_item);
    _api_start_transcode__deinit_primitives (req, self, node);
} // end of api__transcoding_file__send_async_jobs


static void  _load_filepart_info__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *)     target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    api__transcoding_file__send_async_jobs(self, req, node);
} // end of _load_filepart_info__rs_free

static void  _load_filepart_info__row_fetch(db_query_t *target, db_query_result_t *rs)
{
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    json_t *req_body_json = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t *parts_size = json_object_get(req_body_json, "parts_size");
    uint32_t size_bytes = (uint32_t) strtoul(row->values[0], NULL, 10);
    json_array_append_new(parts_size, json_integer(size_bytes));
} // end of _load_filepart_info__row_fetch


static  void  _mark_old_transcoded_version__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *)     target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    json_t *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t _resource_owner_id = (uint32_t) json_integer_value(json_object_get(_req_body, "usr_id"));
    uint32_t _last_upld_req     = (uint32_t) json_integer_value(json_object_get(_req_body, "last_upld_req"));
#pragma GCC diagnostic pop
#define SQL_PATTERN "SELECT  `size_bytes` FROM `upload_filechunk` WHERE `usr_id` = %u " \
        " AND `req_id` = x'%08x' ORDER BY `part` ASC;"
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_last_upld_req);
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, _resource_owner_id, _last_upld_req);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define  NUM_USR_ARGS  3
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),   .loop = req->conn->ctx->loop,
        .callbacks = {.result_rdy = app_db_async_dummy_cb,   .row_fetched = _load_filepart_info__row_fetch,
            .result_free = _load_filepart_info__rs_free,  .error = api__start_transcoding__db_async_err,
        }
    };
#undef NUM_USR_ARGS
    if(app_db_query_start(&cfg) == DBA_RESULT_OK) {
        json_object_set_new(_req_body, "parts_size", json_array());
    } else {
        _api_start_transcode__deinit_primitives (req, self, node);
    }
} // end of _mark_old_transcoded_version__rs_free

static void  _mark_old_transcoded_version__row_fetch(db_query_t *target, db_query_result_t *rs)
{
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    json_t *req_body_json = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    const char *version_stored = row->values[0];
    uint16_t height_pxl_stored = (uint16_t) strtoul(row->values[1], NULL, 10);
    uint16_t width_pxl_stored  = (uint16_t) strtoul(row->values[2], NULL, 10);
    uint8_t  framerate_stored  = (uint8_t)  strtoul(row->values[3], NULL, 10);
    json_t *outputs_toplvl = json_object_get(req_body_json, "outputs");
    json_t *output_new = json_object_get(outputs_toplvl, version_stored);
    json_t *output_internal = json_object_get(output_new, "__internal__");
    const char *video_key = json_string_value(json_object_get(output_internal, "video_key"));
    json_t *elm_streams = json_object_get(req_body_json, "elementary_streams");
    json_t *elm_st_entry = json_object_get(elm_streams, video_key);
    json_t *elm_st_attri = json_object_get(elm_st_entry, "attribute");
    uint16_t  height_pxl_new = (uint16_t) json_integer_value(json_object_get(elm_st_attri, "height_pixel"));
    uint16_t  width_pxl_new  = (uint16_t) json_integer_value(json_object_get(elm_st_attri, "width_pixel"));
    uint8_t   framerate_new  = (uint8_t)  json_integer_value(json_object_get(elm_st_attri, "framerate"));
    uint8_t height_pxl_edit = height_pxl_stored != height_pxl_new;
    uint8_t width_pxl_edit  = width_pxl_stored  != width_pxl_new ;
    uint8_t framerate_edit  = framerate_stored  != framerate_new ;
    // message-queue consumer (in later step) check this field and optionally rename exising version
    // folder (to stale state, so it would be deleted after new version is transcoded)
    if(height_pxl_edit || width_pxl_edit || framerate_edit) {
        json_object_set_new(output_internal, "is_update", json_true());
    } else { // discard if the existing version doesn't change all the attributes
        // (no need to transcode again with the same attributes)
        json_object_del(outputs_toplvl, version_stored);
    }
} // end of _mark_old_transcoded_version__row_fetch

static void _mark_old_transcoded_version (RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    char  *res_id_encoded = (char *)app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t *req_body_json = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t *outputs = json_object_get(req_body_json, "outputs");
    size_t  outputs_sz = json_object_size(outputs);
#define SQL_PATTERN "EXECUTE IMMEDIATE 'SELECT `version`, `height_pixel`, `width_pixel`, `framerate` FROM" \
       " `transcoded_video_metadata` WHERE `file_id` = ? and `version` IN (%s)' USING FROM_BASE64('%s'), %s;"
    size_t num_comma = outputs_sz - 1;
    size_t param_markers_sz = num_comma + outputs_sz * 1;
    size_t param_val_sz = num_comma + outputs_sz * (APP_TRANSCODED_VERSION_SIZE + 2); // 2 extra charaters for quote
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(res_id_encoded) + param_markers_sz + param_val_sz;
    char raw_sql[raw_sql_sz];
    {
        const char *version = NULL;
        json_t *output = NULL;
        char param_markers[param_markers_sz + 1];
        char param_values[param_val_sz + 1];
        param_markers[param_markers_sz] = 0x0;
        param_values[param_val_sz]  = 0x0; 
        memset(&raw_sql[0], 0x0, raw_sql_sz);
        memset(&param_markers[0], ',', param_markers_sz);
        memset(&param_values[0],  ',', param_val_sz);
        for(int idx = 0; idx < param_markers_sz; param_markers[idx]='?', idx+=2);
        char *param_values_ptr = &param_values[0];
        json_object_foreach(outputs, version, output) {
            *param_values_ptr++ = '\'';
            memcpy(param_values_ptr, version, APP_TRANSCODED_VERSION_SIZE);
            param_values_ptr += APP_TRANSCODED_VERSION_SIZE;
            *param_values_ptr++ = '\'';
            param_values_ptr++; // comma
        }
        size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, &param_markers[0],
                res_id_encoded, &param_values[0]);
        assert(nwrite_sql <= (raw_sql_sz-1));
    }
#undef SQL_PATTERN
#define  NUM_USR_ARGS  3
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = _mark_old_transcoded_version__row_fetch,
            .result_free = _mark_old_transcoded_version__rs_free,
            .error = api__start_transcoding__db_async_err,
        }
    };
#undef NUM_USR_ARGS
    if(app_db_query_start(&cfg) != DBA_RESULT_OK)
        _api_start_transcode__deinit_primitives (req, self, node);
} // end of _mark_old_transcoded_version


static void _api_abac_pdp__try_match_rule (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t  *res_body   = usr_args[3];
    if(result->flag.error) {
        req->res.status = 503;
    } else if(result->data.size != 1 || !result->data.entries) {
        req->res.status = 403;
        json_object_set_new(res_body, "usr_id", json_string("missing access-control setup"));
    } else {
        aacl_data_t *d = &result->data.entries[0];
        if(!d->capability.transcode) {
            req->res.status = 403;
            json_object_set_new(res_body, "usr_id", json_string("operation denied"));
        }
    }
    if(req->res.status == 0) {
        app_run_next_middleware(hdlr, req, node);
    } else {
        _api_start_transcode__deinit_primitives (req, hdlr, node);
    }
} // end of  _api_abac_pdp__try_match_rule


static void _api_abac_pdp__verify_resource_owner (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t  *res_body   = usr_args[3];
    req->res.status = api_http_resp_status__verify_resource_id (result, res_body);
    if(json_object_size(res_body) == 0) {
        json_t *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
        json_object_set_new(_req_body, "usr_id", json_integer(result->owner_usr_id));
        json_object_set_new(_req_body, "last_upld_req", json_integer(result->upld_req));
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
        if(curr_usr_id == result->owner_usr_id) {
            app_run_next_middleware(hdlr, req, node);
        } else {
            char  *_res_id_encoded = (char *)app_fetch_from_hashmap(node->data, "res_id_encoded");
            void *usr_args[4] = {req, hdlr, node, res_body};
            aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=4}, .resource_id=_res_id_encoded,
                    .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                    .usr_id=curr_usr_id, .callback=_api_abac_pdp__try_match_rule };
            int err = app_resource_acl_load (&cfg);
            if(err)
                _api_start_transcode__deinit_primitives (req, hdlr, node);
        }
    } else {
        _api_start_transcode__deinit_primitives (req, hdlr, node);
    }
} // end of  _api_abac_pdp__verify_resource_owner


static int api_abac_pep__start_transcode (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node)
{
    json_error_t  j_err = {0};
    json_t *req_body = json_loadb((const char *)req->entity.base, req->entity.len, JSON_REJECT_DUPLICATES, &j_err);
    json_t *res_body = json_object();
    app_save_ptr_to_hashmap(node->data, "res_body_json", (void *)res_body);
    app_save_ptr_to_hashmap(node->data, "req_body_json", (void *)req_body);
    if(j_err.line >= 0 || j_err.column >= 0) {
        req->res.status = 400;
        json_object_set_new(res_body, "non-field", json_string("json parsing error on request body"));
    } else {
        const char *resource_id = json_string_value(json_object_get(req_body, "resource_id"));
        int err = app_verify_printable_string(resource_id, APP_RESOURCE_ID_SIZE);
        if(err) {
            req->res.status = 400;
            json_object_set_new(res_body, "res_id", json_string("contains non-printable charater"));
        } else {
            size_t out_len = 0;
            unsigned char *res_id_encoded = base64_encode((const unsigned char *)resource_id,
                    strlen(resource_id), &out_len);
            app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)res_id_encoded);
            void *usr_args[4] = {req, hdlr, node, res_body};
            aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=4}, .resource_id=(char *)res_id_encoded,
                    .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                    .callback=_api_abac_pdp__verify_resource_owner };
            int err = app_acl_verify_resource_id (&cfg);
            if(err)
                json_object_set_new(res_body, "reason", json_string("internal error"));
        }
    }
    if(json_object_size(res_body) > 0)
        _api_start_transcode__deinit_primitives (req, hdlr, node);
    return 0;
} // end of  api_abac_pep__start_transcode


#define VALIDATE_CODEC_LABEL_COMMON(codec_type) \
{ \
    const char *codec_name = json_string_value(json_object_get(elm, "codec")); \
    if(codec_name) { \
        uint8_t verified = 0; \
        aav_cfg_codec_t  *encoder = &acfg->transcoder.output.encoder; \
        for(idx = 0; (!verified) && (idx < encoder-> codec_type .size); idx++) { \
            AVCodec *codec = (AVCodec *)encoder-> codec_type .entries[idx]; \
            verified = strncmp(codec->name, codec_name, strlen(codec->name)) == 0; \
        } \
        if(!verified) \
            json_object_set_new(err, "codec", json_string("unknown label")); \
    } else { \
        json_object_set_new(err, "codec", json_string("required")); \
    } \
}

static void _validate_request__output_video(json_t *elm, json_t *res_body)
{ // TODO, improve err-info structure
    size_t idx = 0;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    VALIDATE_CODEC_LABEL_COMMON(video);
    json_t *attribute = json_object_get(elm, "attribute");
    int height_pixel = (int) json_integer_value(json_object_get(attribute, "height_pixel"));
    int width_pixel  = (int) json_integer_value(json_object_get(attribute, "width_pixel"));
    int framerate    = (int) json_integer_value(json_object_get(attribute, "framerate"));
    if(height_pixel <= 0)
        json_object_set_new(err, "height_pixel", json_string("has to be positive integer"));
    if(width_pixel <= 0)
        json_object_set_new(err, "width_pixel", json_string("has to be positive integer"));
    if(framerate <= 0)
        json_object_set_new(err, "framerate", json_string("has to be positive integer"));
    if(json_object_size(err) == 0) {
        aav_cfg_resolution_v_t  *rso_v  = &acfg->transcoder.output.resolution.video;
        uint8_t rso_accepted = 0;
        uint8_t fps_accepted = 0;
        for(idx = 0; (!rso_accepted) && (idx < rso_v->pixels.size); idx++) {
            aav_cfg_resolution_pix_t *pix = &rso_v->pixels.entries[idx];
            rso_accepted = (pix->width == width_pixel) && (pix->height == height_pixel);
        }
        for(idx = 0; (!fps_accepted) && (idx < rso_v->fps.size); idx++) {
            fps_accepted = (framerate == rso_v->fps.entries[idx]);
        }
        if(!rso_accepted)
            json_object_set_new(err, "height_pixel", json_string("invalid resolution"));
        if(!fps_accepted)
            json_object_set_new(err, "framerate", json_string("invalid framerate"));
    }
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(res_body, "elementary_streams", err);
    }
} // end of _validate_request__output_video

static void _validate_request__output_audio(json_t *elm, json_t *res_body)
{
    size_t idx = 0;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    VALIDATE_CODEC_LABEL_COMMON(audio);
    json_t *attribute = json_object_get(elm, "attribute");
    int bitrate_kbps = (int) json_integer_value(json_object_get(attribute, "bitrate_kbps"));
    if(bitrate_kbps <= 0)
        json_object_set_new(err, "bitrate_kbps", json_string("has to be positive integer"));
    if(json_object_size(err) == 0) {
        aav_cfg_resolution_a_t  *rso_a  = &acfg->transcoder.output.resolution.audio;
        uint8_t accepted = 0;
        for(idx = 0; (!accepted) && (idx < rso_a->bitrate_kbps.size); idx++) {
           accepted = bitrate_kbps == rso_a->bitrate_kbps.entries[idx];
        }
        if(!accepted)
            json_object_set_new(err, "bitrate_kbps", json_string("invalid bitrate"));
    }
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(res_body, "elementary_streams", err);
    }
} // end of _validate_request__output_audio


static void _validate_request__elementary_streams(json_t *elm_streams, json_t *res_body)
{
    const char *key = NULL;
    json_t *elm_entry = NULL;
    if(!elm_streams || !json_is_object(elm_streams) || json_object_size(elm_streams) == 0) {
        json_object_set_new(res_body, "elementary_streams", json_string("missing field"));
        return;
    }
    json_object_foreach(elm_streams, key, elm_entry) {
        const char *st_type = json_string_value(json_object_get(elm_entry, "type"));
        const char *err_msg = NULL, *err_field = NULL;
        if(!key) {
            err_field = "non-field";
            err_msg   = "missing key label for the entry";
        } else if(!st_type) {
            err_field = "type";
            err_msg   = "unkown stream type";
        } else if(!json_object_get(elm_entry, "attribute")) {
            err_field = "non-field";
            err_msg   = "missing attributes";
        } else if(strncmp(st_type,"video",5) == 0) {
            _validate_request__output_video(elm_entry, res_body);
        } else if(strncmp(st_type,"audio",5) == 0) {
            _validate_request__output_audio(elm_entry, res_body);
        } else {
            err_field = "type";
            err_msg   = "unsupported stream type";
        } // TODO, support subtitle and other types of streams
        if(err_msg && err_field) {
            json_t *err_info = json_object_get(res_body, "elementary_streams");
            if(!err_info) {
                err_info = json_object();
                json_object_set_new(res_body, "elementary_streams", err_info);
            }
            json_object_set_new(err_info, err_field, json_string(err_msg));
        }
        if(json_object_size(res_body) > 0)
            break;
    } // end of elementary-stream-entry loop
} // end of _validate_request__elementary_streams

static void _validate_request__outputs_elm_st_map(json_t *output, json_t *elm_st_dict, json_t *err)
{
    json_t *elm_st_keys = json_object_get(output, "elementary_streams");
    if(!json_is_array(elm_st_keys)) {
        json_object_set_new(err, "elementary_streams", json_string("unknown streams to mux"));
        return;
    } 
    int idx = 0;
    json_t *key_item = NULL;
    uint8_t audio_stream_included = 0, video_stream_included = 0;
    char *audio_stream_key = NULL, *video_stream_key = NULL;
    json_array_foreach(elm_st_keys, idx, key_item) {
        const char *key = json_string_value(key_item);
        json_t *elm_entry = json_object_get(elm_st_dict, key);
        if(!elm_entry) { continue; }
        const char *st_type = json_string_value(json_object_get(elm_entry, "type"));
        if(strncmp(st_type,"audio",5) == 0) {
            audio_stream_key = audio_stream_key ? audio_stream_key: strdup(key);
            audio_stream_included++;
        } else if(strncmp(st_type,"video",5) == 0) {
            video_stream_key = video_stream_key ? video_stream_key: strdup(key);
            video_stream_included++;
        }
    }
    if(audio_stream_included == 1 && video_stream_included == 1) {
        json_t *internal = json_object();
        json_object_set_new(internal, "audio_key", json_string(audio_stream_key));
        json_object_set_new(internal, "video_key", json_string(video_stream_key));
        json_object_set_new(output, "__internal__", internal);
    } else { // TODO, does the app have to support 2 audio/video streams in the same media container ?
        json_object_set_new(err, "elementary_streams",
                json_string("each output item should have exact one audio stream and exact one video stream to mux"));
    }
    if(audio_stream_key) { free(audio_stream_key); }
    if(video_stream_key) { free(video_stream_key); }
} // end of _validate_request__outputs_elm_st_map

static  void _validate_request__outputs(json_t *outputs, json_t *elm_streams, json_t *res_body)
{
    if(!outputs || !json_is_object(outputs) || json_object_size(outputs) == 0) {
        json_object_set_new(res_body, "outputs", json_string("missing field"));
        return;
    } // TODO, set limit on max number of transcoding requests
    int idx = 0;
    const char *version = NULL;
    json_t *output = NULL;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    json_object_foreach(outputs, version, output) {
        const char *container = json_string_value(json_object_get(output, "container"));
        if(strlen(version) == APP_TRANSCODED_VERSION_SIZE) {
            int err_ret = app_verify_printable_string(version, APP_TRANSCODED_VERSION_SIZE);
            if(err_ret) // TODO, accept only English letters
                json_object_set_new(err, "version", json_string("contains non-printable charater"));
        } else {
            json_object_set_new(err, "version", json_string("invalid length"));
        }
        uint8_t muxer_accepted = 0;
        for(idx = 0; (!muxer_accepted) && (idx < acfg->transcoder.output.muxers.size); idx++) {
            AVOutputFormat *muxer = (AVOutputFormat *) acfg->transcoder.output.muxers.entries[idx];
            muxer_accepted = strncmp(container, muxer->name, strlen(muxer->name)) == 0;
        }
        if(!muxer_accepted)
            json_object_set_new(err, "container", json_string("unknown muxer type"));
        _validate_request__outputs_elm_st_map(output, elm_streams, err);
        if(json_object_size(err) > 0)
            break;
    } // end of output-info iteration
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(res_body, "outputs", err);
    }
} // end of _validate_request__outputs


// TODO, abstract interface for image and video, current implementation is
//   closely tied to video transcoding
RESTAPI_ENDPOINT_HANDLER(start_transcoding_file, POST, self, req)
{
    json_t *req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t *res_body = (json_t *)app_fetch_from_hashmap(node->data, "res_body_json");
    _validate_request__elementary_streams(json_object_get(req_body, "elementary_streams"), res_body);
    if(json_object_size(res_body) == 0)
        _validate_request__outputs( json_object_get(req_body, "outputs"),
                json_object_get(req_body, "elementary_streams"), res_body );
    if(json_object_size(res_body) == 0) {
        _mark_old_transcoded_version(self, req, node);
    } else {
        req->res.status = 400;
        _api_start_transcode__deinit_primitives (req, self, node);
    }
    return 0;
} // end of start_transcoding_file
