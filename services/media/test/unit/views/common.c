#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>

#include "middleware.h"
#include "models/pool.h"
#include "models/query.h"
#include "models/connection.h"
#include "api/setup.h"

#define UTEST_DBPOOL_ALIAS "db_server_1"
#define UTEST_DB_ASYNC_FETCH_ROW_READY      1
#define UTEST_DB_ASYNC_FREE_RESULTSET_DONE  2

static DBA_RES_CODE mock_dbpool_3pty_init_fn(db_conn_t *conn)
{ return DBA_RESULT_OK; }

static DBA_RES_CODE mock_dbpool_3pty_deinit_fn(db_conn_t *conn)
{ return DBA_RESULT_OK; }

static uint8_t mock_dbpool__can_change_state(db_conn_t *conn)
{ return 1; }

static void  mock_dbpool_3pty_state_transition (app_timer_poll_t *target, int uv_status, int event_flags)
{
    uint8_t  has_row = 0;
    uint8_t *has_row_ptr = &has_row;
    size_t   num_cols = 0;
    size_t  *num_cols_ptr = &num_cols;
    mock(has_row_ptr, num_cols_ptr);
    db_conn_t *conn = H2O_STRUCT_FROM_MEMBER(db_conn_t, timer_poll, target);
    { conn->ops.update_ready_queries(conn); }
    db_query_t *curr_query = (db_query_t *) &conn->processing_queries->data;
    uv_run(curr_query->cfg.loop, UV_RUN_NOWAIT);
    if(has_row) {
        size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t) +
            sizeof(db_query_row_info_t) + sizeof(char *) * num_cols;
        db_llnode_t *rs_node = malloc(rs_node_sz);
        db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
        *rs = (db_query_result_t) {.free_data_cb = free, .app_result = DBA_RESULT_OK,
                .conn = {.state=UTEST_DB_ASYNC_FETCH_ROW_READY, .async=0,
                   .alias = conn->pool->cfg.alias}, ._final = 0};
        {
            db_query_row_info_t *cloned_row = (db_query_row_info_t *) &rs->data[0];
            cloned_row->num_cols = num_cols;
            cloned_row->values = (char **) &cloned_row->data[0];
            for(size_t idx = 0; idx < num_cols; idx++) {
                char **col_val_p = &cloned_row->values[idx];
                mock(col_val_p);
            }
        } // end of cloning a row
        app_db_query_notify_with_result(curr_query, rs);
    }
    app_db_conn_try_evict_current_processing_query(conn);
    uv_run(curr_query->cfg.loop, UV_RUN_NOWAIT);
    {
        size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t);
        db_llnode_t *rs_node = malloc(rs_node_sz);
        db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
        *rs = (db_query_result_t) {.free_data_cb = free, .app_result = DBA_RESULT_OK,
            .conn = {.state=UTEST_DB_ASYNC_FREE_RESULTSET_DONE, .async=0,
                .alias=conn->pool->cfg.alias}, ._final = 1};
        app_db_query_notify_with_result(curr_query, rs);
    }
    uv_run(curr_query->cfg.loop, UV_RUN_NOWAIT);
    { conn->ops.update_ready_queries(conn); }
} // end of mock_dbpool_3pty_state_transition

static uint8_t mock_dbpool__notify_query(db_query_t *query, db_query_result_t *rs)
{
    uint8_t final = 0;
    switch(rs->conn.state) {
        case UTEST_DB_ASYNC_FETCH_ROW_READY:
            query->cfg.callbacks.row_fetched(query, rs);
            break;
        case UTEST_DB_ASYNC_FREE_RESULTSET_DONE:
            query->cfg.callbacks.result_free(query, rs);
            final = rs->_final;
            break;
    }
    return final;
} // end of mock_dbpool__notify_query

static int  mock_dbpool__get_sock_fd(db_conn_t *conn)
{ return -1;}

static uint64_t  mock_dbpool__get_timeout_ms(db_conn_t *conn)
{ return 123; }

static uint8_t   mock_dbpool__is_conn_closed(db_conn_t *conn)
{ return 0; }


static db_pool_cfg_t  utest_db_pool_cfg = {
    .alias=UTEST_DBPOOL_ALIAS, .capacity=1, .idle_timeout=13, .bulk_query_limit_kb=3,
    .conn_detail = {.db_name="x", .db_user="x", .db_passwd="x", .db_host="x", .db_port=4567},
    .ops = {.conn_init_fn=mock_dbpool_3pty_init_fn, .conn_deinit_fn=mock_dbpool_3pty_deinit_fn,
        .state_transition=mock_dbpool_3pty_state_transition, .notify_query=mock_dbpool__notify_query,
        .is_conn_closed=mock_dbpool__is_conn_closed, .get_sock_fd=mock_dbpool__get_sock_fd,
        .get_timeout_ms=mock_dbpool__get_timeout_ms, .can_change_state=mock_dbpool__can_change_state }
};

