#include "utils.h"
#include "views.h"


RESTAPI_ENDPOINT_HANDLER(edit_file_acl, PATCH, self, req)
{
    int idx = 0;
    json_t *item = NULL;
    json_t *req_body = json_loadb(req->entity.base, req->entity.len, 0, NULL);
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
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
    app_run_next_middleware(self, req, node);
    return 0;
} // end of edit_file_acl
