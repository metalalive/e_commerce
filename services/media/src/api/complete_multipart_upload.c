#include "utils.h"
#include "base64.h"
#include "api/setup.h"
#include "models/pool.h"
#include "models/query.h"

static void _api_complete_multipart_upload__deinit_primitives(
    h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node, json_t *spec, json_t *resp_body
) {
    size_t nb_required = json_dumpb(resp_body, NULL, 0, 0);
    char   body_raw[nb_required + 1];
    size_t nwrite = json_dumpb(resp_body, &body_raw[0], nb_required, JSON_COMPACT);
    assert(nwrite < nb_required);
    body_raw[nwrite] = 0;
    if (req->res.status == 0)
        req->res.status = 500;
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json")
    );
    h2o_send_inline(req, body_raw, nwrite);
    if (spec)
        json_decref(spec);
    if (resp_body)
        json_decref(resp_body);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_complete_multipart_upload__deinit_primitives

static void api__complete_multipart_upload__db_async_err(db_query_t *target, db_query_result_t *rs) {
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *hdlr = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    json_t                *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t                *spec = app_fetch_from_hashmap(node->data, "spec");
    json_object_set_new(err_info, "internal", json_string("temporarily unavailable"));
    req->res.status = 503;
    _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
} // end of api__complete_multipart_upload__db_async_err

static void api__complete_multipart_upload__db_write_done(db_query_t *target, db_query_result_t *rs) {
    assert(rs->_final);
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *hdlr = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    json_t                *resp_body = app_fetch_from_hashmap(node->data, "err_info");
    json_t                *spec = app_fetch_from_hashmap(node->data, "spec");
    json_t                *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t               curr_usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
    uint32_t               curr_req_seq = (uint32_t)json_integer_value(json_object_get(spec, "req_seq"));
    const char *resource_id = json_string_value(json_object_get(spec, API_QPARAM_LABEL__RESOURCE_ID));
    json_object_set_new(resp_body, API_QPARAM_LABEL__RESOURCE_ID, json_string(resource_id));
    json_object_set_new(resp_body, "req_seq", json_integer(curr_req_seq));
    json_object_set_new(resp_body, "usr_id", json_integer(curr_usr_id));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    req->res.status = (uint32_t)target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
    _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, resp_body);
} // end of api__complete_multipart_upload__db_write_done

#define DO_FINAL_WRITE_COMMON_CODE(http_resp_code, ...) \
    { \
        time_t     now_time = time(NULL); \
        struct tm *brokendown = localtime(&now_time); \
        strftime(&curr_time_str[0], DATETIME_STR_SIZE, "%F %T", brokendown); \
        size_t nwrite = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, __VA_ARGS__); \
        raw_sql[nwrite] = 0; \
        assert(nwrite < raw_sql_sz); \
        void *db_async_usr_data[4] = {(void *)req, (void *)hdlr, (void *)node, (void *)http_resp_code}; \
        db_query_cfg_t cfg = \
            {.statements = {.entry = &raw_sql[0], .num_rs = 1}, \
             .usr_data = {.entry = (void **)&db_async_usr_data, .len = 4}, \
             .pool = app_db_pool_get_pool("db_server_1"), \
             .loop = req->conn->ctx->loop, \
             .callbacks = { \
                 .result_rdy = api__complete_multipart_upload__db_write_done, \
                 .error = api__complete_multipart_upload__db_async_err, \
                 .row_fetched = app_db_async_dummy_cb, \
                 .result_free = app_db_async_dummy_cb, \
             }}; \
        if (app_db_query_start(&cfg) != DBA_RESULT_OK) \
            _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info); \
    }

// clang-format off
#define PREP_STMT__UPLDREQ__TIME_UPDATE \
    "UPDATE `upload_request` SET `time_committed`=? WHERE `req_id`=? AND `usr_id`=?"
#define PREP_STMT__UPLOADED_FILE__UPDATE \
    "UPDATE `uploaded_file` SET `usr_id`=?,`last_upld_req`=?,`type`=?,`last_update`=?  WHERE " \
    "`id`=?"
#define PREP_STMT__UPLOADED_FILE__INSERT \
    "INSERT INTO `uploaded_file`(`usr_id`,`last_upld_req`,`type`,`last_update`,`id`) VALUES " \
    "(?,?,?,?,?)"
// clang-format on

