#ifndef MEDIA__MODELS_DATATYPES_H
#define MEDIA__MODELS_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <stdatomic.h>
#include <pthread.h>
#include "utils.h"
#include "timer_poll.h"

typedef enum {
    DBA_RESULT_OK = 0,
    DBA_RESULT_UNKNOWN_ERROR,
    DBA_RESULT_MEMORY_ERROR,
    DBA_RESULT_OS_ERROR,
    DBA_RESULT_LIBRARY_ERROR,
    DBA_RESULT_CONFIG_ERROR,
    DBA_RESULT_NETWORK_ERROR,
    DBA_RESULT_REMOTE_RESOURCE_ERROR,
    DBA_RESULT_ERROR_ARG,
    DBA_RESULT_POOL_BUSY,
    DBA_RESULT_CONNECTION_BUSY,
    DBA_RESULT_QUERY_STILL_PROCESSING,
    DBA_RESULT_RSET_STILL_LOADING,
    DBA_RESULT_ROW_STILL_FETCHING,
    DBA_RESULT_REST_RELEASING,
    DBA_RESULT_END_OF_RSETS_REACHED,
    DBA_RESULT_END_OF_ROWS_REACHED,
    DBA_RESULT_SKIPPED,
} DBA_RES_CODE;

typedef int  dbconn_async_state;

struct db_conn_s;
struct db_pool_s;
struct db_query_s;
struct db_query_result_s;
typedef app_llnode_t db_llnode_t;

typedef struct {
    DBA_RES_CODE app_result;
    int  uv_result;
    int  uv_event;
} db_conn_err_detail_t;

typedef struct {
    char     *db_name;
    char     *db_user;
    char     *db_passwd;
    char     *db_host;
    uint16_t  db_port;
} db_conn_cfg_t;

typedef struct { // connection operations for specific database API, registered in pool object
    DBA_RES_CODE (*global_init_fn)(struct db_pool_s *);
    DBA_RES_CODE (*global_deinit_fn)(struct db_pool_s *);
    DBA_RES_CODE (*conn_init_fn)(struct db_conn_s *, struct db_pool_s *);
    DBA_RES_CODE (*conn_deinit_fn)(struct db_conn_s *);
    void  (*error_cb)(struct db_conn_s *, db_conn_err_detail_t *detail);
    uint8_t   (*can_change_state)(struct db_conn_s *);
    timerpoll_poll_cb  state_transition;
    int       (*get_sock_fd)(struct db_conn_s *); // low-level socket file descriptor
    uint64_t  (*get_timeout_ms)(struct db_conn_s *);
    uint8_t   (*notify_query)(struct db_query_s *, struct db_query_result_s *);
    uint8_t   (*is_conn_closed)(struct db_conn_s *);
} db_3rdparty_ops_t;

typedef struct {
    char     *alias; // label the given pool registered in the internal global pool map
    size_t    capacity; // max number of connections to preserved
    uint32_t  idle_timeout; // timeout in seconds
    size_t  bulk_query_limit_kb; // size limit of bulk queries in KBytes for each connection object
    db_conn_cfg_t  conn_detail;
    db_3rdparty_ops_t  ops;
    uint8_t  skip_tls:1; // whether to enable secure connection between this app and databse server
} db_pool_cfg_t;

typedef struct { // handle for specific database e.g. MariaDB, postgreSQL
    void  *conn; // connection object at low-level specific database
    void  *resultset;
    void  *row;
} db_lowlvl_t;

