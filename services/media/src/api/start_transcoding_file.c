#include <openssl/sha.h>

#include "utils.h"
#include "base64.h"
#include "app_cfg.h"
#include "api/setup.h"
#include "models/pool.h"
#include "models/query.h"
#include "rpc/core.h"
#include "storage/cfg_parser.h"
#include "transcoder/file_processor.h"

static void api__dealloc_req_hashmap(app_middleware_node_t *node) {
    char   *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t *_res_body = app_fetch_from_hashmap(node->data, "res_body_json");
    json_t *_req_body = app_fetch_from_hashmap(node->data, "req_body_json");
    if (res_id_encoded) {
        free(res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    if (_res_body) {
        json_decref(_res_body);
        app_save_ptr_to_hashmap(node->data, "res_body_json", (void *)NULL);
    }
    if (_req_body) {
        json_decref(_req_body);
        app_save_ptr_to_hashmap(node->data, "req_body_json", (void *)NULL);
    }
} // end of api__dealloc_req_hashmap

static void
_api_start_transcode__deinit_primitives(h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node) {
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json")
    );
    if (req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[api][transcode] line:%d \n", __LINE__);
    }
    // h2o_send_error_503(req, "server temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    json_t *_resp_body = app_fetch_from_hashmap(node->data, "res_body_json");
    size_t  nb_required = json_dumpb(_resp_body, NULL, 0, 0);
    if (nb_required > 0) {
        char   body[nb_required + 1];
        size_t nwrite = json_dumpb(_resp_body, &body[0], nb_required, JSON_COMPACT);
        body[nwrite++] = 0x0;
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, strlen(&body[0]));
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    api__dealloc_req_hashmap(node);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_start_transcode__deinit_primitives

static void _api_start_transcode__db_async_err(db_query_t *target, db_query_result_t *rs) {
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *self = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    req->res.status = 503;
    _api_start_transcode__deinit_primitives(req, self, node);
} // end of _api_start_transcode__db_async_err

ARPC_STATUS_CODE api__start_transcoding__render_rpc_corr_id(
    const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz
) {
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    size_t           md_hex_sz = (SHA_DIGEST_LENGTH << 1) + 1;
    size_t           tot_wr_sz = strlen(name_pattern) + md_hex_sz;
    if (tot_wr_sz > wr_sz)
        return APPRPC_RESP_MEMORY_ERROR;
    json_t  *_usr_data = (json_t *)args->usr_data;
    uint32_t usr_prof_id = (uint32_t)json_integer_value(json_object_get(_usr_data, "usr_id"));
    json_t  *outputs = json_object_get(_usr_data, "outputs");
    if (usr_prof_id > 0 && outputs) {
        SHA_CTX sha_ctx = {0};
        SHA1_Init(&sha_ctx);
        SHA1_Update(&sha_ctx, (const char *)&usr_prof_id, sizeof(usr_prof_id));
        SHA1_Update(&sha_ctx, (const char *)&args->_timestamp, sizeof(args->_timestamp));
        const char *version = NULL;
        json_t     *req_output = NULL;
        json_object_foreach(outputs, version, req_output) {
            SHA1_Update(&sha_ctx, version, APP_TRANSCODED_VERSION_SIZE);
        }
        char md[SHA_DIGEST_LENGTH] = {0};
        char md_hex[md_hex_sz];
        SHA1_Final((unsigned char *)&md[0], &sha_ctx);
        app_chararray_to_hexstr(&md_hex[0], md_hex_sz - 1, &md[0], SHA_DIGEST_LENGTH);
        md_hex[md_hex_sz - 1] = 0x0;
        size_t nwrite = snprintf(wr_buf, wr_sz, name_pattern, &md_hex[0]);
        if (nwrite >= wr_sz)
            status = APPRPC_RESP_MEMORY_ERROR;
        OPENSSL_cleanse(&sha_ctx, sizeof(SHA_CTX));
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api__start_transcoding__render_rpc_corr_id

static __attribute__((optimize("O0"))) void _api_start_transcode__send_async_job(
    db_query_t *target, db_query_result_t *rs
) { // create async job send it to message queue, since it takes time to transcode media file
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *self = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];

    char   *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t *_res_body = app_fetch_from_hashmap(node->data, "res_body_json");
    json_t *_req_body = app_fetch_from_hashmap(node->data, "req_body_json");
    json_t *req_outputs = json_object_get(_req_body, "outputs");
    json_t *elm_streams = json_object_get(_req_body, "elementary_streams");
    json_t *parts_size = json_object_get(_req_body, "parts_size");
    json_t *res_id_item = json_object_get(_req_body, "resource_id");
    json_t *usr_id_item = json_object_get(_req_body, "usr_id"); // resource owner, not current authorized user
    json_t *upld_req_item = json_object_get(_req_body, "last_upld_req");
    json_object_set(_res_body, "resource_id", res_id_item);
    // determine source and destination storage for RPC consumers, TODO, scalability
    asa_cfg_t *src_storage = app_storage_cfg_lookup("persist_usr_asset");
    asa_cfg_t *dst_storage = app_storage_cfg_lookup("persist_usr_asset");
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
        json_t     *req_output = NULL;
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
        char  *msg_body_raw = calloc(nb_required, sizeof(char));
        size_t nwrite = json_dumpb(msgq_body_item, msg_body_raw, nb_required, JSON_COMPACT);
        assert(nwrite <= nb_required);
        char           job_id_raw[MAX_BYTES_JOB_ID] = {0};
        arpc_exe_arg_t rpc_arg = {
            .conn = req->conn->ctx->storage.entries[1].data,
            .job_id = {.bytes = &job_id_raw[0], .len = MAX_BYTES_JOB_ID},
            .msg_body = {.len = nwrite, .bytes = msg_body_raw},
            .alias = "app_mqbroker_1",
            .routing_key = "rpc.media.transcode_video_file",
            .usr_data = (void *)msgq_body_item,
        }; // will start a new job and transcode asynchronously
        if (app_rpc_start(&rpc_arg) == APPRPC_RESP_ACCEPTED) {
            req->res.status = 202;
            json_object_set_new(_res_body, "job_id", json_string(&job_id_raw[0]));
        } else {
            req->res.status = 503;
            json_object_set_new(_res_body, "job_id", json_null());
        }
        free(msg_body_raw);
    } // end of construct message body
    json_decref(msgq_body_item);
    _api_start_transcode__deinit_primitives(req, self, node);
} // end of _api_start_transcode__send_async_job

static void _load_filechunk_info__row_fetch(db_query_t *target, db_query_result_t *rs) {
    db_query_row_info_t   *row = (db_query_row_info_t *)&rs->data[0];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];

    json_t  *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t  *parts_size = json_object_get(_req_body, "parts_size");
    uint32_t size_bytes = (uint32_t)strtoul(row->values[0], NULL, 10);
    json_array_append_new(parts_size, json_integer(size_bytes));
}

static void _api_transcode__load_filechunk_info(db_query_t *target, db_query_result_t *rs) {
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *self = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    json_t                *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t _resource_owner_id = (uint32_t)json_integer_value(json_object_get(_req_body, "usr_id"));
    uint32_t _last_upld_req = (uint32_t)json_integer_value(json_object_get(_req_body, "last_upld_req"));
#pragma GCC diagnostic pop
    // clang-format off
#define SQL_PATTERN \
    "SELECT  `size_bytes` FROM `upload_filechunk` WHERE `usr_id` = %u " \
    " AND `req_id` = x'%08x' ORDER BY `part` ASC;"
    // clang-format on
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_last_upld_req);
    char   raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, _resource_owner_id, _last_upld_req);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define NUM_USR_ARGS 3
    void          *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t cfg =
        {.statements = {.entry = &raw_sql[0], .num_rs = 1},
         .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
         .pool = app_db_pool_get_pool("db_server_1"),
         .loop = req->conn->ctx->loop,
         .callbacks = {
             .result_rdy = app_db_async_dummy_cb,
             .row_fetched = _load_filechunk_info__row_fetch,
             .result_free = _api_start_transcode__send_async_job,
             .error = _api_start_transcode__db_async_err,
         }};
