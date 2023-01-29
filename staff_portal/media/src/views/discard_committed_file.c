#include "utils.h"
#include "views.h"

RESTAPI_ENDPOINT_HANDLER(discard_committed_file, DELETE, self, req)
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
