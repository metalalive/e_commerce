#include <pthread.h>
#include <cgreen/cgreen.h>
#undef is_null  // workaround
//// NOTE: the macro `is_null` in `cgreen/cgreen.h` conflicts the same name in `mysql.h`
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <mysql.h>
#include <mysqld_error.h>
#include <h2o/memory.h>

#include "models/connection.h"
#include "models/mariadb.h"

#define CONN_BULK_QUERY_LIMIT_KB 1
#define CALLED_BY_APP 0, 0


typedef struct {
    db_llnode_t  node;
    db_query_t   query;
} db_query_extend_t;

static int mock_app__timerpoll_init(uv_loop_t *loop, app_timer_poll_t *handle, int fd)
{ return (int) mock(loop, handle, fd); }

static int mock_app__timerpoll_deinit(app_timer_poll_t *handle)
{ return (int) mock(handle); }

static int mock_app__timerpoll_change_fd(app_timer_poll_t *handle, int fd)
{ return (int) mock(handle, fd); }

static int mock_app__timerpoll_start(app_timer_poll_t *handle, uint64_t timeout_ms,
        uint32_t event_flags, timerpoll_poll_cb poll_cb)
{ return (int) mock(handle, timeout_ms, event_flags, poll_cb); }

static int mock_app__timerpoll_stop(app_timer_poll_t *handle)
{ return (int) mock(handle); }

static uint8_t  mock_db_pool__is_closing_fn(struct db_pool_s *pool)
{ return (uint8_t) mock(pool); }

static DBA_RES_CODE mock_db_conn__update_ready_queries(struct db_conn_s *conn)
{ return (DBA_RES_CODE) mock(conn); }

static __attribute__((optimize("O0"))) void  mock_db_query__notify_callback(uv_async_t *handle)
{
    db_query_t  *q_found = H2O_STRUCT_FROM_MEMBER(db_query_t, notification, handle);
    db_llnode_t *curr_node = NULL;
    db_llnode_t *next_node = NULL;
    for(curr_node = q_found->db_result.head ; curr_node; curr_node = next_node)
    {
        db_query_result_t *rs = (db_query_result_t *) &curr_node->data[0];
        enum _dbconn_async_state conn_state = rs->conn.state;
        DBA_RES_CODE  app_result = rs->app_result;
        uint8_t  is_async = rs->conn.async;
        uint8_t  is_final = rs->_final;
        db_query_row_info_t *row_info = NULL;
        size_t  num_cols = 0;
        if(conn_state == DB_ASYNC_FETCH_ROW_READY) {
            row_info = (db_query_row_info_t *) &rs->data[0];
        }
        if(row_info) {
            num_cols = row_info->num_cols;
        }
        mock(q_found, app_result, conn_state, is_async, is_final, num_cols);
        for(size_t idx = 0; row_info && idx < num_cols; idx++) {
            char *col_value = row_info->values[idx];
            mock(q_found, col_value);
        }
        next_node = curr_node->next;
        rs->free_data_cb(curr_node);
    } // end of loop
    q_found->db_result.head = NULL;
    q_found->db_result.tail = NULL;
} // end of mock_db_query__notify_callback


Ensure(app_mariadb_test_init_error) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    expect(mysql_init,  will_return(NULL));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_MEMORY_ERROR));
    assert_that(conn.pool, is_equal_to(NULL));
    assert_that(conn.lowlvl.conn, is_equal_to(NULL));
} // end of app_mariadb_test_init_error


Ensure(app_mariadb_test_init_set_option_error) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    MYSQL expect_mysql = {0};
    expect(mysql_init,  will_return(&expect_mysql));
    expect(mysql_close, when(mysql, is_equal_to(&expect_mysql)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
    expect(mysql_options, will_return(1), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_CONFIG_ERROR));
    assert_that(conn.pool, is_equal_to(NULL));
    assert_that(conn.lowlvl.conn, is_equal_to(NULL));
} // end of app_mariadb_test_init_set_option_error


Ensure(app_mariadb_test_init_ok) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    MYSQL expect_mysql = {0};
    expect(mysql_init,  will_return(&expect_mysql));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_WRITE_TIMEOUT)));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    assert_that(conn.pool, is_equal_to(&pool));
    assert_that(conn.lowlvl.conn, is_equal_to(&expect_mysql));
    expect(mysql_close, when(mysql, is_equal_to(&expect_mysql)));
    result = app_db_mariadb_conn_deinit(&conn);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    assert_that(conn.lowlvl.conn, is_equal_to(NULL));
} // end of app_mariadb_test_init_ok