static void utest__db_async_err_cb(db_query_t *q, db_query_result_t *rs)
{ mock(q, rs); }

static int utest__uncommitted_upld_req_success(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{ return mock(); }

static int utest__uncommitted_upld_req_failure(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{ return mock(); }


Ensure(apiview_common_test__upload_request_found) {
#define  UTEST_NUM_ENTRIES_HASHMAP  6
    struct hsearch_data    htab = {0};
    hcreate_r(UTEST_NUM_ENTRIES_HASHMAP, &htab);
    json_t *mock_jwt_claims = json_object();
    json_object_set_new(mock_jwt_claims, "profile", json_integer(0x468));
    app_save_int_to_hashmap(&htab, "req_seq", 0x456);
    app_save_ptr_to_hashmap(&htab, "auth", mock_jwt_claims);
    uv_loop_t     *mock_loop = uv_default_loop();
    h2o_context_t  http_srv_ctx = {.loop=mock_loop};
    h2o_conn_t     http_conn = {.ctx=&http_srv_ctx};
    h2o_req_t      http_req = {.conn=&http_conn};
    h2o_handler_t  hdlr = {0};
    app_middleware_node_t  node = {.data=&htab};
    app_db_pool_init(&utest_db_pool_cfg);
    {
        uint8_t mock_has_row = 1;
        size_t  mock_num_cols = 0;
        expect(mock_dbpool_3pty_state_transition,
                will_set_contents_of_parameter(has_row_ptr, &mock_has_row, sizeof(uint8_t)),
                will_set_contents_of_parameter(num_cols_ptr, &mock_num_cols, sizeof(size_t)));
        expect(utest__uncommitted_upld_req_success);
        DBA_RES_CODE db_result = app_validate_uncommitted_upld_req(
                &hdlr, &http_req, &node, "utest_upld_req_table", utest__db_async_err_cb,
                utest__uncommitted_upld_req_success, utest__uncommitted_upld_req_failure
            );
        assert_that(db_result, is_equal_to(DBA_RESULT_OK));
    }
    {
        uint8_t mock_has_row = 0;
        size_t  mock_num_cols = 0;
        expect(mock_dbpool_3pty_state_transition,
                will_set_contents_of_parameter(has_row_ptr, &mock_has_row, sizeof(uint8_t)),
                will_set_contents_of_parameter(num_cols_ptr, &mock_num_cols, sizeof(size_t)));
        expect(utest__uncommitted_upld_req_failure);
        DBA_RES_CODE db_result = app_validate_uncommitted_upld_req(
                &hdlr, &http_req, &node, "utest_upld_req_table", utest__db_async_err_cb,
                utest__uncommitted_upld_req_success, utest__uncommitted_upld_req_failure
            );
        assert_that(db_result, is_equal_to(DBA_RESULT_OK));
    }
    app_db_pool_deinit(UTEST_DBPOOL_ALIAS);
    hdestroy_r(&htab);
    json_decref(mock_jwt_claims);
#undef  UTEST_NUM_ENTRIES_HASHMAP
} // end of apiview_common_test__upload_request_found


Ensure(apiview_common_test__resource_id_format) {
    assert_that(app_verify_printable_string(NULL,0), is_not_equal_to(0));
    assert_that(app_verify_printable_string("",0), is_not_equal_to(0));
    assert_that(app_verify_printable_string("abcde", 5), is_equal_to(0));
    assert_that(app_verify_printable_string("abc e", 5), is_not_equal_to(0));
    int ret = app_verify_printable_string("a\x01cde", 5);
    assert_that(ret, is_not_equal_to(0));
    char _res_id[APP_RESOURCE_ID_SIZE + 5] = {0};
    memset(&_res_id[0], 0x61, sizeof(char) * (APP_RESOURCE_ID_SIZE + 3));
    assert_that(app_verify_printable_string(&_res_id[0], APP_RESOURCE_ID_SIZE), is_not_equal_to(0));
} // end of apiview_common_test__resource_id_format

TestSuite *app_views_common_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, apiview_common_test__upload_request_found);
    add_test(suite, apiview_common_test__resource_id_format);
    return suite;
}