#undef NUM_USR_ARGS
    if (app_db_query_start(&cfg) == DBA_RESULT_OK) {
        json_object_set_new(_req_body, "parts_size", json_array());
    } else {
        _api_start_transcode__deinit_primitives(req, self, node);
    }
} // end of _api_transcode__load_filechunk_info

static void _mark_old_transcoded_version__row_fetch(db_query_t *target, db_query_result_t *rs) {
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    json_t              *_req_body = (json_t *)target->cfg.usr_data.entry[3];
    const char          *resource_type = json_string_value(json_object_get(_req_body, "res_type"));
    atfp_validate_req_dup_version(resource_type, _req_body, row);
} // end of _mark_old_transcoded_version__row_fetch

static void _mark_old_transcoded_version(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) {
    char       *res_id_encoded = (char *)app_fetch_from_hashmap(node->data, "res_id_encoded");
    json_t     *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t     *outputs = json_object_get(_req_body, "outputs");
    size_t      outputs_sz = json_object_size(outputs), _sql_patt_sz = 0;
    const char *resource_type = json_string_value(json_object_get(_req_body, "res_type"));
    const char *_sql_patt = atfp_transcoded_version_sql_pattern(resource_type, &_sql_patt_sz);
    assert(_sql_patt);
    assert(_sql_patt_sz > 0);
    size_t num_comma = outputs_sz - 1;
    size_t param_markers_sz = num_comma + outputs_sz * 1;
    size_t param_val_sz =
        num_comma + outputs_sz * (APP_TRANSCODED_VERSION_SIZE + 2); // 2 extra charaters for quote
    size_t raw_sql_sz = _sql_patt_sz + strlen(res_id_encoded) + param_markers_sz + param_val_sz;
    char   raw_sql[raw_sql_sz];
    {
        const char *version = NULL;
        json_t     *output = NULL;
        char        param_markers[param_markers_sz + 1];
        char        param_values[param_val_sz + 1];
        param_markers[param_markers_sz] = 0x0;
        param_values[param_val_sz] = 0x0;
        memset(&raw_sql[0], 0x0, raw_sql_sz);
        memset(&param_markers[0], ',', param_markers_sz);
        memset(&param_values[0], ',', param_val_sz);
        for (int idx = 0; idx < param_markers_sz; param_markers[idx] = '?', idx += 2)
            ;
        char *param_values_ptr = &param_values[0];
        json_object_foreach(outputs, version, output) {
            *param_values_ptr++ = '\'';
            memcpy(param_values_ptr, version, APP_TRANSCODED_VERSION_SIZE);
            param_values_ptr += APP_TRANSCODED_VERSION_SIZE;
            *param_values_ptr++ = '\'';
            param_values_ptr++; // comma
        }
        size_t nwrite_sql =
            snprintf(&raw_sql[0], raw_sql_sz, _sql_patt, &param_markers[0], res_id_encoded, &param_values[0]);
        assert(nwrite_sql <= (raw_sql_sz - 1));
    }
#define NUM_USR_ARGS 4
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node, (void *)_req_body};
    db_query_cfg_t cfg =
        {.statements = {.entry = &raw_sql[0], .num_rs = 1},
         .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
         .pool = app_db_pool_get_pool("db_server_1"),
         .loop = req->conn->ctx->loop,
         .callbacks = {
             .result_rdy = app_db_async_dummy_cb,
             .row_fetched = _mark_old_transcoded_version__row_fetch,
             .result_free = _api_transcode__load_filechunk_info,
             .error = _api_start_transcode__db_async_err,
         }};
