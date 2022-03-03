#include <mysql.h>
#include "models/connection.h"
#include "models/mariadb.h"

DBA_RES_CODE app_db_mariadb_conn_init(db_conn_t *conn, db_pool_t *pool)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    MYSQL *handle = NULL;
    if(!conn || !pool) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    result = app_db_conn_init(conn, pool);
    if(result != DBA_RESULT_OK) {
        goto done;
    }
    handle = mysql_init(NULL);
    if(!handle) {
        result = DBA_RESULT_MEMORY_ERROR;
        goto error;
    }
    mysql_options(handle, MYSQL_READ_DEFAULT_GROUP, "async_queries");
    if(mysql_options(handle, MYSQL_OPT_NONBLOCK, NULL) != 0) {
        result = DBA_RESULT_CONFIG_ERROR;
        goto error;
    }
    uint32_t timeout = conn->pool->cfg.idle_timeout;
    if(mysql_options(handle, MYSQL_OPT_CONNECT_TIMEOUT, &timeout) ||
            mysql_options(handle, MYSQL_OPT_READ_TIMEOUT, &timeout) ||
            mysql_options(handle, MYSQL_OPT_WRITE_TIMEOUT, &timeout)) {
        result = DBA_RESULT_CONFIG_ERROR;
        goto error;
    }
    conn->lowlvl_handle = (void *)handle;
    goto done;
error:
    if(handle) {
        mysql_close(handle);
    }
    app_db_conn_deinit(conn);
done:
    return result;
} // end of app_db_mariadb_conn_init


DBA_RES_CODE app_db_mariadb_conn_deinit(db_conn_t *conn)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    if(!conn) {
        result = DBA_RESULT_ERROR_ARG;
        goto done;
    }
    // use blocking function, currently this function is supposed to be invoked
    // after app server received shutdown request, already completed
    // all client requests, and closed all the HTTP connections.
    mysql_close((MYSQL *)conn->lowlvl_handle);
    conn->lowlvl_handle = NULL;
    result = app_db_conn_deinit(conn);
done:
    return result;
} // end of app_db_mariadb_conn_deinit

DBA_RES_CODE app_db_mariadb_conn_connect(db_conn_t *conn, dba_done_cb done_cb)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    return result;
} // end of app_db_mariadb_conn_connect


DBA_RES_CODE app_db_mariadb_conn_close(db_conn_t *conn, dba_done_cb done_cb)
{
    DBA_RES_CODE result = DBA_RESULT_OK;
    return result;
} // end of app_db_mariadb_conn_close