Ensure(app_mariadb_test_acquire_state_change) {
    db_conn_t conn = {0};
    uint8_t allowed = 0;
    { // assume 2 worker threads are contending to perform the state transition whenever
      // the connection state is valid
        enum _dbconn_async_state  valid_states[] = {DB_ASYNC_INITED, DB_ASYNC_CONN_START, DB_ASYNC_QUERY_START};
        size_t num = sizeof(valid_states) / sizeof(enum _dbconn_async_state);
        for(size_t idx = 0; idx < num; idx++) {
            conn.state = (enum _dbconn_async_state) valid_states[idx];
            atomic_flag_clear_explicit(&conn.flags.state_changing, memory_order_relaxed);
            allowed = app_mariadb_acquire_state_change(&conn); // the first worker succeeded
            assert_that(allowed, is_equal_to(1));
            allowed = app_mariadb_acquire_state_change(&conn); // the second worker succeeded
            assert_that(allowed, is_equal_to(0));
            allowed = app_mariadb_acquire_state_change(&conn);
            assert_that(allowed, is_equal_to(0));
        } // end of loop
    }
    {
        enum _dbconn_async_state valid_states[] = {DB_ASYNC_CONN_WAITING, DB_ASYNC_CONN_DONE,
            DB_ASYNC_QUERY_WAITING, DB_ASYNC_QUERY_READY, DB_ASYNC_CHECK_CURRENT_RESULTSET,
            DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START, DB_ASYNC_FETCH_ROW_START, DB_ASYNC_FREE_RESULTSET_START,
            DB_ASYNC_CLOSE_START, DB_ASYNC_CLOSE_WAITING, DB_ASYNC_CLOSE_DONE,
        };
        size_t num = sizeof(valid_states) / sizeof(enum _dbconn_async_state);
        for(size_t idx = 0; idx < num; idx++) {
            allowed = app_mariadb_acquire_state_change(&conn); // none of workers can transit the state
            assert_that(allowed, is_equal_to(0));
        } // end of loop
    }
} // end of app_mariadb_test_acquire_state_change


Ensure(app_mariadb_test_start_connection_failure) {
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB},
        .is_closing_fn = mock_db_pool__is_closing_fn };
    db_conn_t conn = {.pool = &pool, .ops = {.timerpoll_stop = mock_app__timerpoll_stop},
        .processing_queries = NULL, .pending_queries = {.head = NULL}, .lowlvl = {0},
        .state = DB_ASYNC_INITED
    };
    MYSQL  expect_mysql = {0};
    MYSQL *mysql_conn_ret = NULL;
    { // assume connection error on client side
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
        expect(mysql_real_connect_start, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_conn_ret, sizeof(MYSQL *)));
        // expect to invoke following sequence of functions 
        expect(mysql_errno, will_return(ER_TOO_MANY_USER_CONNECTIONS));
        expect(mock_app__timerpoll_stop, will_return(0));
        // assume error happenes again when closing connection
        expect(mysql_close_start, will_return(0));
        expect(mysql_errno, will_return(ER_NET_READ_ERROR_FROM_PIPE));
        expect(mock_app__timerpoll_stop, will_return(0));
        // assume the app has NOT been closing the conneciton pool , reinit low-level handle
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mysql_init,  will_return(&expect_mysql));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_WRITE_TIMEOUT)));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, CALLED_BY_APP);
        assert_that(conn.state, is_equal_to(DB_ASYNC_CONN_START));
        assert_that(conn.lowlvl.conn, is_equal_to(&expect_mysql));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    }
} // end of  app_mariadb_test_start_connection_failure

