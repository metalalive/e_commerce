#ifndef MEDIA__MODELS_MARIADB_H
#define MEDIA__MODELS_MARIADB_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"

DBA_RES_CODE app_db_mariadb_conn_init(db_conn_t *conn, db_pool_t *pool);

DBA_RES_CODE app_db_mariadb_conn_deinit(db_conn_t *conn);

DBA_RES_CODE app_db_mariadb_conn_close(db_conn_t *conn, dba_done_cb done_cb);

DBA_RES_CODE app_db_mariadb_conn_connect(db_conn_t *conn, dba_done_cb done_cb);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_MARIADB_H
