#include "utils.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"


/*
    {
        // TODO, figure out how to use generator to stream the response data
        h2o_generator_t generator = {NULL, NULL};
        json_t *res_body = json_object();
        // upld id : sha1 digest, used to identify current upload request
        json_object_set_new(res_body, "upld_id", json_string("9j3r8t483ugi32ut"));
        req->res.status = 200;
        req->res.reason = "OK";
        {
            size_t MAX_BYTES_RESP_BODY = 128;
            char body_raw[MAX_BYTES_RESP_BODY];
            size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],
                    MAX_BYTES_RESP_BODY, JSON_COMPACT);
            h2o_iovec_t body = h2o_strdup(&req->pool, &body_raw[0], nwrite);
            size_t bufcnt = 1;
            h2o_add_header(NULL, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));
            h2o_start_response(req, &generator);
            h2o_send(req, &body, bufcnt, H2O_SEND_STATE_FINAL);
        }
        json_decref(res_body);
        app_run_next_middleware(self, req, node);
    }
 * */
RESTAPI_ENDPOINT_HANDLER(abort_multipart_upload, DELETE, self, req)
{
    req->res.status = 204;
    req->res.reason = "request deleted";
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));
    // DELETE request cannot include response body in libh2o ?
    h2o_send_inline(req, "", 0);
    app_run_next_middleware(self, req, node);
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(single_chunk_upload, POST, self, req)
{
    json_t *res_body = json_array();
    for(int idx = 0; idx < 2; idx++) {
        json_t *item = json_object();
        json_object_set_new(item, "resource_id", json_string("r8fj3Il"));
        json_object_set_new(item, "file_name",   json_string("some_file.jpg"));
        json_object_set_new(item, "mime_type",   json_string("video/mp4"));
        json_object_set_new(item, "last_update", json_string("2022-01-26"));
        json_object_set_new(item, "checksum", json_string("b17a33501506315093eb082"));
        json_object_set_new(item, "alg"     , json_string("md5"));
        json_array_append_new(res_body, item);
    } // end of for-loop
    req->res.status = 201;
    req->res.reason = "file uploaded";
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
}


RESTAPI_ENDPOINT_HANDLER(discard_ongoing_job, DELETE, self, req)
{ // TODO:job ID required
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *job_id = json_string_value(json_object_get(qparams, "id"));
    if(job_id) {
        req->res.status = 204; // or 410 if the job has been done before receiving this request
        req->res.reason = "job discarded";
    } else {
        req->res.status = 410;
        req->res.reason = "job does NOT exist";
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, "", 0);
    json_decref(qparams);
    app_run_next_middleware(self, req, node);
    return 0;
}

RESTAPI_ENDPOINT_HANDLER(discard_file, DELETE, self, req)
{
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    const char *body = "";
    if(resource_id) {
        req->res.status = 204;
        req->res.reason = "resource deleted";
    } else {
        req->res.status = 410;
        req->res.reason = "resource already deleted";
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, body, strlen(body));
    // h2o_send_inline(req, "", 0);
    app_run_next_middleware(self, req, node);
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(read_file_acl, GET, self, req)
{
    const char *body = "[{\"usr_id\": 728462, \"read\":True, \"renew\":False}, {\"usr_id\": 199204, \"read\":False, \"renew\":True}]";
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    if(resource_id) {
        req->res.status = 200;
        req->res.reason = "OK";
    } else {
        req->res.status = 404;
        req->res.reason = "resource not found";
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, body, strlen(body));
    json_decref(qparams);
    app_run_next_middleware(self, req, node);
    return 0;
}

