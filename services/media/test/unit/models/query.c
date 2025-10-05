#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <h2o/memory.h>
#include "models/query.h"

static uint8_t mock_db_pool__is_closing_fn(struct db_pool_s *pool) { return (uint8_t)mock(pool); }

static db_conn_t *mock_db_pool__acquire_free_conn_fn(db_pool_t *pool) { return (db_conn_t *)mock(pool); }

static DBA_RES_CODE mock_db_pool__release_used_conn_fn(db_conn_t *conn) { return (DBA_RES_CODE)mock(conn); }

static DBA_RES_CODE mock_db_conn__add_new_query(db_conn_t *conn, db_query_t *query) {
    db_llnode_t *q_node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, query);
    conn->pending_queries.head = q_node;
    conn->pending_queries.tail = q_node;
    return (DBA_RES_CODE)mock(conn, query);
}

static DBA_RES_CODE mock_db_conn__try_process_queries(db_conn_t *conn, uv_loop_t *loop) {
    return (DBA_RES_CODE)mock(conn, loop);
}

static void mock_db_query_cb__result_set_ready(db_query_t *target, db_query_result_t *detail) {
    mock(target, detail);
}

static void mock_db_query_cb__fetch_row(db_query_t *target, db_query_result_t *detail) {
    mock(target, detail);
}

static void mock_db_query_cb__result_set_free(db_query_t *target, db_query_result_t *detail) {
    mock(target, detail);
}

static void mock_db_query_cb__async_error(db_query_t *target, db_query_result_t *detail) {
    mock(target, detail);
}

Ensure(app_db_query_test_queue_resultset) {
#define EXPECT_NUM_RS 5
    db_query_t        query = {0};
    db_query_result_t rs[EXPECT_NUM_RS] = {0};
    size_t            expect_rs_idx_seq[EXPECT_NUM_RS] = {4, 2, 1, 3, 0};
    DBA_RES_CODE      result = DBA_RESULT_OK;
    size_t            idx = 0;
    pthread_mutex_init(&query.db_result.lock, NULL);
    {
        result = app_db_query_enqueue_resultset(&query, NULL);
        assert_that(result, is_equal_to(DBA_RESULT_ERROR_ARG));
        for (idx = 0; idx < EXPECT_NUM_RS; idx++) {
            result = app_db_query_enqueue_resultset(&query, &rs[expect_rs_idx_seq[idx]]);
            assert_that(result, is_equal_to(DBA_RESULT_OK));
        }
        for (idx = 0; idx < EXPECT_NUM_RS; idx++) {
            db_query_result_t *actual_rs = app_db_query_dequeue_resultset(&query);
            assert_that(actual_rs, is_equal_to(&rs[expect_rs_idx_seq[idx]]));
        }
        assert_that(app_db_query_dequeue_resultset(&query), is_null);
        assert_that(app_db_query_dequeue_resultset(&query), is_null);
    }
    pthread_mutex_destroy(&query.db_result.lock);
#undef EXPECT_NUM_RS
} // end of app_db_query_test_queue_resultset

#define EXPECT_RAW_SQL \
    "SELECT col2, col3 FROM table56; INSERT INTO table56(col2, col3) VALUES('beard', 839274); " \
    "UPDATE table356 SET logusr = 'wood';"
#define EXPECT_NUM_RS 2
Ensure(app_db_query_test_start_new_query_failure) {
    db_pool_t pool = {
        .cfg = {.bulk_query_limit_kb = 0},
        .is_closing_fn = mock_db_pool__is_closing_fn,
        .acquire_free_conn_fn = mock_db_pool__acquire_free_conn_fn,
        .release_used_conn_fn = mock_db_pool__release_used_conn_fn
    };
    uv_loop_t      loop = {0};
    db_query_cfg_t qcfg =
        {.statements = {.entry = EXPECT_RAW_SQL, .num_rs = EXPECT_NUM_RS},
         .pool = &pool,
         .usr_data = {.entry = (void **)NULL, .len = 0},
         .loop = &loop,
         .callbacks = {
             .result_rdy = mock_db_query_cb__result_set_ready,
             .row_fetched = mock_db_query_cb__fetch_row,
             .result_free = mock_db_query_cb__result_set_free,
             .error = mock_db_query_cb__async_error,
         }};
    db_conn_t free_conn = {0};
    { // assume app has been closing the connection pool
        expect(mock_db_pool__is_closing_fn, will_return(1), when(pool, is_equal_to(&pool)));
        assert_that(app_db_query_start(&qcfg), is_equal_to(DBA_RESULT_POOL_BUSY));
        expect(mock_db_pool__is_closing_fn, will_return(0), when(pool, is_equal_to(&pool)));
        expect(mock_db_pool__acquire_free_conn_fn, will_return(NULL));
        assert_that(app_db_query_start(&qcfg), is_equal_to(DBA_RESULT_POOL_BUSY));
    }
    { // error happened due to incorrect size of internal buffer in `db_conn_t` object
        expect(mock_db_pool__is_closing_fn, will_return(0), when(pool, is_equal_to(&pool)));
        expect(mock_db_pool__acquire_free_conn_fn, will_return(&free_conn), when(pool, is_equal_to(&pool)));
        expect(
            mock_db_pool__release_used_conn_fn, will_return(DBA_RESULT_OK),
            when(conn, is_equal_to(&free_conn))
        );
        assert_that(app_db_query_start(&qcfg), is_equal_to(DBA_RESULT_ERROR_ARG));
    }
} // end of app_db_query_test_start_new_query_failure

