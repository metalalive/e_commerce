#include "utils.h"
#include "views.h"
#include "models/pool.h"
#include "models/query.h"



RESTAPI_ENDPOINT_HANDLER(fetch_video_stream_element, GET, self, req)
{ // grab next media segment (usually audio/video) when running HLS protocol
    json_t *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
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
    app_run_next_middleware(self, req, node);
    return 0;
}
