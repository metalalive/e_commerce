#include <assert.h>
#include <mysql.h>
#include <mysqld_error.h>
#include <h2o/memory.h>

#include "models/connection.h"
#include "models/query.h"
#include "models/mariadb.h"

static int _app_mariadb_convert_evt_from_uv(int uv_status, int uv_evts)
{
    int my_evts = 0;
    if(uv_evts & UV_READABLE) {
        my_evts |=  MYSQL_WAIT_READ;
    }
    if(uv_evts & UV_WRITABLE) {
        my_evts |= MYSQL_WAIT_WRITE;
    }
    if(uv_status == UV_ETIMEDOUT) {
        my_evts |= MYSQL_WAIT_TIMEOUT;
    }
    return my_evts;
} // end of _app_mariadb_convert_evt_from_uv


static int  _app_mariadb_convert_evt_to_uv(int my_evts)
{
    int uv_evts = 0;
    if((my_evts & MYSQL_WAIT_READ) || (my_evts & MYSQL_WAIT_EXCEPT)) {
        uv_evts |= UV_READABLE;
    }
    if(my_evts & MYSQL_WAIT_WRITE) {
        uv_evts |= UV_WRITABLE;
    }
    // not able to convert MYSQL_WAIT_TIMEOUT to uv_poll_event ?
    return uv_evts;
} // end of _app_mariadb_convert_evt_to_uv

static DBA_RES_CODE _app_mariadb_async_cont_status_sanity(int uv_status) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(uv_status == UV_EADDRINUSE || uv_status == UV_EADDRNOTAVAIL || uv_status == UV_EAI_AGAIN
            || uv_status == UV_ENETDOWN || uv_status == UV_ENETUNREACH || uv_status == UV_EPROTO)
    {
        result = DBA_RESULT_NETWORK_ERROR;
    } else if(uv_status == UV_EAGAIN || uv_status == UV_EIO || uv_status == UV_EMFILE
            || uv_status == UV_EPIPE) {
        result = DBA_RESULT_OS_ERROR;
    } else if(uv_status == UV_EAI_MEMORY) {
        result = DBA_RESULT_NETWORK_ERROR;
    } else if(uv_status == UV_ETIMEDOUT) {
        // skip , TODO any better design option ?
    } else if(uv_status < 0) {
        result = DBA_RESULT_UNKNOWN_ERROR;
    }
    return result;
} // end of _app_mariadb_async_cont_status_sanity


static void _app_mariadb_error_notify_queries(db_conn_t *conn, db_llnode_t *q_head, DBA_RES_CODE app_result)
{
    db_llnode_t *q_node = NULL;
    for(q_node = q_head; q_node; q_node = q_node->next) {
        // TODO, allocate common shared memory for async handles in these query objects
        db_llnode_t *rs_node = malloc(sizeof(db_llnode_t) + sizeof(db_query_result_t));
        db_query_result_t  *q_ret = (db_query_result_t *)&rs_node->data[0];
        db_query_t *q = (db_query_t *)&q_node->data[0];
        q->db_result.num_rs_remain = 0;
        q_ret->app_result = app_result;
        q_ret->free_data_cb = free;
        q_ret->conn.alias = conn->pool->cfg.alias;
        q_ret->conn.state = conn->state;
        q_ret->conn.async = (conn->loop != q->cfg.loop);
        q_ret->_final = 1;
        app_db_query_notify_with_result(q, q_ret);
    }
} // end of _app_mariadb_error_notify_queries

// error occured at here should be OS-level or network problem, and should NOT be
// specific to certain statement(s) in one of the processing queries.
static void _app_mariadb_error_reset_all_processing_query(db_conn_t *conn, DBA_RES_CODE app_result)
{
    _app_mariadb_error_notify_queries(conn, conn->processing_queries, app_result);
    pthread_mutex_lock(&conn->lock);
    conn->processing_queries = NULL;
    pthread_mutex_unlock(&conn->lock);
} // end of _app_mariadb_error_reset_all_processing_query


static void _app_mariadb_error_reset_all_pending_query(db_conn_t *conn, DBA_RES_CODE app_result)
{   // simply forwarding current error to all the queries registered at the connection
    // by signaling query->async (uv_async_t)
    _app_mariadb_error_notify_queries(conn, conn->pending_queries.head, app_result);
    pthread_mutex_lock(&conn->lock);
    conn->pending_queries.head = NULL;
    conn->pending_queries.tail = NULL;
    pthread_mutex_unlock(&conn->lock);
} // end of _app_mariadb_error_reset_all_pending_query


