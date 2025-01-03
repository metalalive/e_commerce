#ifndef MEDIA__MODELS_MARIADB_H
#define MEDIA__MODELS_MARIADB_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"

enum _dbconn_async_state {
    DB_ASYNC_INITED     = 0,
    DB_ASYNC_CONN_START = 1,
    DB_ASYNC_CONN_WAITING,
    DB_ASYNC_CONN_DONE,

    DB_ASYNC_QUERY_START,
    DB_ASYNC_QUERY_WAITING,
    DB_ASYNC_QUERY_READY,

    DB_ASYNC_CHECK_CURRENT_RESULTSET,
    DB_ASYNC_MOVE_TO_NEXT_RESULTSET_START,
    DB_ASYNC_MOVE_TO_NEXT_RESULTSET_WAITING,
    DB_ASYNC_MOVE_TO_NEXT_RESULTSET_DONE,

    DB_ASYNC_FETCH_ROW_START,
    DB_ASYNC_FETCH_ROW_WAITING,
    DB_ASYNC_FETCH_ROW_READY,

    DB_ASYNC_FREE_RESULTSET_START,
    DB_ASYNC_FREE_RESULTSET_WAITING,
    DB_ASYNC_FREE_RESULTSET_DONE,

    DB_ASYNC_CLOSE_START,
    DB_ASYNC_CLOSE_WAITING,
    DB_ASYNC_CLOSE_DONE
}; // end of enum _dbconn_async_state

DBA_RES_CODE  app_db_mariadb__global_init(db_pool_t *);

DBA_RES_CODE  app_db_mariadb__global_deinit(db_pool_t *);

DBA_RES_CODE app_db_mariadb_conn_init(db_conn_t *);

DBA_RES_CODE app_db_mariadb_conn_deinit(db_conn_t *);

uint8_t  app_mariadb_acquire_state_change(db_conn_t *);

void app_mariadb_async_state_transition_handler(app_timer_poll_t *target, int uv_status, int event_flags);

uint8_t  app_mariadb_conn_notified_query_callback(db_query_t *, db_query_result_t *);

uint8_t  app_mariadb_conn_is_closed(db_conn_t *);

int app_db_mariadb_get_sock_fd(db_conn_t *); // get low-level socket file descriptor

uint64_t  app_db_mariadb_get_timeout_ms(db_conn_t *);

void  app_db_mariadb__cfg_ops(db_3rdparty_ops_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_MARIADB_H
