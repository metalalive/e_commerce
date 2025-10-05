#include <h2o/memory.h>
#include "models/pool.h"
#include "models/connection.h"

static DBA_RES_CODE app_db_conn__append_pending_query(db_conn_t *conn, db_query_t *query) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    if (!conn || !query) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    pthread_mutex_lock(&conn->lock);
    {
        db_llnode_t *new_node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, query);
        if (!conn->pending_queries.head) {
            conn->pending_queries.head = new_node;
        }
        app_llnode_link(NULL, conn->pending_queries.tail, new_node); // append to the end of pnding list
        conn->pending_queries.tail = new_node;
    }
    pthread_mutex_unlock(&conn->lock);
done:
    return result;
} // end of app_db_conn__append_pending_query

static size_t _app_db_conn_rdy_stmts_to_net_buffer(char *wr_ptr, db_query_t *q) {
    // buffer for accessing multiple statements of currently processing queries in
    // consecutive address space, the statements are separate by semicolon `;`, this
    // is for ease of copying the statements in low-level function executing the
    // bulk queries in one go.
    char *wr_ptr_origin = wr_ptr;
    memcpy(wr_ptr, q->cfg.statements.entry, q->_stmts_tot_sz);
    wr_ptr += q->_stmts_tot_sz;
    return (size_t)(wr_ptr - wr_ptr_origin);
} // end of _app_db_conn_rdy_stmts_to_net_buffer

static DBA_RES_CODE app_db_conn__update_ready_queries(db_conn_t *conn) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    if (!conn) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    pthread_mutex_lock(&conn->lock);
    if (conn->processing_queries) {
        // skip because currently processing queries haven't been completed yet , it is
        // OK to skip it now, the state transition handler will automatically check
        // any pending query after the connection object finishes processing current quries
        result = DBA_RESULT_SKIPPED;
    } else {
        // Due to memory constraint, application has to determine number of queires to process
        // at bulk, which is determined in application configuration module, this function
        // moves adjacent linked-list nodes from pending query to the processing query list,
        // the total bytes of SQL statements in the processing queries MUST NOT to exceed
        // the size limit specified in  `db_pool_cfg_t.query_limit_kb`
        size_t       q_limit_bytes = conn->pool->cfg.bulk_query_limit_kb << 10;
        size_t       stmt_total_sz = 0;
        db_llnode_t *pq_tail = NULL;
        conn->bulk_query_rawbytes.wr_sz = 0;
        char *wr_ptr = &conn->bulk_query_rawbytes.data[0];
        for (pq_tail = conn->pending_queries.head; pq_tail; pq_tail = pq_tail->next) {
            db_query_t *query = (db_query_t *)pq_tail->data;
            assert(query->_stmts_tot_sz <= q_limit_bytes);
            size_t stmt_total_sz_tmp = stmt_total_sz + query->_stmts_tot_sz;
            if (stmt_total_sz_tmp < q_limit_bytes) {
                size_t n_wr = _app_db_conn_rdy_stmts_to_net_buffer(wr_ptr, query);
                conn->bulk_query_rawbytes.wr_sz += n_wr;
                wr_ptr += n_wr;
                stmt_total_sz = stmt_total_sz_tmp;
            } else {
                pq_tail = pq_tail->prev;
                break;
            }
        }
        *wr_ptr++ = 0x0; // always append NULL char to the end of bulk query statements
        //// conn->bulk_query_rawbytes.wr_sz ++; // NULL char must NOT be sent to database server
        {
            atomic_thread_fence(memory_order_acquire);
            conn->processing_queries = conn->pending_queries.head;
            uint8_t value = (conn->processing_queries != NULL);
            atomic_store_explicit(&conn->flags.has_ready_query_to_process, value, memory_order_release);
        }
        if (!pq_tail) {
            pq_tail = conn->pending_queries.tail;
        } // should move all pending queries
        if (pq_tail) {
            conn->pending_queries.head = pq_tail->next;
            if (pq_tail->next) {
                pq_tail->next->prev = NULL; // split to 2 query lists
                pq_tail->next = NULL;
            }
        } else {
            conn->pending_queries.head = NULL;
        }
        if (!conn->pending_queries.head) {
            conn->pending_queries.tail = NULL;
        } // move all pending queries to processing queries
        if (!conn->processing_queries) {
            result = DBA_RESULT_SKIPPED;
        } // no pending query to execute
    } // end of critical section
    pthread_mutex_unlock(&conn->lock);
