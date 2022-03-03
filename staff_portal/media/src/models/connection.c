#include <h2o/memory.h>
#include "models/connection.h"

DBA_RES_CODE app_db_conn_init(db_conn_t *conn, db_pool_t *pool)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(!conn || !pool) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    memset(conn, 0, sizeof(db_conn_t));
    conn->pool = pool;
done:
    return result;
} // end of app_db_conn_init


DBA_RES_CODE app_db_conn_deinit(db_conn_t *conn)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(!conn) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    // application caller should close the connection first, then call this de-init function
    if(conn->flags.active || conn->flags.used) {
        result = DBA_RESULT_OS_ERROR;
        goto done;
    }
    if(conn->state != DB_ASYNC_INITED && conn->state != DB_ASYNC_CLOSE_DONE) {
        result = DBA_RESULT_OS_ERROR;
        goto done;
    }
    conn->pool = NULL;
done:
    return result;
} // end of app_db_conn_deinit


DBA_RES_CODE app_db_conn_close(db_conn_t *conn, dba_done_cb done_cb)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(!conn || !done_cb) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    // TODO: clean up pending query list
done:
    return result;
} // end of app_db_conn_close


DBA_RES_CODE app_db_conn_connect(db_conn_t *conn, dba_done_cb done_cb)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(!conn || !done_cb) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
done:
    return result;
} // end of app_db_conn_connect

