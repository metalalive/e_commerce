#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include "models/connection.h"

#define CONN_BULK_QUERY_LIMIT_KB 1

typedef struct {
    db_conn_t super;
    char raw_queries[(CONN_BULK_QUERY_LIMIT_KB << 10)]; // raw queries ready to send in bulk
} db_conn_extend_t;

typedef struct {
    db_llnode_t  node;
    db_query_t   query;
} db_query_extend_t;


static uint8_t  mock_db_conn__can_change_state(db_conn_t *conn)
{ return (uint8_t)mock(conn); }

static void mock_db_conn__state_transition(app_timer_poll_t *target, int status, int event)
{ mock(target, status, event); }

static uint8_t  mock_db_pool__is_conn_closed(db_conn_t *conn)
{ return (uint8_t)mock(conn); }

#define _app_db_conn_try_evict_query_test(conn, expect_num_rs, curr_q_node, next_q_node) \
{ \
    DBA_RES_CODE result = DBA_RESULT_OK; \
    for(size_t jdx = 1; jdx < (expect_num_rs); jdx++) { \
        result = app_db_conn_try_evict_current_processing_query((conn)); \
        assert_that(result, is_equal_to(DBA_RESULT_OK)); \
        assert_that((conn)->processing_queries, is_equal_to((curr_q_node))); \
    } \
    result = app_db_conn_try_evict_current_processing_query((conn)); \
    assert_that(result, is_equal_to(DBA_RESULT_OK)); \
    assert_that((conn)->processing_queries, is_equal_to((next_q_node))); \
} // end of _app_db_conn_try_evict_query_test

Ensure(app_db_conn_try_evict_query_test) {
    db_conn_extend_t conn = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB}};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    db_query_extend_t  q_head = {0};
    db_query_extend_t  q_tail = {0};
    {
        db_query_t *q = (db_query_t *) &q_head.node.data[0];
        result = app_db_conn_try_evict_current_processing_query(NULL);
        assert_that(result, is_equal_to(DBA_RESULT_ERROR_ARG));
        result = app_db_conn_try_evict_current_processing_query(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_MEMORY_ERROR));
        q->db_result.num_rs_remain = 0;
        conn.super.processing_queries = &q_head.node;
        result = app_db_conn_try_evict_current_processing_query(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        assert_that(conn.super.processing_queries, is_null);
    }
    size_t max_num_rs = 7;
    size_t idx = 0;
    for(idx = 1; idx < max_num_rs; idx++) {
        size_t _expect_num_rs = idx;
        db_query_t *q = (db_query_t *) &q_head.node.data[0];
        q->db_result.num_rs_remain = _expect_num_rs;
        conn.super.processing_queries = &q_head.node;
        _app_db_conn_try_evict_query_test(&conn.super, _expect_num_rs, &q_head.node, NULL);
    }
    { // assume 2 queries in the chain are processing currently
        db_query_t *q0 = (db_query_t *) &q_head.node.data[0];
        db_query_t *q1 = (db_query_t *) &q_tail.node.data[0];
        size_t expect_num_rs[2] = {3, 5};
        q0->db_result.num_rs_remain = expect_num_rs[0];
        q1->db_result.num_rs_remain = expect_num_rs[1];
        q_head.node.next = &q_tail.node;
        q_tail.node.prev = &q_head.node;
        conn.super.processing_queries = &q_head.node;
        _app_db_conn_try_evict_query_test(&conn.super, expect_num_rs[0], &q_head.node, &q_tail.node);
        _app_db_conn_try_evict_query_test(&conn.super, expect_num_rs[1], &q_tail.node, NULL);
    }
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_try_evict_query_test