#undef NUM_USR_ARGS
    if (app_db_query_start(&cfg) != DBA_RESULT_OK)
        _api_start_transcode__deinit_primitives(req, self, node);
} // end of _mark_old_transcoded_version

static void _api_abac_pdp__try_match_rule(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *res_body = usr_args[3];
    if (result->flag.error) {
        req->res.status = 503;
    } else if (result->data.size != 1 || !result->data.entries) {
        req->res.status = 403;
        json_object_set_new(res_body, "usr_id", json_string("missing access-control setup"));
    } else {
        aacl_data_t *d = &result->data.entries[0];
        if (!d->capability.transcode) {
            req->res.status = 403;
            json_object_set_new(res_body, "usr_id", json_string("operation denied"));
        }
    }
    if (req->res.status == 0) {
        app_run_next_middleware(hdlr, req, node);
    } else {
        _api_start_transcode__deinit_primitives(req, hdlr, node);
    }
} // end of  _api_abac_pdp__try_match_rule

static void _api_abac_pdp__verify_resource_owner(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *res_body = usr_args[3];
    req->res.status = api_http_resp_status__verify_resource_id(result, res_body);
    if (json_object_size(res_body) == 0) {
        json_t *_req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
        json_object_set_new(_req_body, "usr_id", json_integer(result->owner_usr_id));
        json_object_set_new(_req_body, "last_upld_req", json_integer(result->upld_req));
        json_object_set_new(_req_body, "res_type", json_string(&result->type[0]));
        json_t  *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
        if (curr_usr_id == result->owner_usr_id) {
            app_run_next_middleware(hdlr, req, node);
        } else {
            char      *_res_id_encoded = (char *)app_fetch_from_hashmap(node->data, "res_id_encoded");
            void      *usr_args[4] = {req, hdlr, node, res_body};
            aacl_cfg_t cfg = {
                .usr_args = {.entries = &usr_args[0], .size = 4},
                .resource_id = _res_id_encoded,
                .db_pool = app_db_pool_get_pool("db_server_1"),
                .loop = req->conn->ctx->loop,
                .usr_id = curr_usr_id,
                .callback = _api_abac_pdp__try_match_rule
            };
            int err = app_resource_acl_load(&cfg);
            if (err)
                _api_start_transcode__deinit_primitives(req, hdlr, node);
        }
    } else {
        _api_start_transcode__deinit_primitives(req, hdlr, node);
    }
} // end of  _api_abac_pdp__verify_resource_owner

