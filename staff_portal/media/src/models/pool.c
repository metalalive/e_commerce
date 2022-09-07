#include <h2o/memory.h>
#include "models/pool.h"

static db_llnode_t *_app_db_pools_map = NULL;

// maintain a sorted list in every single insertion
// with the key `length of alias` in descending order
static void app_db_poolmap_insert_pool(db_llnode_t *new) {
    new->next = NULL;
    new->prev = NULL;
    db_llnode_t *curr = NULL;
    db_llnode_t *prev = NULL;
    for(curr = _app_db_pools_map; curr; prev = curr, curr = curr->next) {
        db_pool_t *p0 = (db_pool_t *) &curr->data;
        db_pool_t *p1 = (db_pool_t *) &new->data;
        if(strlen(p0->cfg.alias) < strlen(p1->cfg.alias)) {
            break;
        }
    }
    app_llnode_link(curr, prev, new);
    if(!prev) { // the new node has to be new head of list
        _app_db_pools_map = new;
    }
} // end of app_db_poolmap_insert_pool

static void app_db_poolmap_remove_pool(db_llnode_t *node)
{
    db_llnode_t *n1 = node->next;
    app_llnode_unlink(node);
    if(_app_db_pools_map == node) {
        _app_db_pools_map = n1;
    }
} // end of app_db_poolmap_remove_pool

static void app_db_pool_insert_conn(db_pool_t *pool, db_llnode_t *new)
{
    db_llnode_t *old_head = pool->conns.head;
    new->next = NULL;
    new->prev = NULL;
    app_llnode_link(old_head, NULL, new); // insert before the original head node
    pool->conns.head = new;
    if(!old_head) {
        pool->conns.tail = new;
    }
} // end of app_db_pool_insert_conn

static void app_db_pool_remove_conn(db_pool_t *pool, db_llnode_t *node)
{
    db_llnode_t *n1 = node->next;
    app_llnode_unlink(node);
    if(pool->conns.head == node) {
        pool->conns.head = n1;
    }
} // end of app_db_pool_remove_conn


static void _app_db_poolcfg_deinit(db_pool_cfg_t *cfg) {
    if(cfg->alias) {
        free(cfg->alias);
        cfg->alias = NULL;
    }
    if(cfg->conn_detail.db_name) {
        free(cfg->conn_detail.db_name);
        cfg->conn_detail.db_name = NULL;
    }
    if(cfg->conn_detail.db_user) {
        free(cfg->conn_detail.db_user);
        cfg->conn_detail.db_user= NULL;
    }
    if(cfg->conn_detail.db_passwd) {
        free(cfg->conn_detail.db_passwd);
        cfg->conn_detail.db_passwd= NULL;
    }
    if(cfg->conn_detail.db_host) {
        free(cfg->conn_detail.db_host);
        cfg->conn_detail.db_host= NULL;
    }
} // end of _app_db_poolcfg_deinit


static void _app_db_pool_conns_deinit(db_pool_t *pool) {
    db_llnode_t *node = NULL;
    while(pool->conns.head) {
        node = pool->conns.head;
        app_db_pool_remove_conn(pool, node);
        db_conn_t  *conn = (db_conn_t *) node->data;
        assert(pool->cfg.ops.conn_deinit_fn(conn) == DBA_RESULT_OK);
        free(node);
    }
    pool->conns.tail = NULL;
}


// callers get available connection from a given pool, then use the returned connection
//  for subsequent commands (e.g. query) , return NULL means all connections in the pool
//  are in use
static db_conn_t * app_db_pool__acquire_free_connection(db_pool_t *pool) {
    db_conn_t  *found = NULL;
    db_llnode_t *node = NULL;
    pthread_mutex_lock(&pool->lock);
    node = pool->conns.head;
    if(node) { // move from free list to locked list
        if(node == pool->conns.tail) {
            pool->conns.tail = NULL;
        }
        pool->conns.head = node->next;
        app_llnode_unlink(node);
        app_llnode_link(pool->locked_conns, NULL, node); // insert to index 0
        pool->locked_conns = node;
        found = (db_conn_t *) &node->data;
    }
    pthread_mutex_unlock(&pool->lock);
    return found;
} // end of app_db_pool__acquire_free_connection