static DBA_RES_CODE  _app_mariadb_gen_new_handle(MYSQL **handle, uint32_t timeout) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    MYSQL *tmp = mysql_init(NULL);
    if(!tmp) {
        result = DBA_RESULT_MEMORY_ERROR;
        goto error;
    }
    mysql_options(tmp, MYSQL_READ_DEFAULT_GROUP, "async_queries");
    if(mysql_options(tmp, MYSQL_OPT_NONBLOCK, NULL) != 0) {
        result = DBA_RESULT_CONFIG_ERROR;
        goto error;
    }
    if(mysql_options(tmp, MYSQL_OPT_CONNECT_TIMEOUT, &timeout) ||
            mysql_options(tmp, MYSQL_OPT_READ_TIMEOUT, &timeout) ||
            mysql_options(tmp, MYSQL_OPT_WRITE_TIMEOUT, &timeout)) {
        result = DBA_RESULT_CONFIG_ERROR;
        goto error;
    }
    *handle = tmp;
    goto done;
error:
    if(tmp) {
        mysql_close(tmp);
    }
done:
    return result;
} // end of _app_mariadb_gen_new_handle


DBA_RES_CODE app_db_mariadb_conn_init(db_conn_t *conn, db_pool_t *pool)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    MYSQL *handle = NULL;
    if(!conn || !pool) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    result = app_db_conn_init(conn, pool);
    if(result != DBA_RESULT_OK) {
        goto error;
    }
    result = _app_mariadb_gen_new_handle(&handle, conn->pool->cfg.idle_timeout);
    if(result != DBA_RESULT_OK) {
        goto error;
    }
    conn->lowlvl = (db_lowlvl_t){.conn = (void *)handle, .resultset = NULL, .row = NULL};
    goto done;
error:
    app_db_conn_deinit(conn);
done:
    return result;
} // end of app_db_mariadb_conn_init


DBA_RES_CODE app_db_mariadb_conn_deinit(db_conn_t *conn)
{
    if(!conn) {
        return  DBA_RESULT_ERROR_ARG;
    }
    DBA_RES_CODE result = app_db_conn_deinit(conn);
    if(result == DBA_RESULT_OK) {
        // use blocking function, currently this function is supposed to be invoked
        // after app server received shutdown request, already completed
        // all client requests, and closed all the HTTP connections.
        //// mysql_close((MYSQL *)conn->lowlvl.conn);
        conn->lowlvl = (db_lowlvl_t){0};
    }
    return result;
} // end of app_db_mariadb_conn_deinit


static  DBA_RES_CODE _app_mariadb_convert_error_code(MYSQL *handle) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    unsigned int error_code = mysql_errno(handle);
    if(!error_code) {
        // pass
    } else if(error_code == ER_BAD_HOST_ERROR || error_code == ER_DBACCESS_DENIED_ERROR
            || error_code == ER_ACCESS_DENIED_ERROR || error_code == ER_BAD_DB_ERROR
            || error_code == ER_NO_SUCH_TABLE || error_code == ER_WRONG_COLUMN_NAME ) {
        result = DBA_RESULT_ERROR_ARG;
    } else if(error_code == ER_CON_COUNT_ERROR || error_code == ER_SERVER_SHUTDOWN
            || error_code == ER_NORMAL_SHUTDOWN || error_code == ER_ABORTING_CONNECTION
            || error_code == ER_NET_READ_ERROR_FROM_PIPE || error_code == ER_NET_FCNTL_ERROR
            || error_code == ER_NET_READ_ERROR || error_code == ER_NET_READ_INTERRUPTED
            || error_code == ER_NET_ERROR_ON_WRITE || error_code == ER_NET_WRITE_INTERRUPTED) {
        result = DBA_RESULT_NETWORK_ERROR;
    } else if(error_code == ER_USER_LIMIT_REACHED || error_code == ER_TOO_MANY_USER_CONNECTIONS) {
        // use mysql_error() to check detail description
        result = DBA_RESULT_REMOTE_RESOURCE_ERROR;
    } else if(error_code == ER_DISK_FULL || error_code == ER_OUTOFMEMORY
            || error_code == ER_OUT_OF_RESOURCES || error_code == ER_NET_PACKET_TOO_LARGE
            || error_code == ER_TOO_MANY_TABLES || error_code == ER_TOO_MANY_FIELDS
            || error_code == ER_TOO_LONG_STRING) {
        result = DBA_RESULT_MEMORY_ERROR;
    } else {
        result = DBA_RESULT_UNKNOWN_ERROR;
    }
    return result;
} // end of _app_mariadb_convert_error_code


