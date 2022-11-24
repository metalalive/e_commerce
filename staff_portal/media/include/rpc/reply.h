#ifndef MEDIA__RPC_REPLY_H
#define MEDIA__RPC_REPLY_H
#ifdef __cplusplus
extern "C" {
#endif

#include <jansson.h>
#include "storage/localfs.h"
#include "rpc/datatypes.h"

struct arpc_reply_cfg_s;

typedef struct arpc_reply_cfg_s {
    void      *usr_data;
    void      *loop;
    void      *conn; // connection context to AMQP broker
    uint32_t   usr_id;
    uint32_t   timeout_ms;
    uint16_t   max_num_msgs_fetched;
    void     (*on_error)(struct arpc_reply_cfg_s *, ARPC_STATUS_CODE);
    uint8_t  (*on_update)(struct arpc_reply_cfg_s *, json_t *info, ARPC_STATUS_CODE);
    ARPC_STATUS_CODE  (*get_reply_fn)(arpc_exe_arg_t *, size_t max_nread, arpc_reply_corr_identify_fn);
    struct {
        uint8_t replyq_nonexist:1;
    } flags;
} arpc_reply_cfg_t;

void * apprpc_recv_reply_start (arpc_reply_cfg_t *);
void * apprpc_recv_reply_restart (void *ctx);
void   apprpc_reply_deinit_start (void *ctx);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_REPLY_H