typedef struct db_conn_s {
    db_lowlvl_t lowlvl;
    struct db_pool_s *pool; // must NOT be NULL
    uv_loop_t  *loop; // loop controlled by current worker thread
    struct { // list of pending / processing queries which can cast to `db_query_t`
        db_llnode_t  *head;
        db_llnode_t  *tail;
    } pending_queries;
    db_llnode_t  *processing_queries; // processing queries in bulk
    pthread_mutex_t lock;
    dbconn_async_state  state;
    app_timer_poll_t  timer_poll;
    struct {
        DBA_RES_CODE (*add_new_query)(struct db_conn_s *, struct db_query_s *);
        DBA_RES_CODE (*update_ready_queries)(struct db_conn_s *); // move pending query to ready-to list
        DBA_RES_CODE (*try_process_queries)(struct db_conn_s *, uv_loop_t *);
        DBA_RES_CODE (*try_close)(struct db_conn_s *, uv_loop_t *);
        uint8_t  (*is_closed)(struct db_conn_s *);
        int (*timerpoll_init)(uv_loop_t *loop, app_timer_poll_t *handle, int fd);
        int (*timerpoll_deinit)(app_timer_poll_t *handle);
        int (*timerpoll_change_fd)(app_timer_poll_t *handle, int new_fd);
        int (*timerpoll_start)(app_timer_poll_t *handle, uint64_t timeout_ms, uint32_t events, timerpoll_poll_cb poll_cb);
        int (*timerpoll_stop)(app_timer_poll_t *handle);
    } ops;
    struct {
        // the connection is temporarily unavailable when (re)establishing connection to database
        // server is still ongoing, or when closing a connection is ongoing.
        atomic_flag           state_changing;
        atomic_uint_least8_t  has_ready_query_to_process;
    } flags;
    // internal use
    struct {
        size_t  wr_sz;
        char    data[1];
    } bulk_query_rawbytes;
} db_conn_t;

typedef struct db_pool_s {
    struct { // head of list that can cast to `db_conn_t`
        db_llnode_t  *head;
        db_llnode_t  *tail;
    } conns; // free connections
    db_llnode_t  *locked_conns; // temporarily used by applciation callers
    struct db_conn_s *(*acquire_free_conn_fn)(struct db_pool_s *);
    DBA_RES_CODE      (*release_used_conn_fn)(struct db_conn_s *);
    uint8_t           (*is_closing_fn)(struct db_pool_s *);
    pthread_mutex_t lock;
    db_pool_cfg_t   cfg;
    // flags: bit layout
    // bit 0 : whether the application is closing this pool
    atomic_ushort   flags;
} db_pool_t;


typedef struct {
    struct {
        size_t affected;
    } num_rows;
    // TODO, add definition of each column, which is useful information applications can access
} db_query_rs_info_t;

typedef struct {
    size_t num_cols;
    char **values;
    // all data in each column is converted to NULL-terminated string
    char   data[1];
} db_query_row_info_t;

typedef struct db_query_result_s {
    DBA_RES_CODE app_result;
    struct {
        dbconn_async_state state;
        const char *alias;
        uint8_t  async:1;
    } conn;
    uint8_t  _final:1;
    void  (*free_data_cb)(void *);
    char  data[1];
} db_query_result_t;

typedef struct {
    db_pool_t  *pool;
    uv_loop_t  *loop;
    struct {
        void  (*result_rdy )(struct db_query_s *target, db_query_result_t *detail);
        void  (*row_fetched)(struct db_query_s *target, db_query_result_t *detail);
        void  (*result_free)(struct db_query_s *target, db_query_result_t *detail);
        void  (*error)(struct db_query_s *target, db_query_result_t *detail);
    } callbacks;
    struct {
        char   *entry;
        size_t  num_rs; // number of expected result sets to return
    } statements;
    struct {
        void   **entry; // array of user data passed to each callback below
        size_t   len;
    } usr_data;
} db_query_cfg_t;

typedef struct db_query_s {
    // since queries come from different requests handling in different worker threads
    // or event loops, it makes sense to specify loop at query level
    db_query_cfg_t  cfg;
    uv_async_t  notification;
    struct { // maintain a list of struct `db_query_result_t`
        db_llnode_t  *head;
        db_llnode_t  *tail;
        pthread_mutex_t lock;
        size_t num_rs_remain;
    } db_result;
    // --- for internal use ---
    size_t _stmts_tot_sz; // total size of all the statements in bytes in this query
} db_query_t;

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_DATATYPES_H
