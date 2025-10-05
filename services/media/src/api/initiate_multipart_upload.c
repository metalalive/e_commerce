#include "utils.h"
#include "api/setup.h"
#include "models/pool.h"
#include "models/query.h"

#define MYSQL_BINARY_HEX_SIZE(x)       (x << 1)
#define MAX_NUM_ACTIVE_UPLOAD_REQUESTS 3

#define SQL_RESULT_CODE__OK             0
#define SQL_RESULT_CODE__LIMIT_EXCEEDED 1
#define SQL_RESULT_CODE__DB_ERROR       2
// clang-format off
// TODO, make it portable to other databases, current PL/SQL works only for MariaDB
#define SQL_PATTERN \
    "BEGIN NOT ATOMIC" \
    "  DECLARE result_code  TINYINT(2) UNSIGNED DEFAULT %u;" \
    "  DECLARE max_num_active_req  INT UNSIGNED DEFAULT %u;" \
    "  DECLARE num_active_req      INT UNSIGNED DEFAULT  0;" \
    "  DECLARE EXIT HANDLER FOR SQLSTATE '23000' BEGIN" \
    "    ROLLBACK;" \
    "    SET result_code = %u;" \
    "    SELECT result_code, num_active_req;" \
    "  END;" \
    "  START TRANSACTION;" \
    "  SELECT COUNT(`req_id`) INTO num_active_req FROM `upload_request` AS L WHERE L.`usr_id` = %u FOR UPDATE;" \
    "  IF num_active_req < max_num_active_req THEN " \
    "    INSERT INTO `upload_request`(`usr_id`,`req_id`,`time_created`) VALUES (%u, x'%08x', '%s');" \
    "    COMMIT;" \
    "  ELSE" \
    "    ROLLBACK;" \
    "    SET result_code = %u;" \
    "  END IF;" \
    "  SELECT result_code, num_active_req;" \
    "END;"

// clang-format on

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail);

static DBA_RES_CODE initiate_multipart_upload__try_add_new_request(
    h2o_handler_t *self, h2o_req_t *req, app_middleware_node_t *node
);

static void initiate_multipart_upload__db_async_err(db_query_t *target, db_query_result_t *detail) {
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *self = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    fprintf(
        stderr, "[api][init-multi-upld-req] line:%d, app-result:%d, conn-state:%d, async:%d \n", __LINE__,
        detail->app_result, detail->conn.state, detail->conn.async
    );
    h2o_send_error_503(req, "server temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    app_run_next_middleware(self, req, node);
}

static void initiate_multipart_upload__fetch_row(
    db_query_t *target, db_query_result_t *detail
) { // should be called only once whenever new client request comes
    db_query_row_info_t   *row = (db_query_row_info_t *)&detail->data[0];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    char                  *ret_code_str = row->values[0];
    char                  *n_active_req_str = row->values[1];
#if 0
    fprintf(stderr, "[api][init-multi-upld-req] line:%d, app-result:%d, conn-state:%d \
           return-code:%s, num-active-reqs:%s  \n", __LINE__, detail->app_result,
           detail->conn.state, ret_code_str, n_active_req_str
        );
#endif
    if (ret_code_str) {
        int db_result = (int)strtol(ret_code_str, NULL, 10);
        // I'm sure the value will be treated as scalar integer instead of pointer to some type
        // once it is read from hash ENTRY structure, so tell the compiler to suppress the
        // warning of type-casting
        app_save_int_to_hashmap(node->data, "db_result", db_result);
    }
    if (n_active_req_str) {
        int num_active_upld_req = (int)strtol(n_active_req_str, NULL, 10);
        app_save_int_to_hashmap(node->data, "num_active_upld_req", num_active_upld_req);
    }
} // end of initiate_multipart_upload__fetch_row

static void initiate_multipart_upload__finalize_response(
    h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node
) {
    json_t *res_body = json_object();
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    int      db_result = (int)app_fetch_from_hashmap(node->data, "db_result");
    uint32_t req_seq = (uint32_t)app_fetch_from_hashmap(node->data, "upld_req_seq");
#pragma GCC diagnostic pop
    json_t  *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t usr_prof_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
    json_object_set_new(res_body, "usr_id", json_integer(usr_prof_id));
    switch (db_result) {
    case SQL_RESULT_CODE__DB_ERROR:
        fprintf(
            stderr, "[api][init-multi-upld-req] line:%d, usr-prof:%d, req-seq:%x \n", __LINE__, usr_prof_id,
            req_seq
        );
        json_object_set_new(res_body, "detail", json_string("internal error"));
        req->res.status = 503;
        break;
    case SQL_RESULT_CODE__OK:
        json_object_set_new(res_body, "req_seq", json_integer(req_seq));
        req->res.status = 201;
        break;
    case SQL_RESULT_CODE__LIMIT_EXCEEDED:
    default: {
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        int num_req = (int)app_fetch_from_hashmap(node->data, "num_active_upld_req");
#pragma GCC diagnostic pop
        json_object_set_new(res_body, "num_active", json_integer(num_req));
        json_object_set_new(res_body, "max_limit", json_integer(MAX_NUM_ACTIVE_UPLOAD_REQUESTS));
        req->res.status = 400;
        break;
    }
    } // end of switch statement
#define MAX_BYTES_RESP_BODY 64
    char   body_raw[MAX_BYTES_RESP_BODY] = {0};
    size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0], MAX_BYTES_RESP_BODY, JSON_COMPACT);
    h2o_iovec_t body = h2o_strdup(&req->pool, &body_raw[0], nwrite);
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json")
    );

    req->res.content_length = body.len;
    req->res.reason = "";

    h2o_generator_t generator = {0};
    h2o_start_response(req, &generator);
    h2o_send(req, &body, 1, H2O_SEND_STATE_FINAL);
    json_decref(res_body);
    app_run_next_middleware(hdlr, req, node);