Ensure(app_mariadb_test_connect_db_server_error) {
    uv_loop_t loop = {0};
    MYSQL  expect_mysql[2] = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,
        .ops = { .get_sock_fd = app_db_mariadb_get_sock_fd,
            .state_transition = app_mariadb_async_state_transition_handler,
            .get_timeout_ms = app_db_mariadb_get_timeout_ms}},
        .is_closing_fn = mock_db_pool__is_closing_fn };
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = NULL,
        .pending_queries = {.head = NULL, .tail = NULL},  .state = DB_ASYNC_INITED,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
            .timerpoll_init = mock_app__timerpoll_init},
        .lowlvl = {.conn = &expect_mysql[0]}
    };
    int    expect_lowlvl_fd = 123;
    uint64_t expect_timeout_ms = 560;
    MYSQL *mysql_conn_ret = NULL;
    { // assume connection packet is sent successfully  on client side
        mysql_conn_ret = (MYSQL *) &conn.lowlvl.conn;
        assert_that(conn.state, is_equal_to(DB_ASYNC_INITED));
        expect(mysql_real_connect_start, will_return(MYSQL_WAIT_READ),
                will_set_contents_of_parameter(ret, &mysql_conn_ret, sizeof(MYSQL *)));
        expect(mysql_get_socket, will_return(expect_lowlvl_fd));
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_init, will_return(0),  when(loop, is_equal_to(&loop)),
                when(handle, is_equal_to(&conn.timer_poll)),
                when(fd, is_equal_to(expect_lowlvl_fd)) );
        expect(mock_app__timerpoll_start, will_return(0),
                when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE)),
                when(poll_cb,     is_equal_to(pool.cfg.ops.state_transition)) );
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, CALLED_BY_APP);
        assert_that(conn.state, is_equal_to(DB_ASYNC_CONN_WAITING));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
    }
    { // assume db sevrer returns connection error
        mysql_conn_ret = (MYSQL *) NULL;
        assert_that(conn.state, is_equal_to(DB_ASYNC_CONN_WAITING));
        expect(mysql_real_connect_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_conn_ret, sizeof(MYSQL *)));
        expect(mysql_errno, will_return(ER_TOO_MANY_USER_CONNECTIONS));
        expect(mock_app__timerpoll_stop, will_return(0));
        // assume there's no pending or processing query in this case. Start closing connection
        expect(mysql_close_start, will_return(MYSQL_WAIT_READ),
                when(sock, is_equal_to((MYSQL *)conn.lowlvl.conn)));
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),
                when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE)),
                when(poll_cb,     is_equal_to(pool.cfg.ops.state_transition)) );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_CLOSE_WAITING));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
    }
    { // mariadb sevrer always returns success on closing connection.
        assert_that(conn.state, is_equal_to(DB_ASYNC_CLOSE_WAITING));
       // Go straight to re-init low-level handle
        expect(mysql_close_cont, will_return(0));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mysql_init,  will_return(&expect_mysql[1]));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
        expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_WRITE_TIMEOUT)));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, CALLED_BY_APP);
        assert_that(conn.state, is_equal_to(DB_ASYNC_CONN_START));
        assert_that(conn.lowlvl.conn, is_equal_to(&expect_mysql[1]));
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    }
} // end of app_mariadb_test_connect_db_server_error


Ensure(app_mariadb_test_evict_all_queries_on_connection_failure) {
    db_query_extend_t  mock_pending_nodes[3] = {0};
    db_query_extend_t  mock_processing_nodes[3] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB, .alias = "unitest_db",
        .ops = {.state_transition = app_mariadb_async_state_transition_handler,
            .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .pending_queries = {.head = &mock_pending_nodes[0].node, .tail = &mock_pending_nodes[2].node},
        .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .timerpoll_init = mock_app__timerpoll_init},
    };
    MYSQL *mysql_conn_ret = NULL;
    { // init mock queries
        mock_pending_nodes[0].node = (db_llnode_t){.prev = NULL, .next = &mock_pending_nodes[1].node};
        mock_pending_nodes[1].node = (db_llnode_t){.prev = &mock_pending_nodes[0].node, .next = &mock_pending_nodes[2].node};
        mock_pending_nodes[2].node = (db_llnode_t){.prev = &mock_pending_nodes[1].node, .next = NULL};
        mock_processing_nodes[0].node = (db_llnode_t){.prev = NULL, .next = &mock_processing_nodes[1].node};
        mock_processing_nodes[1].node = (db_llnode_t){.prev = &mock_processing_nodes[0].node, .next = &mock_processing_nodes[2].node};
        mock_processing_nodes[2].node = (db_llnode_t){.prev = &mock_processing_nodes[1].node, .next = NULL};
        db_llnode_t *q_node = NULL;
        for(q_node = conn.processing_queries; q_node; q_node = q_node->next) {
            db_query_t *q = (db_query_t *)&q_node->data[0];
            *q = (db_query_t) {.cfg = {.loop = &loop}, .notification = {.async_cb = mock_db_query__notify_callback }};
        }
        for(q_node = conn.pending_queries.head; q_node; q_node = q_node->next) {
            db_query_t *q = (db_query_t *)&q_node->data[0];
            *q = (db_query_t) {.cfg = {.loop = &loop}, .notification = {.async_cb = mock_db_query__notify_callback }};
        }
    }
    // assume current state of a connection is waiting for CONNECT packet
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    conn.state = DB_ASYNC_CONN_WAITING;
    { // assume there are pending and processing queries in this case. Evict them
        expect(mysql_real_connect_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_conn_ret, sizeof(MYSQL *)));
        expect(mysql_errno, will_return(ER_SERVER_SHUTDOWN));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_query__notify_callback,  when(app_result, is_equal_to(DBA_RESULT_NETWORK_ERROR)),
                when(conn_state, is_equal_to(DB_ASYNC_CONN_DONE)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(1)),  when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        expect(mock_db_query__notify_callback, when(q_found, is_equal_to(&mock_processing_nodes[1].node.data)));
        expect(mock_db_query__notify_callback, when(q_found, is_equal_to(&mock_processing_nodes[2].node.data)));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_NETWORK_ERROR)),
                when(conn_state, is_equal_to(DB_ASYNC_CONN_DONE)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(1)), when(q_found, is_equal_to(&mock_pending_nodes[0].node.data)));
        expect(mock_db_query__notify_callback, when(q_found, is_equal_to(&mock_pending_nodes[1].node.data)));
        expect(mock_db_query__notify_callback, when(q_found, is_equal_to(&mock_pending_nodes[2].node.data)));
        expect(mysql_close_start, will_return(MYSQL_WAIT_READ));
        expect(mysql_get_timeout_value_ms, will_return(123));
        expect(mock_app__timerpoll_start, will_return(0));
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
    }
    assert_that(conn.state, is_equal_to(DB_ASYNC_CLOSE_WAITING));
} // end of app_mariadb_test_evict_all_queries_on_connection_failure


