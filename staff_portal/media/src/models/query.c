#include <h2o/memory.h>
#include "models/query.h"

static void _app_db_query_deallocate_node(uv_handle_t *handle) {
    db_query_t *query = H2O_STRUCT_FROM_MEMBER(db_query_t, notification, handle);
    db_llnode_t *node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, query);
    free(node);
} // end of _app_db_query_deallocate_node

DBA_RES_CODE  app_db_query_enqueue_resultset(db_query_t *q, db_query_result_t *rs)
{
    if(!q || !rs) {
        return DBA_RESULT_ERROR_ARG;
    }
    db_llnode_t *new_node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, rs);
    pthread_mutex_lock(&q->db_result.lock);
    db_llnode_t *old_tail = q->db_result.tail;
    new_node->prev = NULL;
    new_node->next = NULL;
    if(old_tail) {
        app_llnode_link(NULL, old_tail, new_node);
    } else {
        q->db_result.head = new_node;
    }
    q->db_result.tail  = new_node;
    pthread_mutex_unlock(&q->db_result.lock);
    return DBA_RESULT_OK;
} // end of app_db_query_enqueue_resultset


db_query_result_t * app_db_query_dequeue_resultset(db_query_t *query) {
    if(!query || !query->db_result.head) {
        return NULL;
    }
    db_llnode_t *curr_node = query->db_result.head;
    pthread_mutex_lock(&query->db_result.lock);
    query->db_result.head = curr_node->next;
    if(!query->db_result.head) {
        query->db_result.tail = NULL;
    }
    app_llnode_unlink(curr_node);
    pthread_mutex_unlock(&query->db_result.lock);
    return (db_query_result_t *) &curr_node->data[0];
} // end of app_db_query_dequeue_resultset


DBA_RES_CODE app_db_query_notify_with_result(db_query_t *q, db_query_result_t *rs)
{
    if(!q || !rs) {
        return DBA_RESULT_ERROR_ARG;
    }
    DBA_RES_CODE result = app_db_query_enqueue_resultset(q, rs);
    // if the thread which runs uv_async_send() is the same as the thread which
    // runs the event loop registered in uv_async_t handle , the event loop will
    // fail to close because a uv_timer_t is never unreferenced for unknown reason,
    // that makes the event loop always return busy code. (TODO) figure out how
    // to fix it
    if(result == DBA_RESULT_OK) {
        if(rs->conn.async) {
            uv_async_send(&q->notification);
        } else {
            q->notification.async_cb(&q->notification);
        }
    }
    return result;
} // end of app_db_query_notify_with_result


static void _app_db_query_notification_callback(uv_async_t *handle)
{ // default callback for notifying the given query whenever result set is ready
    uint8_t final = 0;
    db_query_t *query = H2O_STRUCT_FROM_MEMBER(db_query_t, notification, handle);
    while (query->db_result.head) {
        db_query_result_t *rs = app_db_query_dequeue_resultset(query);
        final = query->cfg.pool->cfg.ops.notify_query(query, rs);
        db_llnode_t *rs_node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, rs);
        rs->free_data_cb(rs_node);
    } // end of iteration on pending results
    if(final) {
        uv_close((uv_handle_t *)handle, _app_db_query_deallocate_node);
    }
} // end of _app_db_query_notification_callback


static size_t _app_db_estimate_query_struct_size(db_query_cfg_t *cfg) {
    return sizeof(db_query_t) +  sizeof(void *) * cfg->usr_data.len;
} // end of _app_db_estimate_query_struct_size

static size_t _app_db_estimate_query_total_bytes_statements(db_query_cfg_t *cfg) {
    size_t out = 0;
    size_t q_limit_bytes = cfg->pool->cfg.bulk_query_limit_kb << 10;
    size_t stmt_len = strlen(cfg->statements.entry);
    // NULL string treated as invalid statement
    // the size of statements in single query must NOT
    // exceed the limit specified in pool configuration
    if(stmt_len > 0 && stmt_len < q_limit_bytes) {
        out = stmt_len;
    }
    // applications should determine its delimiter for each SQL statement, it is NOT necessarily
    // to be always semicolon character `;`
    return out;
} // end of _app_db_estimate_query_total_bytes_statements

