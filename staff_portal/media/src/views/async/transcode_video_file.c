
#include "rpc/core.h"

static  __attribute__((optimize("O0"))) void  api_transcode_video_file__rpc_task_handler(arpc_receipt_t *receipt)
{
    const char *RPC_RETURN_BODY = "{\"origin_res_id\": 1917, \"transcoded_res_id\": 8735, \"resolution\":\"200x150\"}";
    receipt->return_fn(receipt, RPC_RETURN_BODY, strlen(RPC_RETURN_BODY));
}