Ensure(app_mariadb_test_query_failure_local) {
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB, .alias = "unitest_db",
        .ops = {.state_transition = app_mariadb_async_state_transition_handler,
            .get_sock_fd = app_db_mariadb_get_sock_fd, .get_timeout_ms = app_db_mariadb_get_timeout_ms}},
          .is_closing_fn = mock_db_pool__is_closing_fn};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = NULL,
        .pending_queries = {.head = NULL, .tail = NULL},  .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .timerpoll_init = mock_app__timerpoll_init, .timerpoll_deinit = mock_app__timerpoll_deinit,
        .update_ready_queries = mock_db_conn__update_ready_queries},
    };
    { // assume no more pending query when transitting connection state
        conn.state = DB_ASYNC_QUERY_START;
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
        expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_SKIPPED));
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mock_app__timerpoll_deinit, will_return(0));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, CALLED_BY_APP);
        assert_that(conn.state, is_equal_to(DB_ASYNC_QUERY_START)); // state NOT changed
    }
    { // assume error happenes in timer-poll handle
        int expect_lowlvl_fd = 125;
        uint64_t expect_timeout_ms = 166;
        int mysql_query_ret = 0;
        db_query_extend_t  mock_processing_nodes[2] = {0};
        mock_processing_nodes[0].node = (db_llnode_t){.prev = NULL, .next = &mock_processing_nodes[1].node};
        mock_processing_nodes[1].node = (db_llnode_t){.prev = &mock_processing_nodes[0].node, .next = NULL};
        conn.state = DB_ASYNC_QUERY_START;
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
        conn.processing_queries = &mock_processing_nodes[0].node;
        db_llnode_t *q_node = NULL;
        for(q_node = conn.processing_queries; q_node; q_node = q_node->next) {
            db_query_t *q = (db_query_t *)&q_node->data[0];
            *q = (db_query_t) {.cfg = {.loop = &loop}, .notification = {.async_cb = mock_db_query__notify_callback }};
        }
        expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_OK));
        expect(mysql_real_query_start, will_return(MYSQL_WAIT_READ), 
                will_set_contents_of_parameter(ret, &mysql_query_ret, sizeof(int *))  );
        expect(mysql_get_socket, will_return(expect_lowlvl_fd));
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_init, will_return(0), when(fd, is_equal_to(expect_lowlvl_fd)) );
        expect(mock_app__timerpoll_start, will_return(UV_EPERM),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_OS_ERROR)),
                when(conn_state, is_equal_to(DB_ASYNC_QUERY_READY)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(1)), when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        expect(mock_db_query__notify_callback, when(q_found, is_equal_to(&mock_processing_nodes[1].node.data)));
        expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_SKIPPED));
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mock_app__timerpoll_deinit, will_return(0));
        app_mariadb_async_state_transition_handler(&conn.timer_poll, CALLED_BY_APP);
        assert_that(conn.state, is_equal_to(DB_ASYNC_QUERY_START)); // state NOT changed
        assert_that(conn.processing_queries, is_equal_to(NULL));
        assert_that(conn.pending_queries.head, is_equal_to(NULL));
        // assume next worker thread is trying to invoke the function
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    }
} // end of app_mariadb_test_query_failure_local


Ensure(app_mariadb_test_query_failure_remote) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB, .ops = {0}},
          .is_closing_fn = mock_db_pool__is_closing_fn};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .pending_queries = {.head = NULL, .tail = NULL},  .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .timerpoll_init = mock_app__timerpoll_init, .timerpoll_deinit = mock_app__timerpoll_deinit,
        .update_ready_queries = mock_db_conn__update_ready_queries},
    };
    {
        db_query_t *q = (db_query_t *) & mock_processing_nodes[0].node.data[0];
        *q = (db_query_t) {.cfg = {.loop = &loop}, .notification = {.async_cb = mock_db_query__notify_callback }};
    }
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    { // assume local app sent the queries successfully
        int mysql_query_ret = 1; // error happened, without detail code
        conn.state = DB_ASYNC_QUERY_WAITING;
        expect(mysql_real_query_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_query_ret, sizeof(int *)));
        expect(mysql_errno, will_return(ER_TOO_MANY_FIELDS));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_MEMORY_ERROR)),
                when(conn_state, is_equal_to(DB_ASYNC_QUERY_READY)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(1)), when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_SKIPPED));
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mock_app__timerpoll_deinit, will_return(0));
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_QUERY_START));
        assert_that(conn.processing_queries, is_equal_to(NULL));
    }
} // end of app_mariadb_test_query_failure_remote