static int api_abac_pep__start_transcode(h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node) {
    json_error_t j_err = {0};
    json_t      *req_body =
        json_loadb((const char *)req->entity.base, req->entity.len, JSON_REJECT_DUPLICATES, &j_err);
    json_t *res_body = json_object();
    app_save_ptr_to_hashmap(node->data, "res_body_json", (void *)res_body);
    app_save_ptr_to_hashmap(node->data, "req_body_json", (void *)req_body);
    if (j_err.line >= 0 || j_err.column >= 0) {
        req->res.status = 400;
        json_object_set_new(res_body, "non-field", json_string("json parsing error on request body"));
    } else {
        const char *resource_id = json_string_value(json_object_get(req_body, "resource_id"));
        int         err = app_verify_printable_string(resource_id, APP_RESOURCE_ID_SIZE);
        if (err) {
            req->res.status = 400;
            json_object_set_new(
                res_body, API_QPARAM_LABEL__RESOURCE_ID, json_string("contains non-printable charater")
            );
        } else {
            size_t         out_len = 0;
            unsigned char *res_id_encoded =
                base64_encode((const unsigned char *)resource_id, strlen(resource_id), &out_len);
            app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)res_id_encoded);
            void      *usr_args[4] = {req, hdlr, node, res_body};
            aacl_cfg_t cfg = {
                .usr_args = {.entries = &usr_args[0], .size = 4},
                .resource_id = (char *)res_id_encoded,
                .db_pool = app_db_pool_get_pool("db_server_1"),
                .loop = req->conn->ctx->loop,
                .callback = _api_abac_pdp__verify_resource_owner
            };
            int err = app_acl_verify_resource_id(&cfg);
            if (err)
                json_object_set_new(res_body, "reason", json_string("internal error"));
        }
    }
    if (json_object_size(res_body) > 0)
        _api_start_transcode__deinit_primitives(req, hdlr, node);
    return 0;
} // end of  api_abac_pep__start_transcode

RESTAPI_ENDPOINT_HANDLER(start_transcoding_file, POST, self, req) {
    json_t     *req_body = (json_t *)app_fetch_from_hashmap(node->data, "req_body_json");
    json_t     *res_body = (json_t *)app_fetch_from_hashmap(node->data, "res_body_json");
    const char *resource_type = json_string_value(json_object_get(req_body, "res_type"));
    int         err = atfp_validate_transcode_request(resource_type, req_body, res_body);
    if (!err) {
        _mark_old_transcoded_version(self, req, node);
    } else {
        req->res.status = 400;
        _api_start_transcode__deinit_primitives(req, self, node);
    }
    return 0;
} // end of start_transcoding_file