static DBA_RES_CODE app_db_mariadb_conn_connect_start(db_conn_t *conn, int *evt_flgs)
{
    if(!conn || !evt_flgs) {
        return DBA_RESULT_ERROR_ARG;
    }
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_cfg_t *credential = &conn->pool->cfg.conn_detail;
    unsigned long client_flags = CLIENT_REMEMBER_OPTIONS | CLIENT_MULTI_STATEMENTS;
    MYSQL *my_ret = NULL;
    int my_evts = mysql_real_connect_start(&my_ret, (MYSQL *)conn->lowlvl.conn,
           credential->db_host, credential->db_user, credential->db_passwd,
           credential->db_name, (unsigned int)credential->db_port, NULL, client_flags);
    if(my_evts == 0 && !my_ret) { // complete immediately without blocking, there should be error
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn);
    } else { // my_evts should contain event flags, operation sent successfully
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    }
    return result;
} // end of app_db_mariadb_conn_connect_start


static DBA_RES_CODE app_db_mariadb_conn_connect_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        MYSQL *my_ret = NULL;
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_real_connect_cont(&my_ret, (MYSQL *)conn->lowlvl.conn, my_evts);
        if(my_evts) { // needs more time to wait on
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_CONNECTION_BUSY;
        } else {
            if(my_ret) {
                assert(my_ret == conn->lowlvl.conn);
            } else {
                result = DBA_RESULT_NETWORK_ERROR;
            }
        }
    }
    return result;
} // end of app_db_mariadb_conn_connect_cont


static  DBA_RES_CODE app_db_mariadb_conn_send_query_start(db_conn_t *conn, int *evt_flgs)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    int my_err = 0;
    int my_evts = mysql_real_query_start(&my_err, (MYSQL *)conn->lowlvl.conn,
            &conn->bulk_query_rawbytes.data[0], conn->bulk_query_rawbytes.wr_sz);
    if(my_evts == 0 && my_err) {
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn);
    } else {
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    } // end of error handling
    return result;
} // end of app_db_mariadb_conn_send_query_start


static  DBA_RES_CODE app_db_mariadb_conn_send_query_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        int my_err = 0;
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_real_query_cont(&my_err, (MYSQL *)conn->lowlvl.conn, my_evts);
        if(my_evts) { // needs more time to wait on
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_QUERY_STILL_PROCESSING;
        } else if(my_evts == 0 && my_err) {
            result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn);
        }
    }
    return result;
} // end of app_db_mariadb_conn_send_query_cont

#define  _APP_DB_MARIADB_NEXT_RESULTSET___COMMON \
    if(my_ret == 0) { \
        result == DBA_RESULT_OK; \
    } else if (my_ret == -1) { \
        result = DBA_RESULT_END_OF_RSETS_REACHED; \
    } else { \
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn); \
    }

static DBA_RES_CODE  app_db_mariadb_next_resultset_start(db_conn_t *conn, int *evt_flgs)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    int my_ret = 0;
    int my_evts = mysql_next_result_start(&my_ret, (MYSQL *)conn->lowlvl.conn);
    if(my_evts) {
        // 0 and -1 means next result may exist or NOT, other return value means error
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    } else {
        _APP_DB_MARIADB_NEXT_RESULTSET___COMMON
    }
    return result;
} // end of app_db_mariadb_next_resultset_start

static DBA_RES_CODE app_db_mariadb_next_resultset_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        int my_ret = 0;
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_next_result_cont(&my_ret, (MYSQL *)conn->lowlvl.conn, my_evts);
        if(my_evts) { // need more time to wait on
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_RSET_STILL_LOADING;
        } else {
            _APP_DB_MARIADB_NEXT_RESULTSET___COMMON
        }
    }
    return result;
} // end of app_db_mariadb_next_resultset_cont
#undef  _APP_DB_MARIADB_NEXT_RESULTSET___COMMON

#define _APP_DB_MARIADB_FETCH_ROW__COMMON \
    conn->lowlvl.row = (void *) row; \
    if(!row) { \
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn); \
        if(result == DBA_RESULT_OK) { \
            result = DBA_RESULT_END_OF_ROWS_REACHED; \
        } \
    }

static DBA_RES_CODE  app_db_mariadb_fetch_row_start(db_conn_t *conn, int *evt_flgs)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    MYSQL_ROW row = NULL;
    int my_evts = mysql_fetch_row_start(&row, (MYSQL_RES *)conn->lowlvl.resultset);
    if(my_evts) {
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    } else {
        // it is possible that first few rows come within the the result status in the same network
        // packet, in such case, there's no read/write flag to set and application can few
        // the row immediately without waiting
        _APP_DB_MARIADB_FETCH_ROW__COMMON
    }
    return result;
} // end of app_db_mariadb_fetch_row_start

