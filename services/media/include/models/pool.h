#ifndef MEDIA__MODELS_POOL_H
#define MEDIA__MODELS_POOL_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"

// de-initialize the global pool map and all the pools registered
// Note the function below is NOT thread-safe
DBA_RES_CODE app_db_pool_map_deinit(void);

// Register and initialize a new pool to global internal pool map
// In this application, it is good practice to keep separate pools by
// diferent database destinations.
// Note these 2 functions below are  NOT thread-safe
DBA_RES_CODE app_db_pool_init(db_pool_cfg_t *opts);

void app_db_pool_insert_conn(db_pool_t *, db_llnode_t *new_node);
void app_db_pool_remove_conn(db_pool_t *, db_llnode_t *new_node);

// entry function to try closing all connections within all pools
void app_db_pool_map_signal_closing(void);

// test whether all connections of all pools within the global pool map are closed
void     app_db_poolmap_close_all_conns(uv_loop_t *);
uint8_t  app_db_poolmap_check_all_conns_closed(void);

// de-initialize a given pool, and unregister from the global pool map
DBA_RES_CODE app_db_pool_deinit(const char *alias);

db_pool_t *app_db_pool_get_pool(const char *alias);

// In case a runtime application needs to increase / decrease number of connections
// in a given pool, this function adjust number of the connections to the given pool.
DBA_RES_CODE app_db_pool_set_capacity(db_pool_t *pool, size_t  new_capacity, void (*done_cb)(db_pool_t *));

// callers MUST NOT free the space returned by this function
const db_pool_cfg_t *app_db_pool_get_config(db_pool_t *pool);

// get number of connections currently preserved in the given pool
size_t  app_db_pool_get_size(db_pool_t *pool);


#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_POOL_H
