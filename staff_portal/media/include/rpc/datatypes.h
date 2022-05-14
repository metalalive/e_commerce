#ifndef MEDIA__RPC_DATATYPES_H
#define MEDIA__RPC_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>

typedef enum {
    APPRPC_RESP_OK = 1,
    APPRPC_RESP_ACCEPTED,
    APPRPC_RESP_OS_ERROR,
    APPRPC_RESP_MEMORY_ERROR,
    APPRPC_RESP_ARG_ERROR,
    APPRPC_RESP_MSGQ_CONNECTION_ERROR,
    APPRPC_RESP_MSGQ_PUBLISH_ERROR,
    APPRPC_RESP_MSGQ_REMOTE_UNKNOWN_ERROR,
} ARPC_STATUS_CODE;

typedef struct {
    const char *alias; // identify the broker configuration which is used to send command
    void *conn;
    char *routing_key;
    uint64_t _timestamp;
    void *usr_data;
    // RPC function will reply with valid job ID for successfully published message
    struct {
        size_t len;
        char *bytes;
    } job_id;
    struct {
        size_t len;
        char  *bytes;
    } msg_body;
} arpc_exe_arg_t;

typedef struct {
    uint8_t durable:1;
    uint8_t passive:1;
    uint8_t exclusive:1;
    uint8_t auto_delete:1;
} arpc_qcfg_flg_t;

struct arpc_cfg_bind_reply_s;

typedef ARPC_STATUS_CODE (*arpc_replyq_render_fn)(const char *pattern, arpc_exe_arg_t *, char *wr_buf, size_t wr_sz);

typedef struct arpc_cfg_bind_reply_s {
    struct {
        char *name_pattern;
        arpc_replyq_render_fn  render_fn;
    } queue;
    struct {
        char *name_pattern;
        arpc_replyq_render_fn  render_fn;
    } correlation_id;
    // char *exchange_name; // TODO, figure out how to send return value to reply queue with non-default exchange
    int (*task_handler_fn)(char *msg_body, void *arg);
    uint32_t  ttl_sec;
    arpc_qcfg_flg_t  flags;
} arpc_cfg_bind_reply_t;

typedef struct {
    arpc_cfg_bind_reply_t reply;
    char *q_name;
    char *exchange_name;
    char *routing_key;
    size_t  max_msgs_pending;
    arpc_qcfg_flg_t  flags;
} arpc_cfg_bind_t; // per-queue config type

typedef struct {
    char *alias;
    struct {
        char     *username;
        char     *password;
        char     *host;
        uint16_t  port;
    } credential;
    struct {
        char   *vhost; // virtual host is determined at which an user logins to given broker
        size_t  max_channels;
        size_t  max_kb_per_frame;
    } attributes; // connection-level attributes
    H2O_VECTOR(arpc_cfg_bind_t) bindings;
} arpc_cfg_t; // per-host config type

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_DATATYPES_H
