#include "rpc/core.h"

static  __attribute__((optimize("O0"))) void api_complete_multipart_upload__rpc_task_handler(arpc_receipt_t *receipt)
{
    const char *RPC_RETURN_BODY = "{\"resource_id\": 18764}";
    receipt->return_fn(receipt, RPC_RETURN_BODY, strlen(RPC_RETURN_BODY));
}
