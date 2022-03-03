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

// de-initialize a given pool, and unregister from the global pool map
DBA_RES_CODE app_db_pool_deinit(const char *alias);

db_pool_t *app_db_pool_get_pool(const char *alias);

// callers get available connection from a given pool, then use the returned connection
//  for subsequent commands (e.g. query) , return NULL means all connections in the pool
//  are in use
db_conn_t *app_db_pool_get_conn(db_pool_t *pool);

// In case a runtime application needs to increase / decrease number of connections
// in a given pool, this function adjust number of the connections to the given pool.
DBA_RES_CODE app_db_pool_set_capacity(db_pool_t *pool, size_t  new_capacity, dba_done_cb done_cb);

// callers MUST NOT free the space returned by this function
const db_pool_cfg_t *app_db_pool_get_config(db_pool_t *pool);

// get number of connections currently preserved in the given pool
size_t  app_db_pool_get_size(db_pool_t *pool);


#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_POOL_H
