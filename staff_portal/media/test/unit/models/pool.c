#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include "models/pool.h"

static  DBA_RES_CODE mock_db_conn_init(db_conn_t *conn, db_pool_t *pool)
{ return (DBA_RES_CODE)mock(conn, pool); }

static  DBA_RES_CODE mock_db_conn_deinit(db_conn_t *conn)
{ return (DBA_RES_CODE)mock(conn); }

static  DBA_RES_CODE mock_db_conn_close(db_conn_t *conn, dba_done_cb done_cb)
{ return (DBA_RES_CODE)mock(conn, done_cb); }

static  DBA_RES_CODE mock_db_conn_connect(db_conn_t *conn, dba_done_cb done_cb)
{ return (DBA_RES_CODE)mock(conn, done_cb); }

Ensure(app_model_pool_init_missing_arg_test) {
    db_pool_cfg_t cfg_opts = {
        .alias="db_primary", .capacity=4, .idle_timeout=80, .close_cb=NULL,
        .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_123"},
        .conn_ops = {.init_fn = mock_db_conn_init}
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
        .alias="db_primary", .capacity=5, .idle_timeout=80, .close_cb=NULL,
        .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
            .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
        .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
            .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
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
    assert_that(pool_found->conns, is_not_null);
    assert_traverse_linklist(pool_found->conns, cfg_opts.capacity);
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
        .alias="db_primary", .capacity=5, .idle_timeout=80, .close_cb=NULL,
        .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
            .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
        .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
            .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
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
            .alias="db_primary", .capacity=2, .idle_timeout=80, .close_cb=NULL,
            .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
                .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
            .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
        },
        {
            .alias="db_primary", .capacity=3, .idle_timeout=100, .close_cb=NULL,
            .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_456", .db_user="username123",
                .db_passwd="password", .db_host="itest.myhost.com", .db_port=1987 },
            .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
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
    assert_traverse_linklist(pool_found->conns, cfg_opts[0].capacity);
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
            .alias="db_primary", .capacity=2, .idle_timeout=80, .close_cb=NULL,
            .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_123", .db_user="username",
                .db_passwd="password", .db_host="utest.myhost.com", .db_port=1234 },
            .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
        },
        {
            .alias="db_replica_1", .capacity=3, .idle_timeout=100, .close_cb=NULL,
            .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_456", .db_user="bob",
                .db_passwd="uncle", .db_host="itest.myhost.com", .db_port=1987 },
            .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
        },
        {
            .alias="db_repli2", .capacity=4, .idle_timeout=147, .close_cb=NULL,
            .error_cb=NULL, .conn_detail={.db_name = "ecommerce_media_458", .db_user="alice",
                .db_passwd="dreammaker", .db_host="itest.myhost.com", .db_port=1987 },
            .conn_ops = {.init_fn = mock_db_conn_init, .deinit_fn = mock_db_conn_deinit,
                .close_fn = mock_db_conn_close, .connect_fn = mock_db_conn_connect}
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
        assert_traverse_linklist(pool_found->conns, cfg_opts[idx].capacity);
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


TestSuite *app_model_pool_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_model_pool_init_missing_arg_test);
    add_test(suite, app_model_pool_init_one_test_ok);
    add_test(suite, app_model_pool_init_one_test_error_init_conns);
    add_test(suite, app_model_pool_init_duplicate_error);
    add_test(suite, app_model_pool_init_many_test_ok);
    // add_test(suite, app_model_pool_get_conn_test);
    // add_test(suite, app_model_pool_capacity_test);
    return suite;
}