Ensure(app_mariadb_test_query_resultset_no_rows) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .pending_queries = {.head = NULL, .tail = NULL},  .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start},
    };
    pthread_mutex_init(&conn.lock, NULL);
    {
        db_query_t *q = (db_query_t *) & mock_processing_nodes[0].node.data[0];
        *q = (db_query_t) {.cfg = {.loop = &loop}, .db_result = {.num_rs_remain = 0},
            .notification = {.async_cb = mock_db_query__notify_callback }};
    }
    { // assume local app sent the queries successfully
        int mysql_query_ret = 0; // assume remote DB server completed query successfully
        int mysql_nxt_rs_ret = 0; // we don't know if there's any result set to fetch yet
        int expect_timeout_ms = 839;
        conn.state = DB_ASYNC_QUERY_WAITING;
        expect(mysql_real_query_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_query_ret, sizeof(int *)));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mysql_use_result, will_return(NULL));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_END_OF_ROWS_REACHED)),
                when(conn_state, is_equal_to(DB_ASYNC_CHECK_CURRENT_RESULTSET)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(1)), when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        expect(mysql_next_result_start, will_return(MYSQL_WAIT_READ), 
                will_set_contents_of_parameter(ret, &mysql_nxt_rs_ret, sizeof(int *))  );
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        int evt_flgs = UV_READABLE | UV_WRITABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING));
        assert_that(conn.processing_queries, is_equal_to(NULL));
    }
    pthread_mutex_destroy(&conn.lock);
} // end of app_mariadb_test_query_resultset_no_rows


Ensure(app_mariadb_test_query_next_resultset_found) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start},
        .state = DB_ASYNC_INITED,  .lowlvl = {0},
    };
    MYSQL_RES  mysql_res = {0};
    {
        db_query_t *q = (db_query_t *) & mock_processing_nodes[0].node.data[0];
        *q = (db_query_t) {.cfg = {.loop = &loop}, .db_result = {.num_rs_remain = 3},
            .notification = {.async_cb = mock_db_query__notify_callback }};
    }
    { // assume there is next result set after the recent query is executed on DB server
        int mysql_query_ret = 0;
        int expect_timeout_ms = 166;
        size_t expect_num_affect_rows = 15;
        MYSQL_ROW  mysql_row = NULL;
        conn.state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING;
        expect(mysql_next_result_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_query_ret, sizeof(int *))  );
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mysql_use_result, will_return(&mysql_res));
        expect(mysql_affected_rows, will_return(expect_num_affect_rows));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_OK)),
                when(conn_state, is_equal_to(DB_ASYNC_CHECK_CURRENT_RESULTSET)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(0)), when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        expect(mysql_fetch_row_start, will_return(MYSQL_WAIT_READ),
                will_set_contents_of_parameter(ret, &mysql_row, sizeof(MYSQL_ROW *))  );
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_FETCH_ROW_WAITING));
    }
} // end of app_mariadb_test_query_next_resultset_found


Ensure(app_mariadb_test_query_reach_end_of_resultsets) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
            .update_ready_queries = mock_db_conn__update_ready_queries}, .state = DB_ASYNC_INITED};
    {
        uint64_t expect_timeout_ms = 149;
        int mysql_nxt_rs_ret = -1; // no more result set in current query execution
        int mysql_query_ret = 0;
        conn.state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING;
        expect(mysql_next_result_cont, will_return(0),
                will_set_contents_of_parameter(ret, &mysql_nxt_rs_ret, sizeof(int *))  );
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_OK)); // assume there is new pending query
        expect(mysql_real_query_start, will_return(MYSQL_WAIT_READ), 
                will_set_contents_of_parameter(ret, &mysql_query_ret, sizeof(int *))  );
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_QUERY_WAITING));
    }
} // end of app_mariadb_test_query_reach_end_of_resultsets