static DBA_RES_CODE app_db_mariadb_fetch_row_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        MYSQL_ROW row = NULL;
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_fetch_row_cont(&row, (MYSQL_RES *)conn->lowlvl.resultset, my_evts);
        if(my_evts) {
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_ROW_STILL_FETCHING;
        } else {
            _APP_DB_MARIADB_FETCH_ROW__COMMON
        }
    }
    return result;
} // end of app_db_mariadb_fetch_row_cont
#undef  _APP_DB_MARIADB_FETCH_ROW__COMMON

static DBA_RES_CODE  app_db_mariadb_free_resultset_start(db_conn_t *conn, int *evt_flgs)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    int my_evts = mysql_free_result_start((MYSQL_RES *)conn->lowlvl.resultset);
    if(my_evts) {
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    } else {
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn);
    }
    return result;
} // end of app_db_mariadb_free_resultset_start

static DBA_RES_CODE  app_db_mariadb_free_resultset_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_free_result_cont((MYSQL_RES *)conn->lowlvl.resultset, my_evts);
        if(my_evts) {
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_REST_RELEASING;
        }
    }
    return result;
} // end of app_db_mariadb_free_resultset_cont

static DBA_RES_CODE app_db_mariadb_conn_close_start(db_conn_t *conn, int *evt_flgs)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    int my_evts = mysql_close_start((MYSQL *)conn->lowlvl.conn);
    *evt_flgs = my_evts;
    if(my_evts) {
        *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
    } else {
        result = _app_mariadb_convert_error_code((MYSQL *)conn->lowlvl.conn);
    }
    return result;
} // end of app_db_mariadb_conn_close_start

static DBA_RES_CODE  app_db_mariadb_conn_close_cont(db_conn_t *conn, int *evt_flgs, int uv_status)
{
    DBA_RES_CODE result = _app_mariadb_async_cont_status_sanity(uv_status);
    if(result == DBA_RESULT_OK) {
        int my_evts = _app_mariadb_convert_evt_from_uv(uv_status, *evt_flgs);
        my_evts = mysql_close_cont((MYSQL *)conn->lowlvl.conn, my_evts);
        if(my_evts) {
            *evt_flgs = _app_mariadb_convert_evt_to_uv(my_evts);
            result = DBA_RESULT_CONNECTION_BUSY;
        }
    }
    return result;
} // end of app_db_mariadb_conn_close_cont

static void _app_mariadb_row_ready_helper(db_conn_t *conn, DBA_RES_CODE app_result)
{
    assert(conn->lowlvl.row);
    size_t num_cols = (size_t)mysql_num_fields(conn->lowlvl.resultset);
    size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t);
    size_t idx = 0;
    if(conn->lowlvl.row && num_cols > 0) {
        size_t row_tot_sz = sizeof(char *) * num_cols;
        for(idx = 0; idx < num_cols; idx++) {
            char *val = ((MYSQL_ROW)conn->lowlvl.row)[idx];
            row_tot_sz += val ? strlen(val) + 1 : 0; // in case the column stored NULL
        }
        rs_node_sz += sizeof(db_query_row_info_t) + row_tot_sz;
    }
    db_query_t *curr_query = (db_query_t *) &conn->processing_queries->data;
    db_llnode_t *rs_node = malloc(rs_node_sz);
    db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
    *rs = (db_query_result_t) {.free_data_cb = free, .app_result = app_result,
            .conn = {.state = conn->state, .alias = conn->pool->cfg.alias,
            .async = (conn->loop != curr_query->cfg.loop)}, ._final = 0};
    {
        db_query_row_info_t *cloned_row = (db_query_row_info_t *) &rs->data[0];
        cloned_row->num_cols = num_cols;
        cloned_row->values = (char **) &cloned_row->data[0];
        char  *d_ptr = &cloned_row->data[sizeof(char *) * num_cols];
        for(idx = 0; idx < num_cols; idx++) {
            char *val = ((MYSQL_ROW)conn->lowlvl.row)[idx];
            if(val) {
                size_t val_sz = strlen(val) + 1; // including NULL character at the end
                memcpy(d_ptr, val, val_sz);
                cloned_row->values[idx] = d_ptr;
                d_ptr += val_sz;
            } else {
                cloned_row->values[idx] = NULL;
            } // in case the column stored NULL
        }
    } // end of cloning mysql row
    app_db_query_notify_with_result(curr_query, rs);
} // end of _app_mariadb_row_ready_helper

uint8_t  app_mariadb_conn_is_closed(db_conn_t *conn)
{
    if(!conn) {
        return 1;
    }
    enum _dbconn_async_state  curr_state = (enum _dbconn_async_state) conn->state;
    return (curr_state == DB_ASYNC_INITED) || (curr_state == DB_ASYNC_CLOSE_DONE);
}

int app_db_mariadb_get_sock_fd(db_conn_t *conn)
{
    return mysql_get_socket((MYSQL *)conn->lowlvl.conn);
} // end of app_db_mariadb_get_sock_fd

