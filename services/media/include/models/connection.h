#ifndef MEDIA__MODELS_CONNECTION_H
#define MEDIA__MODELS_CONNECTION_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"

// initalize a connection with given pool, and initialization callback for specific database
DBA_RES_CODE app_db_conn_init(db_pool_t *pool, db_conn_t **created);

// destroy a given connection object, disconnect remote database server and free up memory
// space allocated to each member of db_conn_t
DBA_RES_CODE app_db_conn_deinit(db_conn_t *conn);

db_query_t *app_db_conn_get_first_query(db_conn_t *conn);

DBA_RES_CODE  app_db_conn_try_evict_current_processing_query(db_conn_t *conn);

DBA_RES_CODE app_db_async_add_poll_event(db_conn_t *conn, uint32_t event_flags);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_CONNECTION_H