Ensure(app_mariadb_test_rs_fetch_a_row) {
#define  EXPECT_NUM_COLUMNS  3
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        }, .state = DB_ASYNC_INITED,
    };
    {
        db_query_t *q = (db_query_t *) & mock_processing_nodes[0].node.data[0];
        *q = (db_query_t) {.cfg = {.loop = &loop}, .db_result = {.num_rs_remain = 1},
            .notification = {.async_cb = mock_db_query__notify_callback }};
    }
    {
        const char *expect_columns[EXPECT_NUM_COLUMNS] = {"oauth2", "gRPC", "docker"};
        MYSQL_ROW  expect_mysql_rows[2] = {(MYSQL_ROW)expect_columns, NULL};
        uint64_t expect_timeout_ms = 80;
        conn.state = DB_ASYNC_FETCH_ROW_WAITING;
        expect(mysql_fetch_row_cont, will_return(0),
                will_set_contents_of_parameter(ret, &expect_mysql_rows[0], sizeof(MYSQL_ROW *))  );
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mysql_num_fields, will_return(EXPECT_NUM_COLUMNS));
        expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_OK)),
                when(conn_state, is_equal_to(DB_ASYNC_FETCH_ROW_READY)), when(is_async, is_equal_to(0)),
                when(is_final, is_equal_to(0)), when(num_cols, is_equal_to(EXPECT_NUM_COLUMNS)),
                when(q_found, is_equal_to(&mock_processing_nodes[0].node.data)));
        for(size_t idx = 0; idx < EXPECT_NUM_COLUMNS; idx++) {
            expect(mock_db_query__notify_callback, when(col_value,
                        is_equal_to_string(expect_mysql_rows[0][idx])));
        }
        expect(mysql_fetch_row_start, will_return(MYSQL_WAIT_READ),
                will_set_contents_of_parameter(ret, &expect_mysql_rows[1], sizeof(MYSQL_ROW *))  );
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_FETCH_ROW_WAITING));
    }
#undef  EXPECT_NUM_COLUMNS
} // end of app_mariadb_test_rs_fetch_a_row


Ensure(app_mariadb_test_rs_end_of_row) {
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .state = DB_ASYNC_INITED,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start},
    };
    { // for mariadb, end-of-row situation can be identified when (1) the handle cannot
       // fetch new row (2) no error code returned from mysql_errno()
        uint64_t expect_timeout_ms = 364;
        MYSQL_ROW  expect_mysql_row = NULL;
        conn.state = DB_ASYNC_FETCH_ROW_WAITING;
        expect(mysql_fetch_row_cont, will_return(0),
                will_set_contents_of_parameter(ret, &expect_mysql_row, sizeof(MYSQL_ROW *))  );
        expect(mysql_errno, will_return(0));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mysql_free_result_start, will_return(MYSQL_WAIT_READ));
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE))  );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_FREE_RESULTSET_WAITING));
    }
} // end of app_mariadb_test_rs_end_of_row


static void _app_mariadb_test_free_resultset_iteration(db_conn_t *conn, db_query_t *expect_q, uint8_t _is_final) {
    uint64_t expect_timeout_ms = 50;
    int mysql_nxt_rs_ret = 0; // we don't know if there's any result set to fetch yet
    conn->state = DB_ASYNC_FREE_RESULTSET_WAITING;
    expect(mysql_free_result_cont, will_return(0));
    expect(mock_app__timerpoll_stop, will_return(0));
    expect(mock_db_query__notify_callback, when(app_result, is_equal_to(DBA_RESULT_OK)),
            when(conn_state, is_equal_to(DB_ASYNC_FREE_RESULTSET_DONE)), when(is_async, is_equal_to(0)),
            when(is_final, is_equal_to(_is_final)), when(q_found, is_equal_to(expect_q)));
    expect(mysql_next_result_start, will_return(MYSQL_WAIT_READ), 
            will_set_contents_of_parameter(ret, &mysql_nxt_rs_ret, sizeof(int *))  );
    expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
    expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
            when(event_flags, is_equal_to(UV_READABLE))  );
    int evt_flgs = UV_READABLE;
    int uv_status = 0;
    app_mariadb_async_state_transition_handler(&conn->timer_poll, uv_status, evt_flgs);
    assert_that(conn->state, is_equal_to(DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING));
    assert_that(app_mariadb_acquire_state_change(conn), is_equal_to(0));
} // end of _app_mariadb_test_free_resultset_iteration

Ensure(app_mariadb_test_free_resultset) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .processing_queries = &mock_processing_nodes[0].node,
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .update_ready_queries = mock_db_conn__update_ready_queries}, .state = DB_ASYNC_INITED,
    };
    {
        db_query_t *q = (db_query_t *) & mock_processing_nodes[0].node.data[0];
        *q = (db_query_t) {.cfg = {.loop = &loop}, .db_result = {.num_rs_remain = 3},
            .notification = {.async_cb = mock_db_query__notify_callback }};
    }
    pthread_mutex_init(&conn.lock, NULL);
    { // assume the query still has more result set to load
        assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
        db_query_t *expect_q = (db_query_t *)&mock_processing_nodes[0].node.data;
        _app_mariadb_test_free_resultset_iteration(&conn, expect_q, 0);
        assert_that(conn.processing_queries, is_equal_to(&mock_processing_nodes[0].node));
        _app_mariadb_test_free_resultset_iteration(&conn, expect_q, 0);
        assert_that(conn.processing_queries, is_equal_to(&mock_processing_nodes[0].node));
        _app_mariadb_test_free_resultset_iteration(&conn, expect_q, 1);
        assert_that(conn.processing_queries, is_equal_to(NULL));
    }
    pthread_mutex_destroy(&conn.lock);
} // end of app_mariadb_test_free_resultset


