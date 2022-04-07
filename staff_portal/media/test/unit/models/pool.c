#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include "models/pool.h"

static DBA_RES_CODE  mock_db_conn__try_close(db_conn_t *conn, uv_loop_t *loop)
{
    return (DBA_RES_CODE)mock(conn, loop);
} // end of mock_db_conn__try_close

static uint8_t  mock_db_conn__is_closed(db_conn_t *conn)
{
    return (uint8_t) mock(conn);
} // end of mock_db_conn__is_closed

static  DBA_RES_CODE mock_db_conn_init(db_conn_t *conn, db_pool_t *pool)
{
    conn->ops.try_close = mock_db_conn__try_close;
    conn->ops.is_closed = mock_db_conn__is_closed;
    return (DBA_RES_CODE)mock(conn, pool);
}

static  DBA_RES_CODE mock_db_conn_deinit(db_conn_t *conn)
{ return (DBA_RES_CODE)mock(conn); }

static  void  mock_db_conn__error_cb(db_conn_t *conn, db_conn_err_detail_t *detail)
{ mock(conn, detail); }

static  uint8_t  mock_db_conn__can_change_state(db_conn_t *conn)
{ return (uint8_t)mock(conn); }

static void mock_db_conn__state_transition(app_timer_poll_t *target, int status, int event)
{ mock(target, status, event); }

static  int  mock_db_conn__get_sock_fd(db_conn_t *conn)
{ return (int)mock(conn); }

static  uint64_t  mock_db_conn__get_timeout_ms(db_conn_t *conn)
{ return (uint64_t)mock(conn); }

static  uint8_t  mock_db_conn__notify_query(db_query_t *query, db_query_result_t *rs)
{ return (uint8_t)mock(query, rs); }

static  uint8_t  mock_db_pool__is_conn_closed(db_conn_t *conn)
{ return (uint8_t)mock(conn); }


Ensure(app_model_pool_init_missing_arg_test) {
    db_pool_cfg_t cfg_opts = {
        .alias="db_primary", .capacity=4, .idle_timeout=80, .bulk_query_limit_kb=2,
        .conn_detail={.db_name = "ecommerce_media_123"},  .ops = {.init_fn = mock_db_conn_init}
    };
    DBA_RES_CODE result = app_db_pool_init(&cfg_opts);
    assert_that(result, is_equal_to(DBA_RESULT_ERROR_ARG));
} // end of app_model_pool_init_missing_arg_test

static void assert_traverse_linklist(db_llnode_t *head, size_t expect_sz)
{
    size_t actual_sz = 0;
    db_llnode_t *node = NULL;
    db_llnode_t *prev = NULL;
    for(node = head; node; actual_sz++, prev = node, node = node->next);
    assert_that(actual_sz, is_equal_to(expect_sz));
    assert_that(prev, is_not_null);
    actual_sz = 0;
    for(node = prev, prev = NULL; node; actual_sz++, prev = node, node = node->prev);
    assert_that(actual_sz, is_equal_to(expect_sz));
    assert_that(prev, is_not_null);
    assert_that(prev, is_equal_to(head));
}
 // end of assert_traverse_linklist