Ensure(app_db_conn_get_first_query_test) {
    db_conn_extend_t conn = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB}};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    db_query_extend_t  q_pend = {0};
    db_query_extend_t  q_rdy  = {0};
    {
        conn.super.processing_queries = &q_rdy.node;
        conn.super.pending_queries.head = &q_pend.node;
        db_query_t *q = app_db_conn_get_first_query(&conn.super);
        assert_that(q, is_equal_to(&q_rdy.node.data[0]));
    }
    {
        conn.super.processing_queries = NULL;
        conn.super.pending_queries.head = &q_pend.node;
        db_query_t *q = app_db_conn_get_first_query(&conn.super);
        assert_that(q, is_equal_to(&q_pend.node.data[0]));
    }
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_CONNECTION_BUSY));
    conn.super.pending_queries.head = NULL;
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_get_first_query_test

Ensure(app_db_conn_add_new_query_test) {
    db_conn_extend_t conn = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB}};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    {
        db_query_extend_t  qs_pend[4] = {0};
        conn.super.ops.add_new_query(&conn.super, (db_query_t *)&qs_pend[3].node.data[0]);
        assert_that(conn.super.pending_queries.head, is_equal_to(&qs_pend[3].node));
        assert_that(conn.super.pending_queries.tail, is_equal_to(&qs_pend[3].node));
        conn.super.ops.add_new_query(&conn.super, (db_query_t *)&qs_pend[2].node.data[0]);
        assert_that(conn.super.pending_queries.head, is_equal_to(&qs_pend[3].node));
        assert_that(conn.super.pending_queries.tail, is_equal_to(&qs_pend[2].node));
        conn.super.ops.add_new_query(&conn.super, (db_query_t *)&qs_pend[1].node.data[0]);
        conn.super.ops.add_new_query(&conn.super, (db_query_t *)&qs_pend[0].node.data[0]);
        assert_that(conn.super.pending_queries.head, is_equal_to(&qs_pend[3].node));
        assert_that(conn.super.pending_queries.tail, is_equal_to(&qs_pend[0].node));
    }
    conn.super.pending_queries.head = NULL;
    conn.super.pending_queries.tail = NULL;
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_add_new_query_test

#define NUM_NEW_QUERIES  11
#define TEST_RAW_SQL  "SELECT a123, a234, a345, other_column FROM some_table WHERE a456 = 987 AND a567 = 108 ORDER BY b123 DESC LIMIT 21;"
Ensure(app_db_conn_update_ready_queries_test_1) {
    db_conn_extend_t conn = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB}};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    db_query_extend_t  qs_pend[NUM_NEW_QUERIES] = {0};
    size_t  expect_sql_nbytes = strlen(TEST_RAW_SQL);
    size_t  expect_num_processing = 0;
    size_t  actual_num_processing = 0;
    int idx = 0;
    for(idx = NUM_NEW_QUERIES - 1; idx >= 0; idx--) {
        db_query_t *q = (db_query_t *) &qs_pend[idx].node.data[0];
        q->cfg.statements.entry = TEST_RAW_SQL;
        q->cfg.statements.num_rs = 1;
        q->_stmts_tot_sz = expect_sql_nbytes;
        conn.super.ops.add_new_query(&conn.super, q);
    }
    { // determine how many of queries to process (due to buffer restriction)
        db_llnode_t *q_node = NULL;
        expect_num_processing = (pool.cfg.bulk_query_limit_kb << 10) / expect_sql_nbytes;
        actual_num_processing = 0;
        assert_that(expect_num_processing, is_less_than(NUM_NEW_QUERIES));
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        actual_num_processing = 0;
        for(q_node = conn.super.processing_queries; q_node;
                q_node = q_node->next, actual_num_processing++);
        assert_that(actual_num_processing, is_equal_to(expect_num_processing));
        for(q_node = conn.super.pending_queries.head;
                q_node != conn.super.pending_queries.tail; q_node = q_node->next);
        assert_that(conn.super.pending_queries.head,
                is_equal_to(&qs_pend[NUM_NEW_QUERIES - expect_num_processing - 1].node));
        assert_that(conn.super.pending_queries.tail, is_equal_to(&qs_pend[0].node));
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_SKIPPED));
    }
    conn.super.processing_queries = NULL;
    { // move the rest of pending queries to ready list , for later process
        db_llnode_t *q_node = NULL;
        expect_num_processing = NUM_NEW_QUERIES - expect_num_processing;
        actual_num_processing = 0;
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        for(q_node = conn.super.processing_queries; q_node;
                q_node = q_node->next, actual_num_processing++);
        assert_that(actual_num_processing, is_equal_to(expect_num_processing));
        assert_that(conn.super.processing_queries, is_equal_to(&qs_pend[expect_num_processing - 1].node));
        assert_that(conn.super.pending_queries.head, is_equal_to(NULL));
        assert_that(conn.super.pending_queries.tail, is_equal_to(NULL));
        uint8_t has_more =  (uint8_t) atomic_load_explicit(
                &conn.super.flags.has_ready_query_to_process, memory_order_relaxed);
        assert_that(has_more, is_equal_to(1));
    }
    conn.super.processing_queries = NULL;
    {
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_SKIPPED));
        uint8_t has_more =  (uint8_t) atomic_load_explicit(
                &conn.super.flags.has_ready_query_to_process, memory_order_relaxed);
        assert_that(has_more, is_equal_to(0));
    }
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_update_ready_queries_test_1
#undef  NUM_NEW_QUERIES
#undef  TEST_RAW_SQL


