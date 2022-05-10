#ifndef MEDIA__RPC_CORE_H
#define MEDIA__RPC_CORE_H
#ifdef __cplusplus
extern "C" {
#endif

#include "rpc/datatypes.h"

void *app_rpc_conn_init(arpc_cfg_t *cfgs, size_t nitem);
void  app_rpc_conn_deinit(void *conn);

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *);
ARPC_STATUS_CODE app_rpc_get_reply(arpc_exe_arg_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_CORE_H