#define PREP_FN_NAME__UPLDREQ_TIME_UPDATE "app_media_upldreq_time_update"
#define PREP_FN_NAME__UPLD_FILE__UPDATE   "app_media_commit_upldreq_update"
#define PREP_FN_NAME__UPLD_FILE__INSERT   "app_media_commit_upldreq_insert"

static int api__complete_upload__resource_id_exist(
    RESTAPI_HANDLER_ARGS(hdlr, req), app_middleware_node_t *node, json_t *spec, json_t *err_info,
    uint32_t last_req_seq, uint32_t resource_owner_id
) {
    // clang-format off
#define SQL_PATTERN \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `" PREP_FN_NAME__UPLDREQ_TIME_UPDATE "` FROM '" PREP_STMT__UPLDREQ__TIME_UPDATE "';" \
    "  PREPARE `" PREP_FN_NAME__UPLD_FILE__UPDATE "` FROM '" PREP_STMT__UPLOADED_FILE__UPDATE "';" \
    "  START TRANSACTION;" \
    "    EXECUTE `" PREP_FN_NAME__UPLDREQ_TIME_UPDATE "` USING NULL,x'%08x',%u;" \
    "    EXECUTE `" PREP_FN_NAME__UPLD_FILE__UPDATE \
    "` USING %u,x'%08x','%s','%s',FROM_BASE64('%s');" \
    "    EXECUTE `" PREP_FN_NAME__UPLDREQ_TIME_UPDATE "` USING '%s',x'%08x',%u;" \
    "  COMMIT;" \
    "END;"
    // clang-format on
    json_t  *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t curr_usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
    if (curr_usr_id == resource_owner_id || resource_owner_id == 0) {
        uint32_t    curr_req_seq = (uint32_t)json_integer_value(json_object_get(spec, "req_seq"));
        const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
        const char *_res_typ = json_string_value(json_object_get(spec, "type"));
        size_t      raw_sql_sz = sizeof(SQL_PATTERN) + strlen(_res_id_encoded) + USR_ID_STR_SIZE * 3 +
                            (DATETIME_STR_SIZE - 1) * 2 + UPLOAD_INT2HEX_SIZE(curr_req_seq) * 3 +
                            strlen(_res_typ);
        char raw_sql[raw_sql_sz], curr_time_str[DATETIME_STR_SIZE] = {0}; // ISO8601 date format
        DO_FINAL_WRITE_COMMON_CODE(
            200, last_req_seq, resource_owner_id, curr_usr_id, curr_req_seq, _res_typ, &curr_time_str[0],
            _res_id_encoded, &curr_time_str[0], curr_req_seq, curr_usr_id
        )
    } else {
        req->res.status = 403;
        json_object_set_new(
            err_info, API_QPARAM_LABEL__RESOURCE_ID, json_string("NOT allowed to use the ID")
        );
        _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    }
    return 0;
#undef SQL_PATTERN
} // end of api__complete_upload__resource_id_exist

static int api__complete_upload__resource_id_notexist(
    RESTAPI_HANDLER_ARGS(hdlr, req), app_middleware_node_t *node, json_t *spec, json_t *err_info
) {
    // clang-format off
#define SQL_PATTERN \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `" PREP_FN_NAME__UPLDREQ_TIME_UPDATE "` FROM '" PREP_STMT__UPLDREQ__TIME_UPDATE "';" \
    "  PREPARE `" PREP_FN_NAME__UPLD_FILE__INSERT "` FROM '" PREP_STMT__UPLOADED_FILE__INSERT "';" \
    "  START TRANSACTION;" \
    "    EXECUTE `" PREP_FN_NAME__UPLD_FILE__INSERT \
    "` USING %u,x'%08x','%s','%s',FROM_BASE64('%s');" \
    "    EXECUTE `" PREP_FN_NAME__UPLDREQ_TIME_UPDATE "` USING '%s',x'%08x',%u;" \
    "  COMMIT;" \
    "END;"
    // clang-format on
    json_t     *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t    curr_usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
    uint32_t    curr_req_seq = (uint32_t)json_integer_value(json_object_get(spec, "req_seq"));
    const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
    const char *_res_typ = json_string_value(json_object_get(spec, "type"));
    size_t      raw_sql_sz = sizeof(SQL_PATTERN) + strlen(_res_id_encoded) + USR_ID_STR_SIZE * 2 +
                        (DATETIME_STR_SIZE - 1) * 2 + UPLOAD_INT2HEX_SIZE(curr_req_seq) * 2 +
                        strlen(_res_typ);
    char raw_sql[raw_sql_sz], curr_time_str[DATETIME_STR_SIZE] = {0};
    DO_FINAL_WRITE_COMMON_CODE(
        201, curr_usr_id, curr_req_seq, _res_typ, &curr_time_str[0], _res_id_encoded, &curr_time_str[0],
        curr_req_seq, curr_usr_id
    )
    return 0;
#undef SQL_PATTERN
} // end of api__complete_upload__resource_id_notexist