Ensure(app_mariadb_test_deinit_start) {
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB, .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}, .is_closing_fn = mock_db_pool__is_closing_fn};
    db_conn_t conn = {.pool = &pool, .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .update_ready_queries = mock_db_conn__update_ready_queries},
    };
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    uint64_t expect_timeout_ms = 143;
    conn.state = DB_ASYNC_QUERY_START;
    expect(mock_db_conn__update_ready_queries, will_return(DBA_RESULT_SKIPPED)); // assume no more pending query
    expect(mock_db_pool__is_closing_fn, will_return(1));
    expect(mysql_close_start, will_return(MYSQL_WAIT_READ));
    expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
    expect(mock_app__timerpoll_start, will_return(0),  when(timeout_ms,  is_equal_to(expect_timeout_ms)),
            when(event_flags, is_equal_to(UV_READABLE))  );
    int evt_flgs = UV_READABLE;
    int uv_status = 0;
    app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
    assert_that(conn.state, is_equal_to(DB_ASYNC_CLOSE_WAITING));
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
} // end of app_mariadb_test_deinit_start


Ensure(app_mariadb_test_deinit_ok) {
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB, .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms}}, .is_closing_fn = mock_db_pool__is_closing_fn};
    db_conn_t conn = {.pool = &pool, .state = DB_ASYNC_INITED,  .lowlvl = {0},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_deinit = mock_app__timerpoll_deinit},
    };
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    conn.state = DB_ASYNC_CLOSE_WAITING;
    expect(mysql_close_cont, will_return(0));
    expect(mock_app__timerpoll_stop, will_return(0));
    expect(mock_db_pool__is_closing_fn, will_return(1));
    expect(mock_app__timerpoll_deinit, will_return(0));
    int evt_flgs = UV_READABLE;
    int uv_status = 0;
    app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
    assert_that(conn.state, is_equal_to(DB_ASYNC_CLOSE_DONE));
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
} // end of app_mariadb_test_deinit_ok


