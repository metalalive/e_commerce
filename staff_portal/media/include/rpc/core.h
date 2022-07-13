#ifndef MEDIA__RPC_CORE_H
#define MEDIA__RPC_CORE_H
#ifdef __cplusplus
extern "C" {
#endif

#include <amqp_tcp_socket.h>
#include "rpc/datatypes.h"

struct arpc_ctx_t {
    arpc_cfg_t  *ref_cfg;
    amqp_socket_t *sock;
    amqp_connection_state_t  conn;
    uint8_t consumer_setup_done;
};

struct arpc_ctx_list_t {
    size_t size;
    struct arpc_ctx_t *entries;
};

void *app_rpc_conn_init(arpc_cfg_t *cfgs, size_t nitem);
void  app_rpc_conn_deinit(void *ctx);

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *);
ARPC_STATUS_CODE app_rpc_get_reply(arpc_exe_arg_t *);

ARPC_STATUS_CODE app_rpc_consume_message(void *ctx, void *loop);

ARPC_STATUS_CODE app_rpc_close_connection(void *ctx);
ARPC_STATUS_CODE app_rpc_open_connection(void *ctx);

void *app_rpc_context_lookup(void *ctxes, const char *alias);
arpc_cfg_t *app_rpc_get_config(void *ctx);
int         app_rpc_get_sockfd(void *ctx);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_CORE_H
