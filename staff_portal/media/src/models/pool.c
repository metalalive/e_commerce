#include <h2o/memory.h>
#include "models/pool.h"

static db_llnode_t *_app_db_pools_map = NULL;

static void app_llnode_link(db_llnode_t *curr, db_llnode_t *prev, db_llnode_t *new)
{
    if(prev) {
        prev->next = new;
        new->prev  = prev;
    }
    if(curr) {
        curr->prev = new;
        new->next = curr;
    }
}

static void app_llnode_unlink(db_llnode_t *node)
{
    db_llnode_t *n0 = node->prev;
    db_llnode_t *n1 = node->next;
    if(n0) {
        n0->next = n1;
    }
    if(n1) {
        n1->prev = n0;
    }
    node->next = node->prev = NULL;
}

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
    new->next = NULL;
    new->prev = NULL;
    app_llnode_link(pool->conns, NULL, new); // insert before the original head node
    pool->conns = new;
} // end of app_db_pool_insert_conn

static void app_db_pool_remove_conn(db_pool_t *pool, db_llnode_t *node)
{
    db_llnode_t *n1 = node->next;
    app_llnode_unlink(node);
    if(pool->conns == node) {
        pool->conns = n1;
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
    while(pool->conns) {
        node = pool->conns;
        db_conn_t  *conn = (db_conn_t *) node->data;
        pool->cfg.conn_ops.deinit_fn(conn);
        app_db_pool_remove_conn(pool, node);
        free(node);
    }
}


static void _app_db_pool_default_error_callback(void *target, void *detail)
{
    db_pool_t *pool = target;
    fprintf(stderr, "[pooling][error][%s][reason_code] \n", pool->cfg.alias);
} // end of _app_db_pool_default_error_callback


DBA_RES_CODE app_db_pool_init(db_pool_cfg_t *opts)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_llnode_t *new_pool_node = NULL;
    db_pool_t   *pool = NULL;
    size_t idx = 0;
    if(!opts || !opts->alias || opts->capacity == 0 || opts->idle_timeout == 0) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    if(!opts->conn_detail.db_name || !opts->conn_detail.db_user || !opts->conn_detail.db_passwd
            || !opts->conn_detail.db_host || opts->conn_detail.db_port == 0 || !opts->conn_ops.init_fn
            || !opts->conn_ops.deinit_fn || !opts->conn_ops.close_fn || !opts->conn_ops.connect_fn
            ) {
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
    pool->conns = NULL;
    if(pthread_mutex_init(&pool->lock, NULL) != 0) {
        result = DBA_RESULT_OS_ERROR;
        goto error;
    }
    dba_error_cb error_cb = opts->error_cb ? opts->error_cb: _app_db_pool_default_error_callback;
    pool->cfg = (db_pool_cfg_t) {
        .alias = strdup(opts->alias),
        .capacity = opts->capacity,  .idle_timeout = opts->idle_timeout,
        .close_cb = opts->close_cb,  .error_cb = error_cb,
        .conn_detail = {
            .db_name   = strdup(opts->conn_detail.db_name), 
            .db_user   = strdup(opts->conn_detail.db_user),
            .db_passwd = strdup(opts->conn_detail.db_passwd), 
            .db_host   = strdup(opts->conn_detail.db_host), 
            .db_port = opts->conn_detail.db_port 
        },
        .conn_ops = {
            .init_fn   = opts->conn_ops.init_fn,
            .deinit_fn = opts->conn_ops.deinit_fn,
            .close_fn  = opts->conn_ops.close_fn,
            .connect_fn = opts->conn_ops.connect_fn
        }
    };
    for(idx = 0; idx < pool->cfg.capacity; idx++) {   // initalize list of connections
        db_llnode_t *new_conn_node = malloc(sizeof(db_llnode_t) + sizeof(db_conn_t));
        db_conn_t   *new_conn = (db_conn_t *) new_conn_node->data;
        result = pool->cfg.conn_ops.init_fn(new_conn, pool);
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
    if(pool->cfg.close_cb) {
        (pool->cfg.close_cb)((void *)pool);
    }
    _app_db_pool_conns_deinit(pool);
    _app_db_poolcfg_deinit(&pool->cfg);
    pthread_mutex_destroy(&pool->lock);
    db_llnode_t *node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, pool);
    app_db_poolmap_remove_pool(node);
    free(node);
    return DBA_RESULT_OK;
} // end of _app_db_pool_deinit


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
        while(_app_db_pools_map) {
            db_llnode_t *node = _app_db_pools_map;
            db_pool_t *p = (db_pool_t *)&node->data;
            _app_db_pool_deinit(p);
        }
        _app_db_pools_map = NULL;
    } else {
        result = DBA_RESULT_SKIPPED;
    }
    return result;
} // end of app_db_pool_map_deinit