Ensure(app_model_pool_init_one_test_ok) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_pool_t *pool_found = NULL;
    db_pool_cfg_t cfg_opts = {
        .alias="db_primary", .capacity=5, .idle_timeout=80, .bulk_query_limit_kb=2,
        .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
            .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
        .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
            .error_cb = mock_db_conn__error_cb,
            .can_change_state = mock_db_conn__can_change_state,
            .state_transition = mock_db_conn__state_transition,
            .get_sock_fd = mock_db_conn__get_sock_fd,
            .get_timeout_ms = mock_db_conn__get_timeout_ms,
            .notify_query   = mock_db_conn__notify_query,
            .is_conn_closed = mock_db_pool__is_conn_closed,
        }
    };
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    result = app_db_pool_init(&cfg_opts);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    pool_found = app_db_pool_get_pool("db_primary");
    assert_that(pool_found, is_not_null);
    assert_that(pool_found->cfg.conn_detail.db_host, is_not_equal_to(cfg_opts.conn_detail.db_host));
    assert_that(pool_found->cfg.conn_detail.db_host, is_equal_to_string(cfg_opts.conn_detail.db_host));
    assert_that(pool_found->cfg.conn_detail.db_user, is_not_equal_to(cfg_opts.conn_detail.db_user));
    assert_that(pool_found->cfg.conn_detail.db_user, is_equal_to_string(cfg_opts.conn_detail.db_user));
    assert_that(pool_found->conns.head, is_not_null);
    assert_that(pool_found->conns.tail, is_not_null);
    assert_traverse_linklist(pool_found->conns.head, cfg_opts.capacity);
    {
        uint16_t init_flag = (uint16_t) atomic_load_explicit(&pool_found->flags, memory_order_relaxed);
        assert_that(init_flag, is_equal_to((uint16_t)0));
    }
    {
        db_conn_cbs_t *ops = &pool_found->cfg.ops;
        assert_that(ops->init_fn   , is_equal_to(mock_db_conn_init));
        assert_that(ops->deinit_fn , is_equal_to(mock_db_conn_deinit));
        assert_that(ops->error_cb  , is_equal_to(mock_db_conn__error_cb));
        assert_that(ops->can_change_state , is_equal_to(mock_db_conn__can_change_state));
        assert_that(ops->state_transition , is_equal_to(mock_db_conn__state_transition));
        assert_that(ops->get_sock_fd    , is_equal_to(mock_db_conn__get_sock_fd));
        assert_that(ops->get_timeout_ms , is_equal_to(mock_db_conn__get_timeout_ms));
        assert_that(ops->notify_query   , is_equal_to(mock_db_conn__notify_query));
        assert_that(ops->is_conn_closed , is_equal_to(mock_db_pool__is_conn_closed));
    }
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    result = app_db_pool_deinit("db_primary");
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    pool_found = app_db_pool_get_pool("db_primary");
    assert_that(pool_found, is_null);
} // end of app_model_pool_init_one_test_ok


Ensure(app_model_pool_init_one_test_error_init_conns) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_pool_t *pool_found = NULL;
    db_pool_cfg_t cfg_opts = {
        .alias="db_primary", .capacity=5, .idle_timeout=80, .bulk_query_limit_kb=2,
        .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
            .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
        .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
            .error_cb = mock_db_conn__error_cb,
            .can_change_state = mock_db_conn__can_change_state,
            .state_transition = mock_db_conn__state_transition,
            .get_sock_fd = mock_db_conn__get_sock_fd,
            .get_timeout_ms = mock_db_conn__get_timeout_ms,
            .notify_query   = mock_db_conn__notify_query,
            .is_conn_closed = mock_db_pool__is_conn_closed,
        }
    };
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_UNKNOWN_ERROR));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    result = app_db_pool_init(&cfg_opts);
    assert_that(result, is_equal_to(DBA_RESULT_UNKNOWN_ERROR));
    pool_found = app_db_pool_get_pool("db_primary");
    assert_that(pool_found, is_null);
} // end of app_model_pool_init_one_test_error_init_conns


Ensure(app_model_pool_init_duplicate_error) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_pool_t *pool_found = NULL;
    db_pool_cfg_t cfg_opts[2] = {
        {
            .alias="db_primary", .capacity=2, .idle_timeout=80, .bulk_query_limit_kb=2,
            .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
                .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
            .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .error_cb = mock_db_conn__error_cb,
                .can_change_state = mock_db_conn__can_change_state,
                .state_transition = mock_db_conn__state_transition,
                .get_sock_fd = mock_db_conn__get_sock_fd,
                .get_timeout_ms = mock_db_conn__get_timeout_ms,
                .notify_query   = mock_db_conn__notify_query,
                .is_conn_closed = mock_db_pool__is_conn_closed,
            }
        },
        {
            .alias="db_primary", .capacity=3, .idle_timeout=100,  .bulk_query_limit_kb=2,
            .conn_detail={.db_name = "ecommerce_media_456", .db_user="username123",
                .db_passwd="password", .db_host="itest.myhost.com", .db_port=1987 },
            .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .error_cb = mock_db_conn__error_cb,
                .can_change_state = mock_db_conn__can_change_state,
                .state_transition = mock_db_conn__state_transition,
                .get_sock_fd = mock_db_conn__get_sock_fd,
                .get_timeout_ms = mock_db_conn__get_timeout_ms,
                .notify_query   = mock_db_conn__notify_query,
                .is_conn_closed = mock_db_pool__is_conn_closed,
            }
        }
    };
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    result = app_db_pool_init(&cfg_opts[0]);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    pool_found = app_db_pool_get_pool("db_primary");
    assert_that(pool_found, is_not_null);
    result = app_db_pool_init(&cfg_opts[1]);
    assert_that(result, is_equal_to(DBA_RESULT_MEMORY_ERROR));
    assert_that(app_db_pool_get_pool("db_primary"), is_equal_to(pool_found));
    assert_traverse_linklist(pool_found->conns.head, cfg_opts[0].capacity);
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    result = app_db_pool_deinit("db_primary");
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    result = app_db_pool_deinit("db_primary");
    assert_that(result, is_equal_to(DBA_RESULT_ERROR_ARG));
} // end of app_model_pool_init_duplicate_error