uint64_t  app_db_mariadb_get_timeout_ms(db_conn_t *conn)
{
    return (uint64_t)mysql_get_timeout_value_ms((MYSQL *)conn->lowlvl.conn);
}


uint8_t app_mariadb_conn_notified_query_callback(db_query_t *query, db_query_result_t *rs)
{
    uint8_t final = 0;
    switch((enum _dbconn_async_state)rs->conn.state) {
        case DB_ASYNC_CHECK_CURRENT_RESULTSET:
            // if the last statement of a query doesn't return anything, check `rs->data` to ensure
            // whether it is the end, `rs->data` must be pointer to information of current result set
            query->cfg.callbacks.result_rdy(query, rs);
            final = rs->_final;
            break;
        case DB_ASYNC_FETCH_ROW_READY:
            query->cfg.callbacks.row_fetched(query, rs);
            break;
        case DB_ASYNC_FREE_RESULTSET_DONE:
            query->cfg.callbacks.result_free(query, rs);
            final = rs->_final;
            break;
        default: // TODO, logging error
            if(rs->app_result != DBA_RESULT_OK) {
                query->cfg.callbacks.error(query, rs);
                final = 1; // immediately deallocate the query (node) as soon as error is reported
            }
            break;
    } // end of switch statement
    return final;
} // end of app_mariadb_conn_notified_query_callback


uint8_t  app_mariadb_acquire_state_change(db_conn_t *conn)
{
    uint8_t allowed = 0;
    uint8_t is_changing = 1;
    // app caller is allowed to perform the transition ONLY when the connection
    // object is in following states...
    switch((enum _dbconn_async_state)conn->state) {
        case DB_ASYNC_INITED:
        case DB_ASYNC_CONN_START:
        case DB_ASYNC_QUERY_START:
            is_changing = atomic_flag_test_and_set_explicit(&conn->flags.state_changing, memory_order_acquire);
            if(!is_changing) {
                allowed = 1 ;
            }
            break;
        default:
            break;
    } // end of switch
    return allowed;
} // end of app_mariadb_acquire_state_change


