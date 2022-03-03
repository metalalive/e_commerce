#ifndef MEDIA__MODELS_CONNECTION_H
#define MEDIA__MODELS_CONNECTION_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"

// initalize a connection with given pool, and initialization callback for specific database
DBA_RES_CODE app_db_conn_init(db_conn_t *conn, db_pool_t *pool);

// destroy a given connection object, disconnect remote database server and free up memory
// space allocated to each member of db_conn_t
DBA_RES_CODE app_db_conn_deinit(db_conn_t *conn);

// `close connection` at here means reset the database network connection and
// return the connection object back to the pool.
DBA_RES_CODE app_db_conn_close(db_conn_t *conn, dba_done_cb done_cb);

DBA_RES_CODE app_db_conn_connect(db_conn_t *conn, dba_done_cb done_cb);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_CONNECTION_H