Ensure(app_model_pool_init_many_test_ok) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_pool_t *pool_found = NULL;
    size_t idx = 0;
    db_pool_cfg_t cfg_opts[3] = {
        {
            .alias="db_primary", .capacity=2, .idle_timeout=80,  .bulk_query_limit_kb=2,
            .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
                .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
            .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .error_cb = mock_db_conn__error_cb,
                .can_change_state = mock_db_conn__can_change_state,
                .state_transition = mock_db_conn__state_transition,
                .get_sock_fd = mock_db_conn__get_sock_fd,
                .get_timeout_ms = mock_db_conn__get_timeout_ms,
                .notify_query   = mock_db_conn__notify_query,
                .is_conn_closed = mock_db_pool__is_conn_closed,
            }
        },
        {
            .alias="db_replica_1", .capacity=3, .idle_timeout=100,  .bulk_query_limit_kb=2,
            .conn_detail={.db_name = "ecommerce_media_456", .db_user="bob",
                .db_passwd="uncle", .db_host="itest.myhost.com", .db_port=1987 },
            .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .error_cb = mock_db_conn__error_cb,
                .can_change_state = mock_db_conn__can_change_state,
                .state_transition = mock_db_conn__state_transition,
                .get_sock_fd = mock_db_conn__get_sock_fd,
                .get_timeout_ms = mock_db_conn__get_timeout_ms,
                .notify_query   = mock_db_conn__notify_query,
                .is_conn_closed = mock_db_pool__is_conn_closed,
            }
        },
        {
            .alias="db_repli2", .capacity=4, .idle_timeout=147,  .bulk_query_limit_kb=2,
            .conn_detail={.db_name = "ecommerce_media_458", .db_user="alice",
                .db_passwd="dreammaker", .db_host="itest.myhost.com", .db_port=1987 },
            .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .error_cb = mock_db_conn__error_cb,
                .can_change_state = mock_db_conn__can_change_state,
                .state_transition = mock_db_conn__state_transition,
                .get_sock_fd = mock_db_conn__get_sock_fd,
                .get_timeout_ms = mock_db_conn__get_timeout_ms,
                .notify_query   = mock_db_conn__notify_query,
                .is_conn_closed = mock_db_pool__is_conn_closed,
            }
        }
    };
    for(idx = 0; idx < 3; idx++) {
        for(size_t jdx = 0; jdx < cfg_opts[idx].capacity; jdx++) {
            expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
        }
        result = app_db_pool_init(&cfg_opts[idx]);
        assert_that(result, is_equal_to(DBA_RESULT_OK));
    }
    for(idx = 0; idx < 3; idx++) {
        pool_found = NULL;
        pool_found = app_db_pool_get_pool(cfg_opts[idx].alias);
        assert_that(pool_found, is_not_null);
        assert_that(pool_found->cfg.conn_detail.db_user, is_not_equal_to(cfg_opts[idx].conn_detail.db_user));
        assert_that(pool_found->cfg.conn_detail.db_user, is_equal_to_string(cfg_opts[idx].conn_detail.db_user));
        assert_traverse_linklist(pool_found->conns.head, cfg_opts[idx].capacity);
    }
    for(idx = 0; idx < 3; idx++) {
        for(size_t jdx = 0; jdx < cfg_opts[idx].capacity; jdx++) {
            expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
        }
    }
    result = app_db_pool_map_deinit();
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    for(idx = 0; idx < 3; idx++) {
        pool_found = app_db_pool_get_pool(cfg_opts[idx].alias);
        assert_that(pool_found, is_null);
    }
} // end of app_model_pool_init_many_test_ok


Describe(MOCK_DB_POOL);