static void _api_complete_upload__check_resource_id_done(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t                *spec = app_fetch_from_hashmap(node->data, "spec");
    if (result->flag.error) {
        _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    } else if (result->flag.res_id_exists) {
        api__complete_upload__resource_id_exist(
            hdlr, req, node, spec, err_info, result->upld_req, result->owner_usr_id
        );
    } else {
        api__complete_upload__resource_id_notexist(hdlr, req, node, spec, err_info);
    }
} // end of  _api_complete_upload__check_resource_id_done

static void
api__complete_multipart_upload__validate_filechunks__rs_free(db_query_t *target, db_query_result_t *rs) {
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *hdlr = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    json_t                *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t                *spec = app_fetch_from_hashmap(node->data, "spec");
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint32_t parts_max = (uint32_t)target->cfg.usr_data.entry[3];
    uint32_t parts_min = (uint32_t)target->cfg.usr_data.entry[4];
    uint32_t parts_cnt = (uint32_t)target->cfg.usr_data.entry[5];
#pragma GCC diagnostic pop
    uint8_t err =
        (parts_max == 0 || parts_min == 0 || parts_cnt == 0) || (parts_min != 1) || (parts_max != parts_cnt);
    if (err) {
        req->res.status = 400;
        json_object_set_new(err_info, "req_seq", json_string("part numbers of file chunks are not adjacent"));
        _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    } else {
        size_t         out_len = 0;
        const char    *resource_id = json_string_value(json_object_get(spec, API_QPARAM_LABEL__RESOURCE_ID));
        unsigned char *__res_id_encoded =
            base64_encode((const unsigned char *)resource_id, strlen(resource_id), &out_len);
        json_object_set_new(spec, "res_id_encoded", json_string((char *)__res_id_encoded));
        free(__res_id_encoded);
        const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
        void       *usr_args[3] = {req, hdlr, node};
        aacl_cfg_t  cfg = {
             .usr_args = {.entries = &usr_args[0], .size = 3},
             .resource_id = (char *)_res_id_encoded,
             .db_pool = app_db_pool_get_pool("db_server_1"),
             .loop = req->conn->ctx->loop,
             .callback = _api_complete_upload__check_resource_id_done
        };
        err = app_acl_verify_resource_id(&cfg);
        if (err)
            _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    }
} // end of api__complete_multipart_upload__validate_filechunks__rs_free

static void
api__complete_multipart_upload__validate_filechunks__row_fetch(db_query_t *target, db_query_result_t *rs) {
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];

    uint32_t parts_max = (uint32_t)strtoul(row->values[0], NULL, 10);
    uint32_t parts_min = (uint32_t)strtoul(row->values[1], NULL, 10);
    uint32_t parts_cnt = (uint32_t)strtoul(row->values[2], NULL, 10);
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
    target->cfg.usr_data.entry[3] = (void *)parts_max;
    target->cfg.usr_data.entry[4] = (void *)parts_min;
    target->cfg.usr_data.entry[5] = (void *)parts_cnt;
#pragma GCC diagnostic pop
} // end of api__complete_multipart_upload__validate_filechunks__row_fetch

