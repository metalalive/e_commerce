#ifndef MEIDA__DATATYPES_H
#define MEIDA__DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <stdio.h>
#include <stdatomic.h>
#include <sys/socket.h>
#include <openssl/crypto.h>
#include <openssl/x509_vfy.h>
#include <h2o.h>
#include <rhonabwy.h>

#include "transcoder/datatypes.h"
#include "storage/datatypes.h"
#include "rpc/datatypes.h"

// TODO: find better way to synchronize from common/data/app_code.json
#define APP_CODE      3
#define APP_LABEL     "media"
#define APP_LABEL_LEN (sizeof(APP_LABEL) - 1) // 5

#define APP_FILETYPE_LABEL_VIDEO "video"
#define APP_FILETYPE_LABEL_IMAGE "image"

#define APP_GRACEFUL_SHUTDOWN 1
#define APP_HARD_SHUTDOWN     2

#define DATETIME_STR_SIZE      20
#define USR_ID_STR_SIZE        10
#define UPLOAD_INT2HEX_SIZE(x) (sizeof(x) << 1)
// TODO, synchronize following parameters with DB migration config file
#define APP_RESOURCE_ID_SIZE        8
#define APP_TRANSCODED_VERSION_SIZE 2

#define API_QPARAM_LABEL__RESOURCE_ID   "res_id"
#define API_QPARAM_LABEL__STREAM_DOC_ID "doc_id"
#define API_QPARAM_LABEL__DOC_DETAIL    "d_detail"

// valid code options represented for quota arrangement in this application
typedef enum { MAX_KBYTES_CONSUMED_SPACE = 1, MAX_TRANSCODING_JOBS = 2 } quota_mat_code_options;

typedef void (*h2o_uv_socket_cb)(uv_stream_t *listener, int status);

typedef enum en_run_mode_t {
    RUN_MODE_MASTER = 0,
    RUN_MODE_DAEMON // TODO
} run_mode_t;

struct app_jwks_t {
    jwks_t      *handle;
    char        *src_url;
    char        *ca_path;
    char        *ca_format; // "PEM" or "DES"
    time_t       last_update;
    unsigned int max_expiry_secs;
    // check whether there is any worker thread rotating the jwks (from remote auth server)
    // , this field works atomically to protect state of key rotation operation among
    //  multiple workers
    atomic_flag is_rotating;
}; // end of app_jwks_t

struct app_cfg_security_t {
    SSL_CTX *ctx;
    // TODO:
    // * handle SNI and different hostnames for the same certificate
    // * ORIGIN frame in http/2 (RFC 8336)
    // h2o_iovec_t *http2_origin_frame;
};

typedef struct {
    // mirror of host config
    h2o_hostconf_t *hostconf;
    // network handles (either TCP or UDP) for current listener, the handle is also
    // manipulated by main thread.
    uv_handle_t *nt_handle;
    // security object  e.g. each includes server certificate, raw public key (if required) ...
    struct app_cfg_security_t security;
    // Note this is app server which doesn't proxy incoming requests
} app_cfg_listener_t;

typedef struct {
    h2o_globalconf_t server_glb_cfg;
    // one app process may require one or more than one listeners
    app_cfg_listener_t **listeners;
    unsigned int         num_listeners;
    FILE                *pid_file;
    int                  error_log_fd;
    // app-level access log , currently there is to need for per-path access log
    h2o_access_log_filehandle_t *access_logger;
    unsigned int                 max_connections;
    run_mode_t                   run_mode;
    // pointer to  notification in each running threads, which can be accessed when signal handling
    // callback function is invoked
    H2O_VECTOR(h2o_multithread_receiver_t *) server_notifications;
    // length of internal queue for caching TCP fastopen cookies
    unsigned int  tfo_q_len;
    time_t        launch_time;
    h2o_barrier_t workers_sync_barrier;
    H2O_VECTOR(asa_cfg_t) storages;
    // all members in the `state` struct must be modified atomically under multithreaded application
    struct {
        atomic_int num_curr_connections; // number of currently handled incoming connections
        int        num_curr_sessions;    // number of opened incoming connections
    } state;
    struct {
        char        *path;
        unsigned int threshold_bytes;
    } tmp_buf; // in case of handling huge data of concurrently incoming requests
    struct app_jwks_t jwks;
    H2O_VECTOR(arpc_cfg_t) rpc;
    aav_cfg_transcode_t transcoder;
    // pointer to path where current executable is placed
    const char *exe_path;
    // number of workers in the app, defaults to number of CPUs, unrelated to number of listeners
    H2O_VECTOR(uv_thread_t) workers;
    // atomic entity among threads & asynchronous interrupts
    volatile sig_atomic_t shutdown_requested; // 1 = graceful shutdown, 2 = hard shutdown
} app_cfg_t;

// data required for network handle (libuv)
typedef struct {
    int ai_flags;
    int ai_family;
    int ai_socktype;
    int ai_protocol;
} uv_nt_handle_data;

#define RESTAPI_HANDLER_ARGS(hdlr_var, req_var) h2o_handler_t *hdlr_var, h2o_req_t *req_var

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__DATATYPES_H