done:
    return result;
} // end of app_db_conn__update_ready_queries

DBA_RES_CODE app_db_conn_try_evict_current_processing_query(db_conn_t *conn) {
    if (!conn) {
        return DBA_RESULT_ERROR_ARG;
    }
    db_llnode_t *pq_head = conn->processing_queries;
    if (!pq_head) {
        return DBA_RESULT_MEMORY_ERROR;
    }
    db_query_t *q = (db_query_t *)&pq_head->data[0];
    size_t      num_rs_remain = q->db_result.num_rs_remain;
    q->db_result.num_rs_remain = num_rs_remain > 0 ? --num_rs_remain : 0;
    if (q->db_result.num_rs_remain == 0) {
        pthread_mutex_lock(&conn->lock);
        conn->processing_queries = pq_head->next;
        app_llnode_unlink(pq_head);
        pthread_mutex_unlock(&conn->lock);
    }
    return DBA_RESULT_OK;
} // end of app_db_conn_try_evict_current_processing_query

db_query_t *app_db_conn_get_first_query(db_conn_t *conn) {
    db_query_t *out = NULL;
    if (conn->processing_queries) {
        out = (db_query_t *)conn->processing_queries->data;
    } else if (conn->pending_queries.head) {
        out = (db_query_t *)conn->pending_queries.head->data;
    }
    return out;
} // end of app_db_conn_get_first_query

DBA_RES_CODE app_db_async_add_poll_event(db_conn_t *conn, uint32_t event_flags) {
    uint64_t timeout_ms = conn->pool->cfg.ops.get_timeout_ms(conn);
    int      err = conn->ops.timerpoll_start(
        &conn->timer_poll, timeout_ms, event_flags, conn->pool->cfg.ops.state_transition
    );
    return (err == 0) ? DBA_RESULT_OK : DBA_RESULT_OS_ERROR;
} // end of app_db_async_add_poll_event

static DBA_RES_CODE _app_db_async__try_processing_queries(
    db_conn_t *conn, uv_loop_t *loop
) { // this callback must be invoked by application, not libuv loop
    if (!conn || !conn->pool || !loop) {
        return DBA_RESULT_ERROR_ARG;
    } // the timer-poll for each connection should be closed before calling this function
    uint8_t _has_ready_query_to_process =
        (uint8_t)atomic_load_explicit(&conn->flags.has_ready_query_to_process, memory_order_relaxed);
    if (_has_ready_query_to_process) {
        // the connection object will process all queries in the pending list
        // , current worker thread does NOT need to call the state transition
        // function at here.
        goto done;
    }
    // otherwise the other worker thread should complete the state transition function very soon
    uint8_t closed = app_timer_poll_is_closed(&conn->timer_poll);
    while (!closed) {
        int ms = 10;
        uv_sleep(ms); // wait a bit if the timer poll is closing
        closed = app_timer_poll_is_closed(&conn->timer_poll);
    }
    if (conn->pool->cfg.ops.can_change_state(conn)) {
        conn->loop = loop;
#define CALLED_BY_APP 0, 0
        conn->pool->cfg.ops.state_transition(&conn->timer_poll, CALLED_BY_APP);
#undef CALLED_BY_APP
    }
done:
    return DBA_RESULT_OK;
} // end of _app_db_async__try_processing_queries

static DBA_RES_CODE _app_db_conn__try_close(
    db_conn_t *conn, uv_loop_t *loop
) { // this function is NOT thread-safe and only be called in main thread at the end
    if (!conn || !conn->pool) {
        return DBA_RESULT_ERROR_ARG;
    }
    if (conn->ops.is_closed(conn)) {
        return DBA_RESULT_SKIPPED;
    }
    if (conn->pool->cfg.ops.can_change_state(conn)) {
        conn->loop = loop;
#define CALLED_BY_APP 0, 0
        conn->pool->cfg.ops.state_transition(&conn->timer_poll, CALLED_BY_APP);
#undef CALLED_BY_APP
        return DBA_RESULT_OK;
    } else {
        return DBA_RESULT_CONNECTION_BUSY;
    }
} // end of _app_db_conn__try_close