#undef MAX_BYTES_RESP_BODY
} // end of initiate_multipart_upload__finalize_response

static void initiate_multipart_upload__result_set_ready(db_query_t *target, db_query_result_t *detail) {
    if (!detail->_final) {
        return;
    }
    h2o_req_t             *req = (h2o_req_t *)target->cfg.usr_data.entry[0];
    h2o_handler_t         *self = (h2o_handler_t *)target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *)target->cfg.usr_data.entry[2];
    initiate_multipart_upload__finalize_response(self, req, node);
} // end of initiate_multipart_upload__result_set_ready

static DBA_RES_CODE initiate_multipart_upload__try_add_new_request(
    h2o_handler_t *self, h2o_req_t *req, app_middleware_node_t *node
) {
    uint32_t usr_prof_id = 0;
    char     curr_time_str[DATETIME_STR_SIZE] = {0};
    uint32_t rand_req_seq[1] = {0};
    size_t   raw_sql_sz = sizeof(SQL_PATTERN) + MYSQL_BINARY_HEX_SIZE(sizeof(rand_req_seq)) +
                        (USR_ID_STR_SIZE * 2) + (DATETIME_STR_SIZE - 1);
    char raw_sql[raw_sql_sz];
    {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        usr_prof_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
#define RND_STATE_SIZE 18
        char               rnd_state[RND_STATE_SIZE] = {0};
        struct random_data rnd_buf = {0};
        unsigned int       seed = (unsigned int)time(NULL);
        initstate_r(seed, &rnd_state[0], RND_STATE_SIZE, &rnd_buf);
        random_r(&rnd_buf, (int32_t *)&rand_req_seq[0]);
#undef RND_STATE_SIZE
        time_t     now_time = time(NULL);
        struct tm *brokendown = localtime(&now_time);
        strftime(&curr_time_str[0], DATETIME_STR_SIZE, "%F %T", brokendown); // ISO8601 date format
    }
    { // record upload request ID, will be delivered with response
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
        ENTRY e = {.key = "upld_req_seq", .data = (void *)(rand_req_seq[0])};
#pragma GCC diagnostic pop
        ENTRY *e_ret = NULL;
        hsearch_r(e, ENTER, &e_ret, node->data);
    }
    memset(&raw_sql[0], 0x0, sizeof(char) * raw_sql_sz);
    sprintf(
        &raw_sql[0], SQL_PATTERN, SQL_RESULT_CODE__OK, MAX_NUM_ACTIVE_UPLOAD_REQUESTS,
        SQL_RESULT_CODE__DB_ERROR, usr_prof_id, usr_prof_id, rand_req_seq[0], &curr_time_str[0],
        SQL_RESULT_CODE__LIMIT_EXCEEDED
    );

    void          *db_async_usr_data[3] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t cfg =
        {.statements = {.entry = &raw_sql[0], .num_rs = 2},
         .usr_data = {.entry = (void **)&db_async_usr_data, .len = 3},
         .pool = app_db_pool_get_pool("db_server_1"),
         .loop = req->conn->ctx->loop,
         .callbacks = {
             .result_rdy = initiate_multipart_upload__result_set_ready,
             .row_fetched = initiate_multipart_upload__fetch_row,
             .result_free = app_db_async_dummy_cb,
             .error = initiate_multipart_upload__db_async_err,
         }};
    return app_db_query_start(&cfg);
} // end of initiate_multipart_upload__try_add_new_request

RESTAPI_ENDPOINT_HANDLER(initiate_multipart_upload, POST, self, req) {
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    if (!jwt_claims) {
        h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
        goto error;
    }
    if (json_integer_value(json_object_get(jwt_claims, "profile")) == 0) {
        h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
        goto error;
    }
    DBA_RES_CODE result = initiate_multipart_upload__try_add_new_request(self, req, node);
    if (result == DBA_RESULT_OK) {
        goto done;
    } else if (result == DBA_RESULT_POOL_BUSY || result == DBA_RESULT_CONNECTION_BUSY) {
        h2o_send_error_503(req, "temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    } else {
        h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
    }
error:
    app_run_next_middleware(self, req, node);
done:
    return 0;
} // end of initiate_multipart_upload
