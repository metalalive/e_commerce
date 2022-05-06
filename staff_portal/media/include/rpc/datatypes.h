#ifndef MEDIA__RPC_DATATYPES_H
#define MEDIA__RPC_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>

typedef enum {
    APPRPC_RESP_ACCEPTED = 1,
    APPRPC_RESP_OS_ERROR,
    APPRPC_RESP_MEMORY_ERROR,
    APPRPC_RESP_ARG_ERROR,
} ARPC_STATUS_CODE;

typedef struct {
    uint8_t durable:1;
    uint8_t passive:1;
    uint8_t exclusive:1;
    uint8_t auto_delete:1;
} arpc_qcfg_flg_t;

typedef struct {
    char *q_name_pattern;
    char *exchange_name;
    int (*task_handler_fn)(char *msg_body, void *arg);
    size_t ttl_sec;
    arpc_qcfg_flg_t  flags;
} arpc_cfg_bind_reply_t;

typedef struct {
    char *q_name;
    char *exchange_name;
    char *routing_key;
    arpc_qcfg_flg_t  flags;
} arpc_cfg_bind_t; // per-queue config type

typedef struct {
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
    H2O_VECTOR(arpc_cfg_bind_reply_t) binding_reply;
} arpc_cfg_t; // per-host config type

typedef struct {
    void *conn;
    char *routing_key;
    char *job_id;
    struct {
        size_t len;
        char  *bytes;
    } msg_body;
} arpc_exe_arg_t;

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_DATATYPES_H
