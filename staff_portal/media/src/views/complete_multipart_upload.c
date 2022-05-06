#include "utils.h"
#include "views.h"
#include "rpc/core.h"

// TODO:another API endpoint for checking status of each upload request that hasn't expired yet
RESTAPI_ENDPOINT_HANDLER(complete_multipart_upload, PATCH, self, req)
{
    json_error_t  j_err = {0};
    json_t *res_body = json_object();
    json_t *req_body = json_loadb((const char *)req->entity.base, req->entity.len,
               JSON_REJECT_DUPLICATES, &j_err);
    if(j_err.line >= 0 || j_err.column >= 0) {
        json_object_set_new(res_body, "message", json_string("parsing error on request body"));
        req->res.status = 400;
    }
    const char *resource_id = json_string_value(json_object_get(req_body, "resource_id"));
    uint32_t req_seq = (uint32_t) json_integer_value(json_object_get(req_body, "req_seq"));
    if(!resource_id) {
        json_object_set_new(res_body, "resource_id", json_string("missing resource_id for committed uploaded file"));
        req->res.status = 400;
    } // TODO, SQL injection check
    if(req_seq == 0) {
        json_object_set_new(res_body, "req_seq", json_string("missing req_seq for upload request"));
        req->res.status = 400;
    }
    if(req->res.status == 400) {
        req->res.reason = "invalid ID";
        goto done;
    }
    { // serialize the URL parameters then pass it to AMQP broker
#define MAX_BYTES_MSG_BODY  128
        char msg_body_raw[MAX_BYTES_MSG_BODY] = {0};
        char job_id_raw[41] = {0};
        size_t nwrite = json_dumpb((const json_t *)req_body, &msg_body_raw[0], MAX_BYTES_MSG_BODY, JSON_COMPACT);
        arpc_exe_arg_t  rpc_arg = {
            .conn = req->conn->ctx->storage.entries[1].data,  .job_id = &job_id_raw[0],
            .msg_body = {.len = nwrite, .bytes = &msg_body_raw[0]},
            .routing_key = "rpc.media.complete_multipart_upload",
        };
        ARPC_STATUS_CODE rpc_status = app_rpc_start(&rpc_arg);
        if(rpc_status == APPRPC_RESP_ACCEPTED) {
            json_object_set_new(res_body, "job_id", json_string(rpc_arg.job_id));
        } else {
            req->res.status = 500;
            req->res.reason = "publish message error";
            goto done;
        }
#undef  MAX_BYTES_MSG_BODY
    }
    //// json_object_set_new(res_body, "resource_id", json_string(resource_id));
    //// json_object_set_new(res_body, "req_seq",     json_string(req_seq));
    //// json_object_set_new(res_body, "mime_type",   json_string("video/mp4"));
    //// json_object_set_new(res_body, "last_update", json_string("2022-01-26"));
    //// json_object_set_new(res_body, "checksum", json_string("b17a33501506315093eb082"));
    //// json_object_set_new(res_body, "alg"     , json_string("md5"));
    req->res.status = 202; // accepted, message broker acknowledges the published message
    req->res.reason = "Created"; // or renew (and erase old one) successfully
done:
#define  MAX_BYTES_RESP_BODY  230
    {
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
#undef  MAX_BYTES_RESP_BODY
    if(req_body) {
        json_decref(req_body);
    }
    json_decref(res_body);
    app_run_next_middleware(self, req, node);
    return 0;
} // end of complete_multipart_upload()