static DBA_RES_CODE app_db_pool__release_used_connection(db_conn_t *conn) {
    if(!conn || !conn->pool) {
        return DBA_RESULT_ERROR_ARG;
    }
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_pool_t *pool = conn->pool;
    db_llnode_t *node  = NULL;
    db_llnode_t *node2 = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, conn);
    uint8_t  found = 0;
    for(node = pool->locked_conns; node; node = node->next) {
        if(node == node2) {
            found = 1;
            break;
        }
    } // end of loop
    if(found) { // move from locked list back to free list
        pthread_mutex_lock(&pool->lock);
        if(pool->locked_conns == node2) {
            pool->locked_conns = node2->next;
        }
        app_llnode_unlink(node2);
        app_llnode_link(NULL, pool->conns.tail, node2); // append to the end of list
        pool->conns.tail = node2;
        if(!pool->conns.head) {
            pool->conns.head = node2;
        }
        pthread_mutex_unlock(&pool->lock);
    } else {
        result = DBA_RESULT_ERROR_ARG;
    }
    return result;
} // end of app_db_pool__release_used_connection

static uint8_t  app_db_pool_is_closing(db_pool_t *pool)
{
    if(!pool) { return 1; }
    uint16_t closing = 0x1;
    uint16_t value = (uint16_t) atomic_load_explicit(&pool->flags, memory_order_acquire);
    return (value & closing) == closing;
} // end of app_db_pool_is_closing


DBA_RES_CODE app_db_pool_init(db_pool_cfg_t *opts)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_llnode_t *new_pool_node = NULL;
    db_llnode_t *new_conn_node = NULL;
    db_pool_t   *pool = NULL;
    size_t idx = 0;
    if(!opts || !opts->alias || opts->capacity == 0 || opts->idle_timeout == 0
            || opts->bulk_query_limit_kb == 0) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    if(!opts->conn_detail.db_name || !opts->conn_detail.db_user || !opts->conn_detail.db_passwd
            || !opts->conn_detail.db_host || opts->conn_detail.db_port == 0 || !opts->ops.conn_init_fn
            || !opts->ops.conn_deinit_fn || !opts->ops.state_transition || !opts->ops.notify_query
            || !opts->ops.is_conn_closed || !opts->ops.get_sock_fd || !opts->ops.get_timeout_ms
            || !opts->ops.can_change_state ) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    pool = app_db_pool_get_pool(opts->alias);
    if(pool) { // alias has to be unique in the map
        result = DBA_RESULT_MEMORY_ERROR;
        goto done;
    }
    new_pool_node = malloc(sizeof(db_llnode_t) + sizeof(db_pool_t));
    pool = (db_pool_t *) &new_pool_node->data;
    pool->conns.head = NULL;
    pool->conns.tail = NULL;
    pool->locked_conns = NULL;
    pool->flags = ATOMIC_VAR_INIT(0x0);
    pool->acquire_free_conn_fn = app_db_pool__acquire_free_connection;
    pool->release_used_conn_fn = app_db_pool__release_used_connection;
    pool->is_closing_fn = app_db_pool_is_closing;
    if(pthread_mutex_init(&pool->lock, NULL) != 0) {
        result = DBA_RESULT_OS_ERROR;
        goto error;
    }
    pool->cfg = (db_pool_cfg_t) {
        .alias = strdup(opts->alias),   .capacity = opts->capacity,
        .idle_timeout = opts->idle_timeout,
        .bulk_query_limit_kb = opts->bulk_query_limit_kb,
        .conn_detail = {
            .db_name   = strdup(opts->conn_detail.db_name), 
            .db_user   = strdup(opts->conn_detail.db_user),
            .db_passwd = strdup(opts->conn_detail.db_passwd), 
            .db_host   = strdup(opts->conn_detail.db_host), 
            .db_port = opts->conn_detail.db_port 
        },
        .ops = {
            .global_init_fn = opts->ops.global_init_fn,
            .global_deinit_fn = opts->ops.global_deinit_fn,
            .conn_init_fn  = opts->ops.conn_init_fn,
            .conn_deinit_fn = opts->ops.conn_deinit_fn,
            .error_cb  = opts->ops.error_cb,
            .can_change_state = opts->ops.can_change_state,
            .state_transition = opts->ops.state_transition,
            .notify_query = opts->ops.notify_query,
            .is_conn_closed = opts->ops.is_conn_closed,
            .get_sock_fd  = opts->ops.get_sock_fd,
            .get_timeout_ms  = opts->ops.get_timeout_ms
        }
    };
    if(pool->cfg.ops.global_init_fn) {
        result = pool->cfg.ops.global_init_fn(pool);
        if(result != DBA_RESULT_OK)
            goto error; 
    }
    size_t conn_sz = sizeof(db_conn_t) + (pool->cfg.bulk_query_limit_kb << 10) + 1; // including NULL-terminated byte
    size_t conn_node_sz = sizeof(db_llnode_t) + conn_sz;
    for(idx = 0; idx < pool->cfg.capacity; idx++) {   // initalize list of connections
        new_conn_node = malloc(conn_node_sz);
        db_conn_t   *new_conn = (db_conn_t *) new_conn_node->data;
        result = pool->cfg.ops.conn_init_fn(new_conn, pool);
        if(result != DBA_RESULT_OK) {
            free(new_conn_node);
            goto error; 
        }
        app_db_pool_insert_conn(pool, new_conn_node);
    } // end of loop
    app_db_poolmap_insert_pool(new_pool_node);
    goto done;
