#ifndef MEDIA__MODELS_DATATYPES_H
#define MEDIA__MODELS_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <pthread.h>
#include "timer_poll.h"

typedef enum {
    DBA_RESULT_OK = 0,
    DBA_RESULT_UNKNOWN_ERROR,
    DBA_RESULT_MEMORY_ERROR,
    DBA_RESULT_OS_ERROR,
    DBA_RESULT_CONFIG_ERROR,
    DBA_RESULT_ERROR_ARG,
    DBA_RESULT_SKIPPED,
} DBA_RES_CODE;

typedef enum {
    DB_ASYNC_INITED     = 0,
    DB_ASYNC_CONN_START = 1,
    DB_ASYNC_CONN_WAITING,
    DB_ASYNC_CONN_DONE,

    DB_ASYNC_QUERY_START,
    DB_ASYNC_QUERY_WAITING,
    DB_ASYNC_QUERY_DONE,

    DB_ASYNC_FETCH_ROW_START,
    DB_ASYNC_FETCH_ROW_WAITING,
    DB_ASYNC_FETCH_ROW_DONE,

    DB_ASYNC_FREE_RESULTSET_START,
    DB_ASYNC_FREE_RESULTSET_WAITING,
    DB_ASYNC_FREE_RESULTSET_DONE,

    DB_ASYNC_CLOSE_START,
    DB_ASYNC_CLOSE_WAITING,
    DB_ASYNC_CLOSE_DONE
} dbconn_async_state;


struct db_conn_s;
struct db_pool_s;

// the target can be pool or connection object, determined in applications
typedef void (*dba_close_cb)(void *target);
typedef void (*dba_error_cb)(void *target, void *detail);
typedef void (*dba_done_cb)(void *target, void *detail);


typedef struct db_llnode_s {
    char   dummy;
    struct db_llnode_s *next;
    struct db_llnode_s *prev;
    char   data[1]; // may extend the storage space based on given type
} db_llnode_t;

typedef struct {
    char     *db_name;
    char     *db_user;
    char     *db_passwd;
    char     *db_host;
    uint16_t  db_port;
} db_conn_cfg_t;

typedef struct { // the type is for connection handler functions
    DBA_RES_CODE (*init_fn)(struct db_conn_s *, struct db_pool_s *);
    DBA_RES_CODE (*deinit_fn)(struct db_conn_s *);
    DBA_RES_CODE (*close_fn)(struct db_conn_s *, dba_done_cb);
    DBA_RES_CODE (*connect_fn)(struct db_conn_s *, dba_done_cb);
} db_conn_cbs_t;

typedef struct {
    char     *alias; // label the given pool registered in the internal global pool map
    size_t    capacity; // max number of connections to preserved
    uint32_t  idle_timeout; // in seconds
    db_conn_cfg_t conn_detail;
    db_conn_cbs_t conn_ops; 
    dba_close_cb  close_cb;
    dba_error_cb  error_cb;
} db_pool_cfg_t;

typedef struct db_conn_s {
    // handle for specific database e.g. MariaDB
    void  *lowlvl_handle;
    app_timer_poll_t  timer_poll;
    struct db_pool_s *pool; // must NOT be NULL
    // list of pending queries which can cast to `db_query_t`
    db_llnode_t  *query_entry_consumer;
    db_llnode_t  *query_entry_producer;
    dbconn_async_state  state;
    char *charset;
    char *collation;
    struct {
        uint8_t  active:1;
        uint8_t  used:1;
    } flags;
} db_conn_t;

typedef struct db_pool_s {
    db_llnode_t  *conns; // head of list that can cast to `db_conn_t`
    pthread_mutex_t lock;
    db_pool_cfg_t   cfg;
} db_pool_t;

typedef struct {
    db_llnode_t  *statements; // could run at once (if multi-statement flag is set)
    db_conn_t    *conn; // specify the connection to handle this query
    // since queries come from different requests handling in different worker threads
    // or event loops, it makes sense to specify loop at query level
    uv_loop_t    *loop;
    dba_done_cb   result_rdy_cb;
    dba_done_cb   row_fetched_cb;
    dba_done_cb   result_free_cb;
    dba_error_cb  err_cb;
    void  *result_set;
    void  *usr_data;
} db_query_t;

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_DATATYPES_H