static int api__complete_multipart_upload__validate_filechunks(
    RESTAPI_HANDLER_ARGS(hdlr, req), app_middleware_node_t *node
) {
    json_t  *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    int req_seq = (int)app_fetch_from_hashmap(node->data, "req_seq");
#pragma GCC diagnostic pop
    // clang-format off
#define SQL_PATTERN \
    "SELECT MAX(`part`), MIN(`part`), COUNT(`part`) FROM `upload_filechunk` " \
    " WHERE `usr_id` = %u AND `req_id` = x'%08x' GROUP BY `req_id`;"
    // clang-format on
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(req_seq);
    char   raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, usr_id, req_seq);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define NUM_USR_ARGS 6
    void *usr_data[NUM_USR_ARGS] = {(void *)req, (void *)hdlr, (void *)node, (void *)0, (void *)0, (void *)0};
    db_query_cfg_t cfg =
        {.statements = {.entry = raw_sql, .num_rs = 1},
         .usr_data = {.entry = (void **)&usr_data, .len = NUM_USR_ARGS},
         .pool = app_db_pool_get_pool("db_server_1"),
         .loop = req->conn->ctx->loop,
         .callbacks = {
             .result_rdy = app_db_async_dummy_cb,
             .row_fetched = api__complete_multipart_upload__validate_filechunks__row_fetch,
             .result_free = api__complete_multipart_upload__validate_filechunks__rs_free,
             .error = api__complete_multipart_upload__db_async_err,
         }};
#undef NUM_USR_ARGS
    if (app_db_query_start(&cfg) != DBA_RESULT_OK) {
        json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
        json_t *spec = app_fetch_from_hashmap(node->data, "spec");
        _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    }
    return 0;
} // end of api__complete_multipart_upload__validate_filechunks

static int api__complete_multipart_upload__validate_reqseq_failure(
    RESTAPI_HANDLER_ARGS(hdlr, req), app_middleware_node_t *node
) {
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec = app_fetch_from_hashmap(node->data, "spec");
    json_object_set_new(err_info, "req_seq", json_string("not exists"));
    req->res.status = 400;
    _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    return 0;
}

static int app_validate_resource_type(const char *type) {
    int ret = strncmp(type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    int valid = ret == 0;
    if (!valid) {
        ret = strncmp(type, APP_FILETYPE_LABEL_IMAGE, sizeof(APP_FILETYPE_LABEL_IMAGE) - 1);
        valid = ret == 0;
    }
    return valid;
} // end of  app_validate_resource_type

// TODO:another API endpoint for checking status of each upload request that hasn't expired yet
RESTAPI_ENDPOINT_HANDLER(complete_multipart_upload, PATCH, hdlr, req) {
    json_error_t j_err = {0};
    json_t      *err_info = json_object();
    json_t      *spec =
        json_loadb((const char *)req->entity.base, req->entity.len, JSON_REJECT_DUPLICATES, &j_err);
    uint32_t req_seq = 0;
    if (j_err.line >= 0 || j_err.column >= 0) {
        json_object_set_new(err_info, "message", json_string("parsing error on request body"));
    } else {
        const char *resource_id = json_string_value(json_object_get(spec, API_QPARAM_LABEL__RESOURCE_ID));
        const char *resource_typ = json_string_value(json_object_get(spec, "type"));
        req_seq = (uint32_t)json_integer_value(json_object_get(spec, "req_seq"));
        if (resource_id) {
            int err = app_verify_printable_string(resource_id, APP_RESOURCE_ID_SIZE);
            if (err) // TODO, consider invalid characters in SQL string literal for each specific
                     // database
                json_object_set_new(err_info, API_QPARAM_LABEL__RESOURCE_ID, json_string("invalid format"));
        } else {
            json_object_set_new(err_info, API_QPARAM_LABEL__RESOURCE_ID, json_string("missing"));
        }
        if (req_seq == 0)
            json_object_set_new(err_info, "req_seq", json_string("missing"));
        if (!resource_typ || !app_validate_resource_type(resource_typ))
            json_object_set_new(err_info, "type", json_string("invalid"));
    }
    if (json_object_size(err_info) > 0) {
        req->res.status = 400;
        req->res.reason = "invalid request";
        _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
    } else {
        app_save_int_to_hashmap(node->data, "req_seq", req_seq);
        DBA_RES_CODE db_result = app_validate_uncommitted_upld_req(
            hdlr, req, node, "upload_request", api__complete_multipart_upload__db_async_err,
            api__complete_multipart_upload__validate_filechunks, // if success
            api__complete_multipart_upload__validate_reqseq_failure
        );
        if (db_result == DBA_RESULT_OK) {
            app_save_ptr_to_hashmap(node->data, "spec", (void *)spec);
            app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
        } else {
            _api_complete_multipart_upload__deinit_primitives(req, hdlr, node, spec, err_info);
        }
    }
    return 0;
} // end of complete_multipart_upload
