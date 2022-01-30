#include <stdio.h>
#include <h2o.h>
#include <jansson.h>

#include "routes.h"

RESTAPI_ENDPOINT_HANDLER(initiate_multipart_upload, POST, self, req)
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
    return 0;
} // end of initiate_multipart_upload


int decode_query_param(char *data, json_t *map) {
    // this function does NOT check inproper characters appeared in name/value field
    // and treat all value as string by default.
    char *tok = data;
    char *ptr_kvpair = NULL;
    for(tok = strtok_r(tok, "&", &ptr_kvpair); tok; tok = strtok_r(NULL, "&", &ptr_kvpair))
    { // strtok_r is thread-safe
        char *ptr = NULL;
        char *name  = strtok_r(tok,  "=", &ptr);
        char *value = strtok_r(NULL, "=", &ptr);
        if(!name) {
            continue;
        }
        json_t *obj_val = NULL;
        if(value) {
            obj_val = json_string(value);
        } else {
            obj_val = json_true();
        }
        json_object_set_new(map, name, obj_val);
        //fprintf(stdout, "[debug] raw data of query params: %s = %s \n", name, value);
    }
    return 0;
} // end of decode_query_param


RESTAPI_ENDPOINT_HANDLER(upload_part, POST, self, req)
{
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *part_str = json_string_value(json_object_get(qparams, "part"));
    int part_num = part_str ? (int)strtol(part_str, NULL, 10) : 0;
    // TODO
    // * validate the untrusted values
    // * stream request body, parse the chunk with multipart/form parser,
    // * hash the chunks
    // * and write it to tmp file
    json_t *res_body = json_object();
    json_object_set_new(res_body, "checksum", json_string("b17a33501506315093eb082"));
    json_object_set_new(res_body, "alg", json_string("md5"));
    json_object_set_new(res_body, "part", json_integer(part_num));
    req->res.status = 200;
    req->res.reason = "OK";
    {
        size_t MAX_BYTES_RESP_BODY = 128;
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
    json_decref(res_body);
    json_decref(qparams);
    return 0;
} // end of upload_part()


// TODO:another API endpoint for checking status of each upload request that hasn't expired yet
RESTAPI_ENDPOINT_HANDLER(complete_multipart_upload, PATCH, self, req)
{
    json_t *res_body = json_object();
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "resource_id"));
    const char *upload_id   = json_string_value(json_object_get(qparams, "upload_id"));
    if(!resource_id || !upload_id) {
        req->res.status = 400;
        req->res.reason = "invalid ID";
        goto done;
    }
    json_object_set_new(res_body, "resource_id", json_string(resource_id));
    json_object_set_new(res_body, "upload_id",   json_string(upload_id));
    json_object_set_new(res_body, "mime_type",   json_string("video/mp4"));
    json_object_set_new(res_body, "last_update", json_string("2022-01-26"));
    json_object_set_new(res_body, "checksum", json_string("b17a33501506315093eb082"));
    json_object_set_new(res_body, "alg"     , json_string("md5"));
    req->res.status = 201; // or 200
    req->res.reason = "Created"; // or renew (and erase old one) successfully
done:
    {
        size_t MAX_BYTES_RESP_BODY = 200;
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
    json_decref(res_body);
    json_decref(qparams);
    return 0;
} // end of complete_multipart_upload()


RESTAPI_ENDPOINT_HANDLER(abort_multipart_upload, DELETE, self, req)
{
    req->res.status = 204;
    req->res.reason = "request deleted";
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));
    // DELETE request cannot include response body in libh2o ?
    h2o_send_inline(req, "", 0);
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
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(start_transcoding_file, POST, self, req)
{
    // TODO, should create async job send it to message queue,
    //  since it takes time to transcode media file
    json_t *res_body = json_object();
    json_object_set_new(res_body, "job",  json_string("903r83y03yr23rsz"));
    req->res.status = 202;
    req->res.reason = "Accepted"; // will start a new job and transcode asynchronously
    {
        size_t MAX_BYTES_RESP_BODY = 256;
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
    }
    json_decref(res_body);
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(discard_ongoing_job, DELETE, self, req)
{ // TODO:job ID required
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
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
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(monitor_job_progress, GET, self, req)
{
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *job_id = json_string_value(json_object_get(qparams, "id"));
    const char *body = "{\"elementary_streams\": [], \"outputs\":[]}";
    if(job_id) {
        req->res.status = 200;
        req->res.reason = "OK";
    } else {
        req->res.status = 404;
        req->res.reason = "job not found";
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, body, strlen(body));
    json_decref(qparams);
    return 0;
}


// fetch file (image or video, audio playlist for streaming)
// TODO: seperate API endpoint for public media resource ?
RESTAPI_ENDPOINT_HANDLER(fetch_entire_file, GET, self, req)
{
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    const char *transcode_version = json_string_value(json_object_get(qparams, "trncver")); // optional in image
    if(resource_id) {
        req->res.status = 200;
        req->res.reason = "OK";
    } else {
        req->res.status = 404;
        req->res.reason = "file not found";
    }
    const char *body = "";
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/oct-stream"));    
    h2o_send_inline(req, body, strlen(body));
    return 0;
} // end of fetch_entire_file


RESTAPI_ENDPOINT_HANDLER(get_next_media_segment, GET, self, req)
{ // grab next media segment (usually audio/video) when running HLS protocol
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    const char *transcode_version = json_string_value(json_object_get(qparams, "trncver"));
    const char *body = "";
    if(resource_id && transcode_version) {
        req->res.status = 200;
        req->res.reason = "OK";
    } else {
        req->res.status = 404;
        req->res.reason = "file not found";
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/oct-stream"));    
    h2o_send_inline(req, body, strlen(body));
    return 0;
}

RESTAPI_ENDPOINT_HANDLER(discard_file, DELETE, self, req)
{
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
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
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(edit_file_acl, PATCH, self, req)
{
    int idx = 0;
    json_t *item = NULL;
    json_t *req_body = json_loadb(req->entity.base, req->entity.len, 0, NULL);
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = json_string_value(json_object_get(qparams, "id"));
    if(!resource_id) {
        req->res.status = 400;
        req->res.reason = "missing resource id in URL";
        goto done;
    }

    if(!req_body || !json_is_array(req_body)) {
        req->res.status = 400;
        req->res.reason = "json decode error";
        goto done;
    }
    json_array_foreach(req_body, idx, item) {
        int usr_id   = (int) json_integer_value(json_object_get(item, "usr_id"));
        int usr_type = (int) json_integer_value(json_object_get(item, "usr_type"));
        json_t *access_control = json_object_get(item, "access_control");
        if(usr_id == 0 || usr_type == 0 || !access_control) {
            req->res.status = 400;
            req->res.reason = "missing fields";
            goto done;
        }
    } // end of iteration
    req->res.status = 200;
    req->res.reason = "OK";
done:
    {
        const char *body = "";
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body, strlen(body));
    }
    json_decref(req_body);
    json_decref(qparams);
    return 0;
} // end of edit_file_acl


RESTAPI_ENDPOINT_HANDLER(read_file_acl, GET, self, req)
{
    const char *body = "[{\"usr_id\": 728462, \"read\":True, \"renew\":False}, {\"usr_id\": 199204, \"read\":False, \"renew\":True}]";
    json_t *qparams = json_object();
    decode_query_param(&req->path.base[req->query_at + 1], qparams);
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
    return 0;
}