error:
    if(new_pool_node) {
        _app_db_pool_conns_deinit(pool);
        pthread_mutex_destroy(&pool->lock);
        _app_db_poolcfg_deinit(&pool->cfg);
        free(new_pool_node);
        new_pool_node = NULL;
    }
done:
    return result;
} // end of app_db_pool_init


static DBA_RES_CODE _app_db_pool_deinit(db_pool_t *pool) {
    if(!pool) {
        return DBA_RESULT_ERROR_ARG;
    }
    DBA_RES_CODE  result = DBA_RESULT_OK;
    _app_db_pool_conns_deinit(pool);
    _app_db_poolcfg_deinit(&pool->cfg);
    pthread_mutex_destroy(&pool->lock);
    if(pool->cfg.ops.global_deinit_fn) 
        result = pool->cfg.ops.global_deinit_fn(pool);
    db_llnode_t *node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, pool);
    app_db_poolmap_remove_pool(node);
    free(node);
    return  result;
} // end of _app_db_pool_deinit


void app_db_pool_map_signal_closing(void)
{
    uint16_t closing = 0x1;
    db_llnode_t *node = NULL;
    for(node = _app_db_pools_map; node; node = node->next) {
        db_pool_t *pool = (db_pool_t *)&node->data;
        uint16_t value = atomic_load_explicit(&pool->flags, memory_order_acquire);
        value = value | closing;
        atomic_store_explicit(&pool->flags, value, memory_order_release);
    }
} // end of app_db_pool_map_signal_closing


void  app_db_poolmap_close_all_conns(uv_loop_t *loop) {
    db_llnode_t *pnode = NULL;
    db_llnode_t *cnode = NULL;
    for(pnode = _app_db_pools_map; pnode; pnode = pnode->next) {
        db_pool_t *pool = (db_pool_t *)&pnode->data;
        for(cnode = pool->conns.head; cnode; cnode = cnode->next) {
            db_conn_t  *conn = (db_conn_t *) &cnode->data[0];
            conn->ops.try_close(conn, loop);
        }
    }
} // end of app_db_poolmap_close_all_conns

uint8_t  app_db_poolmap_check_all_conns_closed(void) {
    uint8_t done = 1;
    db_llnode_t *pnode = NULL;
    db_llnode_t *cnode = NULL;
    for(pnode = _app_db_pools_map; pnode && done; pnode = pnode->next) {
        db_pool_t *pool = (db_pool_t *)&pnode->data;
        for(cnode = pool->conns.head; cnode && done; cnode = cnode->next) {
            db_conn_t  *conn = (db_conn_t *) &cnode->data[0];
            done = done & conn->ops.is_closed(conn);
        }
    }
    return done;
} // end of app_db_poolmap_check_all_conns_closed


DBA_RES_CODE app_db_pool_deinit(const char *alias)
{
    return _app_db_pool_deinit(app_db_pool_get_pool(alias));
} // end of app_db_pool_deinit


db_pool_t *app_db_pool_get_pool(const char *alias)
{
    db_llnode_t *found = NULL;
    db_llnode_t *node = NULL;
    for(node = _app_db_pools_map; node && !found; node = node->next) {
        db_pool_t *p = (db_pool_t *)&node->data;
        if(strcmp(p->cfg.alias, alias) == 0) {
            found = node;
        }
    } // end of loop
    return (found ? (db_pool_t *)&found->data : NULL);
} // end of app_db_pool_get_pool


DBA_RES_CODE app_db_pool_map_deinit(void)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(_app_db_pools_map) {
        while(_app_db_pools_map && result == DBA_RESULT_OK) {
            db_llnode_t *node = _app_db_pools_map;
            db_pool_t *p = (db_pool_t *)&node->data;
            result = _app_db_pool_deinit(p);
        }
        _app_db_pools_map = NULL;
    } else {
        result = DBA_RESULT_SKIPPED;
    }
    return result;
} // end of app_db_pool_map_deinit