static db_query_t *app_db_query_generate_node(db_query_cfg_t *qcfg) {
    size_t stmts_tot_sz = _app_db_estimate_query_total_bytes_statements(qcfg);
    if(stmts_tot_sz == 0) {
        return NULL;
    }
    size_t q_sz = _app_db_estimate_query_struct_size(qcfg) + stmts_tot_sz;
    size_t node_q_sz = sizeof(db_llnode_t) + q_sz;
    db_llnode_t *node = malloc(node_q_sz);
    db_query_t  *query = (db_query_t *)&node->data;
    node->prev = NULL;
    node->next = NULL;
    {
        query->db_result.head = NULL;
        query->db_result.tail = NULL;
        query->db_result.num_rs_remain = qcfg->statements.num_rs;
        memset(&query->db_result.lock, 0, sizeof(pthread_mutex_t));
        pthread_mutex_init(&query->db_result.lock, NULL);
    }
    query->_stmts_tot_sz = stmts_tot_sz;
    memset(&query->notification, 0, sizeof(uv_async_t));
    //// uv_async_init(qcfg->loop, &query->notification, qcfg->pool->cfg.ops.notify_query);
    uv_async_init(qcfg->loop, &query->notification, _app_db_query_notification_callback);
    memcpy((void *)&query->cfg, qcfg, sizeof(db_query_cfg_t));
    char *ptr = ((char *)query) + sizeof(db_query_t);
    // extra allocated space is assigned to statements and user data of callbacks
    if(qcfg->usr_data.len > 0) {
        query->cfg.usr_data.entry = (void **)ptr;
        size_t usr_data_tot_sz = sizeof(void *) * qcfg->usr_data.len;
        memcpy(query->cfg.usr_data.entry, qcfg->usr_data.entry, usr_data_tot_sz);
        ptr += usr_data_tot_sz; // 
    } else {
        query->cfg.usr_data.entry = NULL;
    }
    query->cfg.statements.entry = ptr;
    memcpy(query->cfg.statements.entry, qcfg->statements.entry, query->_stmts_tot_sz);
    { // rest of allocated bytes must be zero, to avoid logical error when parsing result sets later
        ptr += query->_stmts_tot_sz;
        size_t curr_visit_q_sz = (size_t) ((ssize_t)ptr - (ssize_t)node);
        if(curr_visit_q_sz < node_q_sz) {
            size_t rest_sz = node_q_sz - curr_visit_q_sz;
            memset(ptr, 0x0, rest_sz);
        }
    }
    return query;
} // end of app_db_query_generate_node


DBA_RES_CODE app_db_query_start(db_query_cfg_t *cfg)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t *free_conn = NULL;
    if(!cfg || cfg->statements.num_rs == 0 || !cfg->statements.entry || !cfg->pool || !cfg->loop
            || !cfg->callbacks.result_rdy || !cfg->callbacks.row_fetched
            || !cfg->callbacks.result_free || !cfg->callbacks.error) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    if(cfg->pool->is_closing_fn(cfg->pool)) {
        result = DBA_RESULT_POOL_BUSY;
        goto done;
    }
    free_conn = cfg->pool->acquire_free_conn_fn(cfg->pool);
    if(free_conn) {
        db_query_t *query = app_db_query_generate_node(cfg);
        if(query) { // append the new query object to the free connection
            free_conn->ops.add_new_query(free_conn, query);
        } else {
            result = DBA_RESULT_ERROR_ARG;
        }
        cfg->pool->release_used_conn_fn(free_conn);
        if(query) { // try connecting database and perform queries
            result = free_conn->ops.try_process_queries(free_conn, cfg->loop);
        }
    } else {
        result = DBA_RESULT_POOL_BUSY;
    }
done:
    return result;
} // end of app_db_query_start