static uint8_t _app_db_conn__check_is_closed(db_conn_t *conn) {
    if (!conn) {
        return 1;
    }
    db_pool_t *pool = conn->pool;
    uv_loop_t *loop = conn->timer_poll.poll.loop;
    int        uv_ret = 123;
    if (loop) {
        uv_ret = uv_run(loop, UV_RUN_NOWAIT);
        if (!uv_ret) {
            // TODO, logging info, no handles and requests left
        }
    } else {
        // TODO, logging error
    }
    // ensure timer_poll is already closed / de-inited
    uint8_t conn_state_closed = pool->cfg.ops.is_conn_closed(conn);
    uint8_t timerpoll_closed = app_timer_poll_is_closed(&conn->timer_poll);
    //// assert(conn_state_closed);
    return conn_state_closed && timerpoll_closed;
} // end of _app_db_conn__check_is_closed

DBA_RES_CODE app_db_conn_init(db_pool_t *pool, db_conn_t **conn_created) {
    if (!pool) {
        return DBA_RESULT_ERROR_ARG;
    }
    size_t conn_sz =
        sizeof(db_conn_t) + (pool->cfg.bulk_query_limit_kb << 10) + 1; // including NULL-terminated byte
    size_t       conn_node_sz = sizeof(db_llnode_t) + conn_sz;
    db_llnode_t *new_conn_node = malloc(conn_node_sz);
    db_conn_t   *conn = (db_conn_t *)new_conn_node->data;
    assert(conn == (db_conn_t *)&new_conn_node->data[0]);
    memset(new_conn_node, 0, sizeof(char) * conn_node_sz);

    pthread_mutex_init(&conn->lock, NULL);
    conn->pool = pool;
    conn->ops.add_new_query = app_db_conn__append_pending_query;
    conn->ops.update_ready_queries = app_db_conn__update_ready_queries;
    conn->ops.try_process_queries = _app_db_async__try_processing_queries;
    conn->ops.try_close = _app_db_conn__try_close;
    conn->ops.is_closed = _app_db_conn__check_is_closed;
    conn->ops.timerpoll_init = app_timer_poll_init;
    conn->ops.timerpoll_deinit = app_timer_poll_deinit;
    conn->ops.timerpoll_change_fd = app_timer_poll_change_fd;
    conn->ops.timerpoll_start = app_timer_poll_start;
    conn->ops.timerpoll_stop = app_timer_poll_stop;
    conn->flags.state_changing = (atomic_flag)ATOMIC_FLAG_INIT;
    conn->flags.has_ready_query_to_process = ATOMIC_VAR_INIT(0);

    DBA_RES_CODE result = pool->cfg.ops.conn_init_fn(conn);
    if (result == DBA_RESULT_OK) {
        app_db_pool_insert_conn(pool, new_conn_node);
        if (conn_created) {
            *conn_created = conn;
        }
    } else {
        app_db_conn_deinit(conn);
    }
    return result;
} // end of app_db_conn_init

DBA_RES_CODE app_db_conn_deinit(db_conn_t *conn) {
    if (!conn) {
        return DBA_RESULT_ERROR_ARG;
    }
    if (!conn->pool) {
        return DBA_RESULT_MEMORY_ERROR;
    }
    // application caller should close the connection first, then call this de-init function
    if (conn->processing_queries || conn->pending_queries.head) {
        return DBA_RESULT_CONNECTION_BUSY;
    }
    db_llnode_t *node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, conn);
    assert(conn == (db_conn_t *)&node->data[0]);
    app_db_pool_remove_conn(conn->pool, node);
    DBA_RES_CODE result = conn->pool->cfg.ops.conn_deinit_fn(conn);
    pthread_mutex_destroy(&conn->lock);
    conn->ops.add_new_query = NULL;
    conn->ops.update_ready_queries = NULL;
    conn->ops.try_process_queries = NULL;
    conn->ops.try_close = NULL;
    conn->ops.is_closed = NULL;
    conn->ops.timerpoll_init = NULL;
    conn->ops.timerpoll_deinit = NULL;
    conn->ops.timerpoll_change_fd = NULL;
    conn->ops.timerpoll_start = NULL;
    conn->ops.timerpoll_stop = NULL;
    conn->pool = NULL;
    free(node);
    return result;
} // end of app_db_conn_deinit