void app_mariadb_async_state_transition_handler(app_timer_poll_t *target, int uv_status, int event_flags)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t *conn = H2O_STRUCT_FROM_MEMBER(db_conn_t, timer_poll, target);
    uint8_t called_by_app = (uv_status == 0) && (event_flags == 0);
    uint8_t continue_checking = 0;
    do {
        switch((enum _dbconn_async_state)conn->state) {
            case DB_ASYNC_INITED:
                conn->state = DB_ASYNC_CONN_START;
            case DB_ASYNC_CONN_START:
                event_flags = 0;
                result = app_db_mariadb_conn_connect_start(conn, &event_flags);
                if(result == DBA_RESULT_OK && event_flags) { // init timer poll then start immediately
                    int fd = conn->pool->cfg.ops.get_sock_fd(conn);
                    if(called_by_app) {
                        conn->ops.timerpoll_init(conn->loop, target, fd);
                    } else { // called by event loop, should happen in reconnecting case
                        conn->ops.timerpoll_change_fd(target, fd);
                    }
                    result = app_db_async_add_poll_event(conn, event_flags);
                    if(result == DBA_RESULT_OK) {
                        conn->state = DB_ASYNC_CONN_WAITING;
                        continue_checking = 0;
                    } else {
                        conn->state = DB_ASYNC_CONN_DONE;
                        continue_checking = 1;
                    }
                } else { // error detected, unable to start async operation
                    conn->state = DB_ASYNC_CONN_DONE;
                    continue_checking = 1; // forward the result to next state
                }
                { // ------- debug --------
                    //// app_db_mariadb_conn_connect_start(conn, &event_flags);
                    //// app_db_async_init_timerpoll(conn);
                    //// result = DBA_RESULT_OK;
                    //// conn->state = DB_ASYNC_CONN_DONE;
                    //// continue_checking = 1;
                }
                break;
            case DB_ASYNC_CONN_WAITING:
                // do NOT rely on status sent from poll event callback, sometimes external libraries
                // signal that the file descriptor is writable or readable even when it is not, application
                // caller should be prepared to double-check this by using low-level (database-specific)
                // function to check the status of current connection or running query
                result = app_db_mariadb_conn_connect_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_CONNECTION_BUSY) {
                    break; // still waiting
                } else {
                    conn->state = DB_ASYNC_CONN_DONE;
                }
            case DB_ASYNC_CONN_DONE:
                conn->ops.timerpoll_stop(target);
                if(result == DBA_RESULT_OK) {
                    conn->state = DB_ASYNC_QUERY_START;
                } else {
                    _app_mariadb_error_reset_all_processing_query(conn, result);
                    _app_mariadb_error_reset_all_pending_query(conn, result);
                    conn->state = DB_ASYNC_CLOSE_START;
                    continue_checking = 1;
                    break;
                }
            case DB_ASYNC_QUERY_START:
                result = conn->ops.update_ready_queries(conn);
                { // ---------- debug -----------
                    //// conn->processing_queries = conn->pending_queries.head;
                    //// conn->pending_queries.head = NULL;
                    //// conn->pending_queries.tail = NULL;
                    //// result = conn->processing_queries ? DBA_RESULT_OK: DBA_RESULT_SKIPPED;
                }
                if(result == DBA_RESULT_OK) {
                    if(called_by_app) {
                        int fd = conn->pool->cfg.ops.get_sock_fd(conn);
                        conn->ops.timerpoll_init(conn->loop, &conn->timer_poll, fd);
                    }
                    event_flags = 0; // always reset event flags
                    result = app_db_mariadb_conn_send_query_start(conn, &event_flags);
                    if(result == DBA_RESULT_OK && event_flags) {
                        result = app_db_async_add_poll_event(conn, event_flags);
                        if(result == DBA_RESULT_OK) {
                            conn->state = DB_ASYNC_QUERY_WAITING;
                            continue_checking = 0;
                        } else { // TODO, abort queries which were already sent
                            conn->state = DB_ASYNC_QUERY_READY;
                            continue_checking = 1;
                        }
                    } else {
                        conn->state = DB_ASYNC_QUERY_READY;
                        continue_checking = 1; // immediately forward the error result to ready state
                    }
                    { // ------- debug --------
                        //// db_query_t *curr_query = (db_query_t *) &conn->processing_queries->data;
                        //// curr_query->db_result.num_rs_remain = 0;
                        //// conn->processing_queries = NULL;
                        //// conn->lowlvl.resultset = NULL;
                        //// conn->lowlvl.row = NULL;
                        //// size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t);
                        //// db_llnode_t *rs_node = malloc(rs_node_sz);
                        //// db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
                        //// *rs = (db_query_result_t) { .app_result = result, .free_data_cb = free,
                        ////     .conn = {.state = DB_ASYNC_FREE_RESULTSET_DONE, .alias = conn->pool->cfg.alias,
                        ////     .async = (target->poll.loop != curr_query->cfg.loop) }
                        //// };
                        //// app_db_query_notify_with_result(curr_query, rs);
                        //// continue_checking = 0;
                    }
                } else if(conn->pool->is_closing_fn(conn->pool)) {
                    if(called_by_app) {
                        int fd = conn->pool->cfg.ops.get_sock_fd(conn);
                        conn->ops.timerpoll_init(conn->loop, &conn->timer_poll, fd);
                    }
                    conn->state = DB_ASYNC_CLOSE_START;
                    continue_checking = 1;
                } else {
                    conn->ops.timerpoll_deinit(&conn->timer_poll);
                    continue_checking = 0;
                    atomic_flag_clear_explicit(&conn->flags.state_changing, memory_order_release);
                }
                break;
            case DB_ASYNC_QUERY_WAITING:
                // if(uv_status == UV_ETIMEDOUT) {
                //     assert(0);
                // }
                result = app_db_mariadb_conn_send_query_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_QUERY_STILL_PROCESSING) {
                    break;
                } else {
                    conn->state = DB_ASYNC_QUERY_READY;
                }
            case DB_ASYNC_QUERY_READY:
                conn->ops.timerpoll_stop(target);
                if(result == DBA_RESULT_OK) {
                    conn->state = DB_ASYNC_CHECK_CURRENT_RESULTSET;
                } else {
                    _app_mariadb_error_reset_all_processing_query(conn, result);
                    conn->state = (result == DBA_RESULT_NETWORK_ERROR) ? DB_ASYNC_CLOSE_START: DB_ASYNC_QUERY_START;
                    continue_checking = 1;
                    break;
                }
            case DB_ASYNC_CHECK_CURRENT_RESULTSET:
                conn->lowlvl.resultset = (void *) mysql_use_result((MYSQL *)conn->lowlvl.conn);
                conn->lowlvl.row = NULL;
                if(conn->processing_queries) {
                    db_query_t *curr_query = (db_query_t *) &conn->processing_queries->data;
                    size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t);
                    if(conn->lowlvl.resultset) { rs_node_sz += sizeof(db_query_rs_info_t); }
                    db_llnode_t *rs_node = malloc(rs_node_sz);
                    db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
                    *rs = (db_query_result_t) {.free_data_cb = free, .conn = {.state = conn->state,
                        .alias = conn->pool->cfg.alias, .async = (conn->loop != curr_query->cfg.loop)}
                    };
                    if(conn->lowlvl.resultset) {
                        db_query_rs_info_t *cloned_rs_info = (db_query_rs_info_t *) &rs->data[0];
                        cloned_rs_info->num_rows.affected = (size_t)mysql_affected_rows(conn->lowlvl.resultset);
                        rs->app_result = DBA_RESULT_OK;
                    } else {
                        app_db_conn_try_evict_current_processing_query(conn);
                        rs->app_result = DBA_RESULT_END_OF_ROWS_REACHED;
                    } // TODO: copy important attributes from result set to the following query struct
                    rs->_final = (curr_query->db_result.num_rs_remain == 0) && 
                        (rs->app_result == DBA_RESULT_END_OF_ROWS_REACHED);
                    app_db_query_notify_with_result(curr_query, rs);
                    if(conn->lowlvl.resultset) { // current result set has rows
                        conn->state = DB_ASYNC_FETCH_ROW_START;
                        continue_checking = 1;
                        break;
                    } else { // go check next result set
                        conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START;
                    }
                } else { // in case app caller accidentally send junk string literal
                    if(conn->lowlvl.resultset) { // current result set has rows
                        conn->state = DB_ASYNC_FREE_RESULTSET_START;
                        continue_checking = 1;
                        break;
                    } else { // go check next result set
                        conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START;
                    }
                }
            case DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START:
                event_flags = 0; // always reset event flags
                result = app_db_mariadb_next_resultset_start(conn, &event_flags);
                if(result == DBA_RESULT_OK && event_flags) {
                    result = app_db_async_add_poll_event(conn, event_flags);
                    if(result == DBA_RESULT_OK) {
                        conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING;
                        continue_checking = 0;
                    } else {
                        conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_DONE;
                        continue_checking = 1;
                    }
                } else {
                    conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_DONE;
                    continue_checking = 1; // immediately forward the error result to ready state
                }
                break;
            case DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING:
                result = app_db_mariadb_next_resultset_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_RSET_STILL_LOADING) {
                    break;
                } else {
                    conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_DONE;
                }
            case DB_ASYNC_MOVE_TO_NEXT_RESULTSET_DONE:
                conn->ops.timerpoll_stop(target);
                if(result == DBA_RESULT_OK) {
                    conn->state = DB_ASYNC_CHECK_CURRENT_RESULTSET;
                } else if(result == DBA_RESULT_END_OF_RSETS_REACHED){
                    // if all the result sets has been visited, the result should be DBA_RESULT_END_OF_RSETS_REACHED
                    conn->state = DB_ASYNC_QUERY_START;
                } else { // other errors
                    // still switch to this state even we're pretty sure there won't be new result set available
                    // because this handling function still needs to respond to currently processing query added
                    //  by application caller
                    conn->state = DB_ASYNC_CHECK_CURRENT_RESULTSET;
                } // TODO, logging error
                continue_checking = 1;
                break;
            case DB_ASYNC_FETCH_ROW_START:
                event_flags = 0;
                result = app_db_mariadb_fetch_row_start(conn, &event_flags);
                if(result == DBA_RESULT_OK && event_flags) {
                    result = app_db_async_add_poll_event(conn, event_flags);
                    if(result == DBA_RESULT_OK) {
                        conn->state = DB_ASYNC_FETCH_ROW_WAITING;
                        continue_checking = 0;
                    } else {
                        conn->state = DB_ASYNC_FETCH_ROW_READY;
                        continue_checking = 1;
                    }
                } else {
                    conn->state = DB_ASYNC_FETCH_ROW_READY;
                    continue_checking = 1;
                }
                break;
            case DB_ASYNC_FETCH_ROW_WAITING:
                result = app_db_mariadb_fetch_row_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_ROW_STILL_FETCHING) {
                    break;
                } else {
                    conn->state = DB_ASYNC_FETCH_ROW_READY;
                }
            case DB_ASYNC_FETCH_ROW_READY:
                conn->ops.timerpoll_stop(target);
                if(result == DBA_RESULT_END_OF_ROWS_REACHED) {
                    conn->state = DB_ASYNC_FREE_RESULTSET_START;
                } else {
                    _app_mariadb_row_ready_helper(conn, result);
                    conn->state = DB_ASYNC_FETCH_ROW_START;
                    continue_checking = 1;
                    break;
                }
            case DB_ASYNC_FREE_RESULTSET_START:
                event_flags = 0;
                result = app_db_mariadb_free_resultset_start(conn, &event_flags);
                if(result == DBA_RESULT_OK && event_flags) {
                    result = app_db_async_add_poll_event(conn, event_flags);
                    if(result == DBA_RESULT_OK) {
                        conn->state = DB_ASYNC_FREE_RESULTSET_WAITING;
                        continue_checking = 0;
                    } else {
                        conn->state = DB_ASYNC_FREE_RESULTSET_DONE;
                        continue_checking = 1;
                    }
                } else {
                    conn->state = DB_ASYNC_FREE_RESULTSET_DONE;
                    continue_checking = 1;
                }
                break;
            case DB_ASYNC_FREE_RESULTSET_WAITING:
                result = app_db_mariadb_free_resultset_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_REST_RELEASING) {
                    break;
                } else {
                    conn->state = DB_ASYNC_FREE_RESULTSET_DONE;
                }
            case DB_ASYNC_FREE_RESULTSET_DONE:
                conn->ops.timerpoll_stop(target);
                conn->lowlvl.resultset = NULL;
                if(conn->processing_queries) {
                    db_query_t *curr_query = (db_query_t *) &conn->processing_queries->data;
                    app_db_conn_try_evict_current_processing_query(conn);
                    size_t rs_node_sz = sizeof(db_llnode_t) + sizeof(db_query_result_t);
                    db_llnode_t *rs_node = malloc(rs_node_sz);
                    db_query_result_t *rs = (db_query_result_t *) &rs_node->data[0];
                    *rs = (db_query_result_t) {.free_data_cb = free, .app_result = result,
                        .conn = {.state = conn->state, .alias = conn->pool->cfg.alias,
                        .async = (conn->loop != curr_query->cfg.loop)},
                        ._final = (curr_query->db_result.num_rs_remain == 0),
                    };  // end of statements in this query reached
                    app_db_query_notify_with_result(curr_query, rs);
                }
                conn->state = DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START;
                continue_checking = 1;
                break;

            case DB_ASYNC_CLOSE_START:
                result = app_db_mariadb_conn_close_start(conn, &event_flags);
                if(result == DBA_RESULT_OK && event_flags) {
                    result = app_db_async_add_poll_event(conn, event_flags);
                    if(result == DBA_RESULT_OK) {
                        conn->state = DB_ASYNC_CLOSE_WAITING;
                        continue_checking = 0;
                    } else {
                        conn->state = DB_ASYNC_CLOSE_DONE;
                        continue_checking = 1;
                    }
                } else {
                    conn->state = DB_ASYNC_CLOSE_DONE;
                    continue_checking = 1;
                }
                { // ---------- debug ------------
                    //// conn->state = DB_ASYNC_CLOSE_DONE;
                    //// continue_checking = 1;
                }
                break;
            case DB_ASYNC_CLOSE_WAITING:
                result = app_db_mariadb_conn_close_cont(conn, &event_flags, uv_status);
                if(result == DBA_RESULT_CONNECTION_BUSY) {
                    break;
                } else {
                    conn->state = DB_ASYNC_CLOSE_DONE;
                }
            case DB_ASYNC_CLOSE_DONE:
                conn->ops.timerpoll_stop(target);
                conn->lowlvl.conn = (void *)NULL;
                continue_checking = app_db_conn_get_first_query(conn) != NULL;
                if(conn->pool->is_closing_fn(conn->pool)) {
                    if(continue_checking) {
                        result = DBA_RESULT_CONNECTION_BUSY;
                        _app_mariadb_error_reset_all_processing_query(conn, result);
                        _app_mariadb_error_reset_all_pending_query(conn, result);
                    }
                    conn->ops.timerpoll_deinit(&conn->timer_poll);
                    continue_checking = 0;
                } else {
                    _app_mariadb_gen_new_handle((MYSQL **)&conn->lowlvl.conn, conn->pool->cfg.idle_timeout);
                    conn->state =  DB_ASYNC_CONN_START;
                }
                if(!continue_checking) {
                    atomic_flag_clear_explicit(&conn->flags.state_changing, memory_order_release);
                }
                break; // keep looping and transitting between the states until all queries are processed
            default: // TODO, logging unknown state
                break;
        } // end of connection state check
    } while(continue_checking); // end of continue_checking loop
} // end of app_mariadb_async_state_transition_handler


void  app_db_mariadb__cfg_ops(db_conn_cbs_t *cfg) {
    if(!cfg) { return; }
    *cfg = (db_conn_cbs_t) {
        .init_fn = app_db_mariadb_conn_init,
        .deinit_fn = app_db_mariadb_conn_deinit,
        .can_change_state = app_mariadb_acquire_state_change,
        .state_transition = app_mariadb_async_state_transition_handler,
        .notify_query = app_mariadb_conn_notified_query_callback,
        .is_conn_closed = app_mariadb_conn_is_closed,
        .get_sock_fd = app_db_mariadb_get_sock_fd,
        .get_timeout_ms = app_db_mariadb_get_timeout_ms
    };
}
