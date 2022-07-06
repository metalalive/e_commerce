#include <ctype.h>

#include "utils.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"
#include "rpc/datatypes.h"

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail)
{
    (void *)detail;
}


static void  app_validate_id_existence__row_fetch(db_query_t *target, db_query_result_t *rs)
{ // supposed to be invoked only once
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
    target->cfg.usr_data.entry[3] = (void *) num_rows_read + 1;
} // end of app_validate_id_existence__row_fetch

static void  app_validate_res_id_existence__row_fetch(db_query_t *target, db_query_result_t *rs)
{
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    if(row->values[0]) {
        uint32_t resource_owner_id = (uint32_t) strtoul(row->values[0], NULL, 10);
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
        target->cfg.usr_data.entry[6] = (void *) resource_owner_id;
#pragma GCC diagnostic pop
    }
    if(row->values[1]) {
        uint32_t last_upld_req = (uint32_t) strtoul(row->values[1], NULL, 16);
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
        target->cfg.usr_data.entry[7] = (void *) last_upld_req;
#pragma GCC diagnostic pop
    }
    app_validate_id_existence__row_fetch(target, rs);
} // end of app_validate_res_id_existence__row_fetch


static void  app_validate_id_existence__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *)     target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
    if (rs->app_result == DBA_RESULT_OK) {
        // check quota limit, estimate all uploaded chunks of the user
        app_middleware_fn  cb = NULL;
        if(num_rows_read == 0) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[5];
            cb(self, req, node);
        } else if(num_rows_read == 1) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[4];
            cb(self, req, node);
        } else {
            target->cfg.callbacks.error(target, rs);
        }
    } else {
        target->cfg.callbacks.error(target, rs);
    }
} // end of app_validate_id_existence__rs_free

static void  app_validate_res_id_existence__rs_free(db_query_t *target, db_query_result_t *rs)
{
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) target->cfg.usr_data.entry[3];
    uint32_t resource_owner_id = (uint32_t) target->cfg.usr_data.entry[6];
    uint32_t last_upld_req     = (uint32_t) target->cfg.usr_data.entry[7];
#pragma GCC diagnostic pop
    if(num_rows_read == 1) {
        app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
        app_save_int_to_hashmap(node->data, "last_upld_req", last_upld_req);
        app_save_int_to_hashmap(node->data, "resource_owner_id", resource_owner_id);
    }
    app_validate_id_existence__rs_free(target, rs);
} // end of app_validate_res_id_existence__rs_free


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
    {
        return DBA_RESULT_ERROR_ARG;
    }
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
#define  NUM_USR_ARGS  6
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node,
            (void *)0, (void *)success_cb, (void *)failure_cb };
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = app_validate_id_existence__row_fetch,
            .result_free = app_validate_id_existence__rs_free,
            .error = err_cb,
        }
    };
    return app_db_query_start(&cfg);
#undef NUM_USR_ARGS
} // end of  app_validate_uncommitted_upld_req


DBA_RES_CODE  app_verify_existence_resource_id (RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node,
        void (*err_cb)(db_query_t *, db_query_result_t *), app_middleware_fn success_cb, app_middleware_fn failure_cb)
{
    if(!self || !req || !node || !err_cb || !failure_cb || !success_cb)
    {
        return DBA_RESULT_ERROR_ARG;
    }
    char  *res_id_encoded = (char *)app_fetch_from_hashmap(node->data, "res_id_encoded");
    if(!res_id_encoded) {
        return DBA_RESULT_ERROR_ARG;
    }
#define SQL_PATTERN "EXECUTE IMMEDIATE 'SELECT `usr_id`, HEX(`last_upld_req`) FROM `uploaded_file` WHERE `id` = ?' USING FROM_BASE64('%s');"
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(res_id_encoded);
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, res_id_encoded);
    assert(nwrite_sql < raw_sql_sz);
#undef SQL_PATTERN
#define  NUM_USR_ARGS  8
    void *db_async_usr_data[NUM_USR_ARGS] = {(void *)req, (void *)self, (void *)node,
            (void *)0, (void *)success_cb, (void *)failure_cb, (void *)0, (void *)0 };
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = NUM_USR_ARGS},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = app_validate_res_id_existence__row_fetch,
            .result_free = app_validate_res_id_existence__rs_free,
            .error = err_cb,
        }
    };
    return app_db_query_start(&cfg);
#undef NUM_USR_ARGS
} // end of app_verify_existence_resource_id


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
    for(size_t idx = 0; idx < actual_sz; idx++) {
        int c = (int)str[idx];
        err = (isprint(c) == 0) || isspace(c);
        if(err) { break; }
    }
done:
    return err;
} // end of app_verify_printable_string

ARPC_STATUS_CODE api__render_rpc_reply_qname(
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    json_t *_usr_data = (json_t *)args->usr_data;
    uint32_t usr_prof_id = (uint32_t) json_integer_value(json_object_get(_usr_data,"usr_id"));
    if(usr_prof_id > 0) {
        snprintf(wr_buf, wr_sz, name_pattern, usr_prof_id);
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api__render_rpc_reply_qname

