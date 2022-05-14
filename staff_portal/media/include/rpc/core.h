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
};

struct arpc_ctx_list_t {
    size_t size;
    struct arpc_ctx_t *entries;
};

void *app_rpc_conn_init(arpc_cfg_t *cfgs, size_t nitem);
void  app_rpc_conn_deinit(void *conn);

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *);
ARPC_STATUS_CODE app_rpc_get_reply(arpc_exe_arg_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_CORE_H