BeforeEach(MOCK_DB_POOL) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    size_t capacity = 2;
    db_pool_cfg_t cfg_opts = {
        .alias="db_primary", .capacity=capacity, .idle_timeout=15, .bulk_query_limit_kb=2,
        .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
            .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
        .ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
            .error_cb = mock_db_conn__error_cb,
            .can_change_state = mock_db_conn__can_change_state,
            .state_transition = mock_db_conn__state_transition,
            .get_sock_fd = mock_db_conn__get_sock_fd,
            .get_timeout_ms = mock_db_conn__get_timeout_ms,
            .notify_query   = mock_db_conn__notify_query,
            .is_conn_closed = mock_db_pool__is_conn_closed,
        }
    };
    for(size_t idx = 0; idx < capacity; idx++) {
        expect(mock_db_conn_init, will_return(DBA_RESULT_OK));
    }
    result = app_db_pool_init(&cfg_opts);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
}

AfterEach(MOCK_DB_POOL) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    size_t capacity = 2;
    for(size_t idx = 0; idx < capacity; idx++) {
        expect(mock_db_conn_deinit, will_return(DBA_RESULT_OK));
    }
    result = app_db_pool_map_deinit();
    assert_that(result, is_equal_to(DBA_RESULT_OK));
}

Ensure(MOCK_DB_POOL, app_model_pool_signal_closing_test) {
    db_pool_t *pool = app_db_pool_get_pool("db_primary");
    assert_that(pool, is_not_null);
    uint16_t value = atomic_load_explicit(&pool->flags, memory_order_relaxed);
    assert_that(value, is_equal_to(0));
    for(size_t idx = 0; idx < 5; idx++) {
        app_db_pool_map_signal_closing();
        value = atomic_load_explicit(&pool->flags, memory_order_relaxed);
        assert_that(value, is_equal_to(1));
    }
} // end of app_model_pool_signal_closing_test

Ensure(MOCK_DB_POOL, app_model_poolmap_close_conns_test) {
    uv_loop_t  expect_loop = {0};
    db_pool_t *pool = app_db_pool_get_pool("db_primary");
    assert_that(pool, is_not_null);
    expect(mock_db_conn__try_close, will_return(DBA_RESULT_OK),
            when(loop, is_equal_to(&expect_loop)),
            when(conn, is_equal_to((db_conn_t *) &pool->conns.head->data[0]))
        );
    expect(mock_db_conn__try_close, will_return(DBA_RESULT_OK),
            when(loop, is_equal_to(&expect_loop)),
            when(conn, is_equal_to((db_conn_t *) &pool->conns.tail->data[0]))
        );
    app_db_poolmap_close_all_conns(&expect_loop);
} // end of app_model_poolmap_close_conns_test

Ensure(MOCK_DB_POOL, app_model_poolmap_check_conns_closed_test) {
    db_pool_t *pool = app_db_pool_get_pool("db_primary");
    assert_that(pool, is_not_null);
    uint8_t is_closed = 0;
    expect(mock_db_conn__is_closed, will_return(1), when(conn, is_equal_to((db_conn_t *) &pool->conns.head->data[0])) );
    expect(mock_db_conn__is_closed, will_return(0), when(conn, is_equal_to((db_conn_t *) &pool->conns.tail->data[0])) );
    is_closed = app_db_poolmap_check_all_conns_closed(); 
    assert_that(is_closed, is_equal_to(0));
    expect(mock_db_conn__is_closed, will_return(1), when(conn, is_equal_to((db_conn_t *) &pool->conns.head->data[0])) );
    expect(mock_db_conn__is_closed, will_return(1), when(conn, is_equal_to((db_conn_t *) &pool->conns.tail->data[0])) );
    is_closed = app_db_poolmap_check_all_conns_closed(); 
    assert_that(is_closed, is_equal_to(1));
} // end of app_model_poolmap_check_conns_closed_test


TestSuite *app_model_pool_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_model_pool_init_missing_arg_test);
    add_test(suite, app_model_pool_init_one_test_ok);
    add_test(suite, app_model_pool_init_one_test_error_init_conns);
    add_test(suite, app_model_pool_init_duplicate_error);
    add_test(suite, app_model_pool_init_many_test_ok);
    add_test_with_context(suite, MOCK_DB_POOL, app_model_pool_signal_closing_test);
    add_test_with_context(suite, MOCK_DB_POOL, app_model_poolmap_close_conns_test);
    add_test_with_context(suite, MOCK_DB_POOL, app_model_poolmap_check_conns_closed_test);
    return suite;
}
