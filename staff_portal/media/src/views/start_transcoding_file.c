#include "utils.h"
#include "views.h"
#include "rpc/core.h"



RESTAPI_ENDPOINT_HANDLER(start_transcoding_file, POST, self, req)
{
#define MAX_BYTES_JOB_ID    70
    // TODO, should create async job send it to message queue,
    //  since it takes time to transcode media file
    json_t *res_body = json_object();
    json_object_set_new(res_body, "job",  json_string("903r83y03yr23rsz"));
    {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        char msg_body_raw[] = "start transxcoding video file...";
        char job_id_raw[MAX_BYTES_JOB_ID] = {0};
        size_t nwrite = strlen(msg_body_raw);
        arpc_exe_arg_t  rpc_arg = {
            .conn = req->conn->ctx->storage.entries[1].data,  .job_id = {.bytes=&job_id_raw[0],
                .len=MAX_BYTES_JOB_ID }, .msg_body = {.len=nwrite, .bytes=&msg_body_raw[0]},
            .alias = "app_mqbroker_1",  .routing_key = "rpc.media.transcode_video_file",
            .usr_data = (void *)json_object_get(jwt_claims, "profile"),
        };
        ARPC_STATUS_CODE rpc_status = app_rpc_start(&rpc_arg);
        if(rpc_status == APPRPC_RESP_ACCEPTED) {
            req->res.status = 202;
            req->res.reason = "Accepted"; // will start a new job and transcode asynchronously
        } else {
            req->res.status = 500;
        }
    }
    {
        size_t MAX_BYTES_RESP_BODY = 256;
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
    json_decref(res_body);
    app_run_next_middleware(self, req, node);
    return 0;
#undef MAX_BYTES_JOB_ID
}