Ensure(app_db_query_test_start_new_query_ok) {
#define NUM_USR_DATA_ITEMS 2
    int16_t   usr_data_1 = 35;
    uint64_t  usr_data_2 = 0x900293d4;
    void     *db_async_usr_data[NUM_USR_DATA_ITEMS] = {(void *)&usr_data_1, (void *)&usr_data_2};
    db_pool_t pool = {
        .cfg = {.bulk_query_limit_kb = 1},
        .is_closing_fn = mock_db_pool__is_closing_fn,
        .acquire_free_conn_fn = mock_db_pool__acquire_free_conn_fn,
        .release_used_conn_fn = mock_db_pool__release_used_conn_fn
    };
    uv_loop_t     *_loop = uv_default_loop();
    db_query_cfg_t qcfg =
        {.statements = {.entry = EXPECT_RAW_SQL, .num_rs = EXPECT_NUM_RS},
         .pool = &pool,
         .usr_data = {.entry = (void **)db_async_usr_data, .len = NUM_USR_DATA_ITEMS},
         .loop = _loop,
         .callbacks = {
             .result_rdy = mock_db_query_cb__result_set_ready,
             .row_fetched = mock_db_query_cb__fetch_row,
             .result_free = mock_db_query_cb__result_set_free,
             .error = mock_db_query_cb__async_error,
         }};
    db_conn_t free_conn =
        {.ops = {
             .try_process_queries = mock_db_conn__try_process_queries,
             .add_new_query = mock_db_conn__add_new_query,
         }};
    {
        expect(mock_db_pool__is_closing_fn, will_return(0), when(pool, is_equal_to(&pool)));
        expect(mock_db_pool__acquire_free_conn_fn, will_return(&free_conn), when(pool, is_equal_to(&pool)));
        expect(mock_db_conn__add_new_query, will_return(DBA_RESULT_OK), when(conn, is_equal_to(&free_conn)));
        expect(
            mock_db_pool__release_used_conn_fn, will_return(DBA_RESULT_OK),
            when(conn, is_equal_to(&free_conn))
        );
        expect(
            mock_db_conn__try_process_queries, will_return(DBA_RESULT_OK),
            when(conn, is_equal_to(&free_conn)), when(loop, is_equal_to(_loop))
        );
        assert_that(app_db_query_start(&qcfg), is_equal_to(DBA_RESULT_OK));
        assert_that(free_conn.pending_queries.head, is_not_null);
    }
    if (free_conn.pending_queries.head) {
        uv_run(_loop, UV_RUN_NOWAIT);
        db_query_t *query = (db_query_t *)&free_conn.pending_queries.head->data[0];
        assert_that(query->cfg.usr_data.len, is_equal_to(NUM_USR_DATA_ITEMS));
        for (size_t idx = 0; idx < NUM_USR_DATA_ITEMS; idx++) {
            assert_that(query->cfg.usr_data.entry[idx], is_equal_to(db_async_usr_data[idx]));
        }
        // ensure the copied statement is NULL-terminated
        assert_that(query->cfg.statements.entry, is_equal_to_string(qcfg.statements.entry));
        uv_close((uv_handle_t *)&query->notification, NULL);
        uv_run(_loop, UV_RUN_ONCE);
        free(free_conn.pending_queries.head);
    }
#undef NUM_USR_DATA_ITEMS
} // end of app_db_query_test_start_new_query_ok

TestSuite *app_model_query_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, app_db_query_test_queue_resultset);
    add_test(suite, app_db_query_test_start_new_query_failure);
    add_test(suite, app_db_query_test_start_new_query_ok);
    return suite;
}