#define NUM_QUERIES 4
#define TEST_RAW_SQL_1  "SELECT COUNT(a123), a789 FROM some_table WHERE a456 = 987 AND a567 = 108 GROUP BY ghj;"
#define TEST_RAW_SQL_2  "SELECT d543, d567 FROM some_other_table WHERE xyz = 'tyui' LIMIT 10;"
#define TEST_RAW_SQL_3  "SELECT g67 FROM third_table WHERE jklm < 90 LIMIT 10;"
#define TEST_RAW_SQL_4  "SELECT m33, risc FROM fourth_table;"
Ensure(app_db_conn_update_ready_queries_test_2) {
    const char *test_raw_sqls[NUM_QUERIES] = { TEST_RAW_SQL_1, TEST_RAW_SQL_2, 
        TEST_RAW_SQL_3, TEST_RAW_SQL_4 };
    db_conn_extend_t conn = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = 1}};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    db_query_extend_t  qs_pend[NUM_QUERIES] = {0};
    int idx = 0;
    for(idx = 0; idx < NUM_QUERIES; idx++) {
        db_query_t *q = (db_query_t *) &qs_pend[idx].node.data[0];
        q->cfg.statements.num_rs = 1;
        q->cfg.statements.entry = test_raw_sqls[idx];
        q->_stmts_tot_sz = strlen(test_raw_sqls[idx]);
    }
    {
        conn.super.ops.add_new_query(&conn.super, (db_query_t *) &qs_pend[0].node.data[0]);
        conn.super.ops.add_new_query(&conn.super, (db_query_t *) &qs_pend[1].node.data[0]);
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        size_t exp_stmts_sz = strlen(TEST_RAW_SQL_1  TEST_RAW_SQL_2);
        assert_that(conn.super.bulk_query_rawbytes.wr_sz, is_equal_to(exp_stmts_sz));
        assert_that(&conn.super.bulk_query_rawbytes.data[0],
                is_equal_to_string(TEST_RAW_SQL_1  TEST_RAW_SQL_2));
    }
    conn.super.processing_queries = NULL;
    {
        conn.super.ops.add_new_query(&conn.super, (db_query_t *) &qs_pend[2].node.data[0]);
        conn.super.ops.add_new_query(&conn.super, (db_query_t *) &qs_pend[3].node.data[0]);
        result = conn.super.ops.update_ready_queries(&conn.super);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        size_t exp_stmts_sz = strlen(TEST_RAW_SQL_3  TEST_RAW_SQL_4);
        assert_that(conn.super.bulk_query_rawbytes.wr_sz, is_equal_to(exp_stmts_sz));
        assert_that(&conn.super.bulk_query_rawbytes.data[0],
                is_equal_to_string(TEST_RAW_SQL_3  TEST_RAW_SQL_4));
    }
} // end of app_db_conn_update_ready_queries_test_2
#undef NUM_QUERIES
#undef TEST_RAW_SQL_1
#undef TEST_RAW_SQL_2
#undef TEST_RAW_SQL_3
#undef TEST_RAW_SQL_4


