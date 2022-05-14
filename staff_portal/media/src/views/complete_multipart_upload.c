#include <openssl/sha.h>
#include "utils.h"
#include "views.h"
#include "rpc/core.h"

static __attribute__((optimize("O0"))) ARPC_STATUS_CODE api_complete_multipart_upload__render_rpc_replyq(
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    uint32_t usr_prof_id = (uint32_t) json_integer_value((json_t *)args->usr_data);
    if(usr_prof_id > 0) {
        snprintf(wr_buf, wr_sz, name_pattern, usr_prof_id);
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api_complete_multipart_upload__render_rpc_replyq

static __attribute__((optimize("O0"))) ARPC_STATUS_CODE api_complete_multipart_upload__render_rpc_corr_id (
        const char *name_pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_OK;
    size_t md_hex_sz = (SHA_DIGEST_LENGTH << 1) + 1;
    size_t tot_wr_sz = strlen(name_pattern) + md_hex_sz;
    if(tot_wr_sz > wr_sz) {
        return APPRPC_RESP_MEMORY_ERROR;
    }
    uint32_t usr_prof_id = (uint32_t) json_integer_value((json_t *)args->usr_data);
    if(usr_prof_id > 0) {
        SHA_CTX  sha_ctx = {0};
        SHA1_Init(&sha_ctx);
        SHA1_Update(&sha_ctx, (const char *)&usr_prof_id, sizeof(usr_prof_id));
        SHA1_Update(&sha_ctx, (const char *)&args->_timestamp, sizeof(args->_timestamp));
        char md[SHA_DIGEST_LENGTH] = {0};
        char md_hex[md_hex_sz];
        SHA1_Final((unsigned char *)&md[0], &sha_ctx);
        app_chararray_to_hexstr(&md_hex[0], md_hex_sz - 1, &md[0], SHA_DIGEST_LENGTH);
        md_hex[md_hex_sz - 1] = 0x0;
        snprintf(wr_buf, wr_sz, name_pattern, &md_hex[0]);
    } else {
        status = APPRPC_RESP_ARG_ERROR;
    }
    return status;
} // end of api_complete_multipart_upload__render_rpc_corr_id



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
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
#define MAX_BYTES_MSG_BODY  128
#define MAX_BYTES_JOB_ID    70
        char msg_body_raw[MAX_BYTES_MSG_BODY] = {0};
        char job_id_raw[MAX_BYTES_JOB_ID] = {0};
        size_t nwrite = json_dumpb((const json_t *)req_body, &msg_body_raw[0], MAX_BYTES_MSG_BODY, JSON_COMPACT);
        arpc_exe_arg_t  rpc_arg = {
            .conn = req->conn->ctx->storage.entries[1].data,  .job_id = {.bytes=&job_id_raw[0],
                .len=MAX_BYTES_JOB_ID }, .msg_body = {.len=nwrite, .bytes=&msg_body_raw[0]},
            .alias = "app_mqbroker_1",  .routing_key = "rpc.media.complete_multipart_upload",
            .usr_data = (void *)json_object_get(jwt_claims, "profile"),
        };
        ARPC_STATUS_CODE rpc_status = app_rpc_start(&rpc_arg);
        if(rpc_status == APPRPC_RESP_ACCEPTED) {
            json_object_set_new(res_body, "job_id", json_string(rpc_arg.job_id.bytes));
        } else {
            req->res.status = 500;
            req->res.reason = "publish message error";
            goto done;
        }
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
#define  MAX_BYTES_RESP_BODY  (100 + MAX_BYTES_JOB_ID)
    {
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
    if(req_body) {
        json_decref(req_body);
    }
    json_decref(res_body);
    app_run_next_middleware(self, req, node);
    return 0;
#undef  MAX_BYTES_MSG_BODY
#undef  MAX_BYTES_JOB_ID
#undef  MAX_BYTES_RESP_BODY
} // end of complete_multipart_upload()
