#include "utils.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail)
{
    (void *)detail;
}


static void  app_validate_uncommitted_upld_req__row_fetch(db_query_t *target, db_query_result_t *rs)
{ // supposed to be invoked only once
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_active_reqs = (size_t) target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
    target->cfg.usr_data.entry[3] = (void *) num_active_reqs + 1;
} // end of app_validate_uncommitted_upld_req__row_fetch


static void  app_validate_uncommitted_upld_req__rs_free(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_active_reqs = (size_t) target->cfg.usr_data.entry[3];
#pragma GCC diagnostic pop
    if (rs->app_result == DBA_RESULT_OK) {
        // check quota limit, estimate all uploaded chunks of the user
        app_middleware_fn  cb = NULL;
        if(num_active_reqs == 0) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[5];
            cb(self, req, node);
        } else if(num_active_reqs == 1) {
            cb = (app_middleware_fn) target->cfg.usr_data.entry[4];
            cb(self, req, node);
        } else {
            target->cfg.callbacks.error(target, rs);
        }
    } else {
        target->cfg.callbacks.error(target, rs);
    }
} // end of app_validate_uncommitted_upld_req__rs_free


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
    {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        if(!jwt_claims) {
            return DBA_RESULT_ERROR_ARG;
        }
        usr_id = (int) json_integer_value(json_object_get(jwt_claims, "profile"));
        if(usr_id == 0) {
            return DBA_RESULT_ERROR_ARG;
        }
    }
#define SQL_PATTERN "SELECT `usr_id` FROM `%s` WHERE `usr_id` = %u AND `req_id` = x'%08x';"
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + strlen(db_table) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(req_seq);
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    size_t nwrite_sql = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, db_table, usr_id, req_seq);
    assert(nwrite_sql < raw_sql_sz);
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
            .row_fetched = app_validate_uncommitted_upld_req__row_fetch,
            .result_free = app_validate_uncommitted_upld_req__rs_free,
            .error = err_cb,
        }
    };
    return app_db_query_start(&cfg);
#undef NUM_USR_ARGS
#undef SQL_PATTERN
} // end of  app_validate_uncommitted_upld_req