Ensure(app_mariadb_test_reconnecting) {
    db_query_extend_t  mock_processing_nodes[1] = {0};
    uv_loop_t loop = {0};
    MYSQL  old_mysql_handle = {0};
    MYSQL  expect_mysql_handle = {0};
    db_pool_t pool = {.cfg = {.bulk_query_limit_kb = CONN_BULK_QUERY_LIMIT_KB,  .ops = {
        .get_timeout_ms = app_db_mariadb_get_timeout_ms, .get_sock_fd = app_db_mariadb_get_sock_fd}
         }, .is_closing_fn = mock_db_pool__is_closing_fn};
    db_conn_t conn = {.pool = &pool, .loop = &loop, .state = DB_ASYNC_INITED, .lowlvl = {.conn = &old_mysql_handle},
        .pending_queries = {.head = &mock_processing_nodes[0].node , .tail = &mock_processing_nodes[0].node},
        .ops = {.timerpoll_stop = mock_app__timerpoll_stop, .timerpoll_start = mock_app__timerpoll_start,
        .timerpoll_change_fd = mock_app__timerpoll_change_fd},
    };
    MYSQL *mysql_conn_ret = NULL;
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(1));
    {
        int expect_lowlvl_fd = 39;
        uint64_t expect_timeout_ms = 180;
        conn.state = DB_ASYNC_CLOSE_WAITING;
        expect(mysql_close_cont, will_return(0));
        expect(mock_app__timerpoll_stop, will_return(0));
        expect(mock_db_pool__is_closing_fn, will_return(0));
        expect(mysql_init, will_return(&expect_mysql_handle));
        expect(mysql_options, will_return(0), when(mysql, is_equal_to(&expect_mysql_handle)),
                when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
        expect(mysql_options, will_return(0), when(mysql, is_equal_to(&expect_mysql_handle)),
                when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
        expect(mysql_options, will_return(0), when(mysql, is_equal_to(&expect_mysql_handle)),
                when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
        expect(mysql_options, will_return(0), when(mysql, is_equal_to(&expect_mysql_handle)),
                when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
        expect(mysql_options, will_return(0), when(mysql, is_equal_to(&expect_mysql_handle)),
                when(option, is_equal_to(MYSQL_OPT_WRITE_TIMEOUT)));
        expect(mysql_real_connect_start, will_return(MYSQL_WAIT_READ),
                will_set_contents_of_parameter(ret, &mysql_conn_ret, sizeof(MYSQL *)));
        expect(mysql_get_socket, will_return(expect_lowlvl_fd));
        expect(mysql_get_timeout_value_ms, will_return(expect_timeout_ms));
        expect(mock_app__timerpoll_change_fd, will_return(0), when(handle, is_equal_to(&conn.timer_poll)),
                when(fd, is_equal_to(expect_lowlvl_fd)) );
        expect(mock_app__timerpoll_start, will_return(0),
                when(timeout_ms,  is_equal_to(expect_timeout_ms)),
                when(event_flags, is_equal_to(UV_READABLE)),
                when(poll_cb,     is_equal_to(pool.cfg.ops.state_transition)) );
        int evt_flgs = UV_READABLE;
        int uv_status = 0;
        app_mariadb_async_state_transition_handler(&conn.timer_poll, uv_status, evt_flgs);
        assert_that(conn.state, is_equal_to(DB_ASYNC_CONN_WAITING));
        assert_that(conn.lowlvl.conn, is_equal_to(&expect_mysql_handle));
    }
    assert_that(app_mariadb_acquire_state_change(&conn), is_equal_to(0));
} // end of app_mariadb_test_reconnecting


static void  mock_db_query__resultset_rdy(struct db_query_s *target, db_query_result_t *detail)
{ mock(target, detail); }

static void  mock_db_query__row_fetched(struct db_query_s *target, db_query_result_t *detail)
{ mock(target, detail); }

static void  mock_db_query__resultset_free(struct db_query_s *target, db_query_result_t *detail)
{ mock(target, detail); }

static void  mock_db_query__error_handler(struct db_query_s *target, db_query_result_t *detail)
{ mock(target, detail); }

Ensure(app_mariadb_test_notify_query_callback) {
    db_query_t query = {.cfg = {.callbacks = {.result_rdy = mock_db_query__resultset_rdy,
        .row_fetched = mock_db_query__row_fetched, .result_free = mock_db_query__resultset_free,
        .error = mock_db_query__error_handler
    }}};
    db_query_result_t  rs = {0};
    uint8_t final = 0;
    {
        rs.conn.state = DB_ASYNC_CHECK_CURRENT_RESULTSET;
        rs._final = 1;
        expect(mock_db_query__resultset_rdy);
        final = app_mariadb_conn_notified_query_callback(&query, &rs);
        assert_that(final, is_equal_to(rs._final));
    }
    {
        rs.conn.state = DB_ASYNC_FETCH_ROW_READY;
        rs._final = 0;
        expect(mock_db_query__row_fetched);
        final = app_mariadb_conn_notified_query_callback(&query, &rs);
        assert_that(final, is_equal_to(rs._final));
    }
    {
        rs.conn.state = DB_ASYNC_FREE_RESULTSET_DONE;
        rs._final = 1;
        expect(mock_db_query__resultset_free);
        final = app_mariadb_conn_notified_query_callback(&query, &rs);
        assert_that(final, is_equal_to(rs._final));
    }
    { // assume any error happened during state transition
        rs.conn.state = DB_ASYNC_CONN_DONE;
        rs.app_result = DBA_RESULT_REMOTE_RESOURCE_ERROR;
        rs._final = 0;
        expect(mock_db_query__error_handler);
        final = app_mariadb_conn_notified_query_callback(&query, &rs);
        assert_that(final, is_equal_to(1));
    }
} // end of app_mariadb_test_notify_query_callback


TestSuite *app_model_mariadb_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_mariadb_test_init_error);
    add_test(suite, app_mariadb_test_init_set_option_error);
    add_test(suite, app_mariadb_test_init_ok);
    add_test(suite, app_mariadb_test_acquire_state_change);
    add_test(suite, app_mariadb_test_start_connection_failure);
    add_test(suite, app_mariadb_test_connect_db_server_error);
    add_test(suite, app_mariadb_test_evict_all_queries_on_connection_failure);
    add_test(suite, app_mariadb_test_query_failure_local);
    add_test(suite, app_mariadb_test_query_failure_remote);
    add_test(suite, app_mariadb_test_query_resultset_no_rows);
    add_test(suite, app_mariadb_test_query_next_resultset_found);
    add_test(suite, app_mariadb_test_query_reach_end_of_resultsets);
    add_test(suite, app_mariadb_test_rs_fetch_a_row);
    add_test(suite, app_mariadb_test_rs_end_of_row);
    add_test(suite, app_mariadb_test_free_resultset);
    add_test(suite, app_mariadb_test_deinit_start);
    add_test(suite, app_mariadb_test_deinit_ok);
    add_test(suite, app_mariadb_test_reconnecting);
    add_test(suite, app_mariadb_test_notify_query_callback);
    return suite;
}