Ensure(app_db_conn_try_process_queries_test) {
    db_conn_extend_t conn = {0};
    uv_loop_t  loop = {0};
    db_pool_t  pool = {.cfg = { .bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,
        .ops = {
            .can_change_state = mock_db_conn__can_change_state,
            .state_transition = mock_db_conn__state_transition
        },
    }};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    { // assume there's still processing query registered in the connection
        uint8_t value = 1;
        atomic_store_explicit(&conn.super.flags.has_ready_query_to_process, value, memory_order_relaxed);
        result = conn.super.ops.try_process_queries(&conn.super, &loop);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
    }
    { // assume there's no more processing query, but another worker thread contends the same connection
        uint8_t value = 0;
        atomic_store_explicit(&conn.super.flags.has_ready_query_to_process, value, memory_order_relaxed);
        expect(mock_db_conn__can_change_state , will_return(0));
        result = conn.super.ops.try_process_queries(&conn.super, &loop);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
    }
    { // no processing query, current worker thread is allowed to transit the state of connection
        assert_that(conn.super.loop, is_equal_to(NULL));
        expect(mock_db_conn__can_change_state, will_return(1));
        expect(mock_db_conn__state_transition,
                when(target, is_equal_to(&conn.super.timer_poll)),
                when(status, is_equal_to(0)),
                when(event,  is_equal_to(0))
            );
        result = conn.super.ops.try_process_queries(&conn.super, &loop);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
        assert_that(conn.super.loop, is_equal_to(&loop));
    }
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_try_process_queries_test


Ensure(app_db_conn_try_close_test) {
    uv_loop_t *loop = uv_default_loop();
    db_conn_extend_t conn = {.super = {.timer_poll = {.poll = {.loop = loop}}}};
    db_pool_t  pool = {.cfg = { .bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,
        .ops = {
            .can_change_state = mock_db_conn__can_change_state,
            .state_transition = mock_db_conn__state_transition,
            .is_conn_closed = mock_db_pool__is_conn_closed
        },
    }};
    DBA_RES_CODE result = app_db_conn_init(&conn.super, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    { // asssume the connection is busy
        expect(mock_db_pool__is_conn_closed, will_return(0));
        expect(mock_db_conn__can_change_state , will_return(0));
        result = conn.super.ops.try_close(&conn.super, loop);
        assert_that(result, is_equal_to(DBA_RESULT_CONNECTION_BUSY));
    }
    { // current thread can close the connection
        expect(mock_db_pool__is_conn_closed, will_return(0));
        expect(mock_db_conn__can_change_state , will_return(1));
        expect(mock_db_conn__state_transition,  when(target, is_equal_to(&conn.super.timer_poll)));
        result = conn.super.ops.try_close(&conn.super, loop);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
    }
    { // already closed
        expect(mock_db_pool__is_conn_closed, will_return(1));
        result = conn.super.ops.try_close(&conn.super, loop);
        assert_that(result, is_equal_to(DBA_RESULT_SKIPPED));
    }
    result = app_db_conn_deinit(&conn.super);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
} // end of app_db_conn_try_close_test


TestSuite *app_model_connection_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_db_conn_try_evict_query_test);
    add_test(suite, app_db_conn_get_first_query_test);
    add_test(suite, app_db_conn_add_new_query_test);
    add_test(suite, app_db_conn_update_ready_queries_test_1);
    add_test(suite, app_db_conn_update_ready_queries_test_2);
    add_test(suite, app_db_conn_try_process_queries_test);
    add_test(suite, app_db_conn_try_close_test);
    return suite;
}
