#include <curl/curl.h>
#include "utils.h"
#include "base64.h"
#include "views.h"
#include "models/pool.h"
#include "storage/cfg_parser.h"
#include "transcoder/file_processor.h"

#define   APP_UPDATE_INTERVAL_SECS_KEYFILE      60.0f

static  void  _api_initiate_file_stream__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *qparams, json_t *res_body)
{
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    size_t  nb_required = json_dumpb(res_body, NULL, 0, 0);
    if(req->res.status == 0)
        req->res.status = 500;
    if(nb_required > 0) {
        char  body[nb_required + 1];
        size_t  nwrite = json_dumpb(res_body, &body[0], nb_required, JSON_COMPACT);
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, nwrite);
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    json_decref(res_body);
    json_decref(qparams);
    // TODO, dealloc jwt if created for ACL check
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_initiate_file_stream__deinit_primitives


static void _api_atfp_init_stream__done_cb(atfp_t *processor)
{
    json_t  *resp_body = NULL;
    json_t  *err_info = processor->data.error;
    json_t  *spec = processor->data.spec;
    json_t  *qparams  = spec;
    h2o_req_t *req = (h2o_req_t *) json_integer_value(json_object_get(spec, "_http_req"));
    h2o_handler_t *hdlr = (h2o_handler_t *) json_integer_value(json_object_get(spec, "_http_handler"));
    app_middleware_node_t *node = (app_middleware_node_t *) json_integer_value(
            json_object_get(spec, "_middleware_node"));
    if(json_object_size(err_info) == 0) {
        json_decref(err_info);
        resp_body = json_object_get(spec, "return_data");
    } else {
        resp_body = err_info;
    }
    req->res.status = (int) json_integer_value(json_object_get(spec, "http_resp_code"));
    processor->data.error = NULL;
    processor->data.spec = NULL;
    _api_initiate_file_stream__deinit_primitives (req, hdlr, node, qparams, resp_body);
} // end of _api_atfp_init_stream__done_cb


static void _api_init_fstream__verify_resource_id_done (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *qparams  = usr_args[3];
    json_t *err_info = usr_args[4];
    int _resp_status =  api_http_resp_status__verify_resource_id (node, result, err_info);
    if(_resp_status != 403) {
        req->res.status = _resp_status;
    } else { // TODO, check ACL prior to authentication
        json_object_clear(err_info);
    }
    if(json_object_size(err_info) == 0) {
        json_object_set_new(qparams, "last_upld_req", json_integer(result->upld_req));
        json_object_set_new(qparams, "resource_owner_id", json_integer(result->owner_usr_id));
        app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
        app_save_ptr_to_hashmap(node->data, "qparams", (void *)qparams);
        app_run_next_middleware(hdlr, req, node);
    } else {
        _api_initiate_file_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
    }
} // end of  _api_init_fstream__verify_resource_id_done

static int api_acl_middleware__init_fstream (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node)
{
    json_t *err_info = json_object(),  *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = app_resource_id__url_decode(qparams, err_info);
    if(!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t out_len = 0;
        char *_res_id_encoded = (char *) base64_encode((const unsigned char *)resource_id,
                  strlen(resource_id), &out_len);
        json_object_set_new(qparams, "res_id_encoded", json_string(_res_id_encoded));
        free(_res_id_encoded);
        _res_id_encoded = (char *)json_string_value(json_object_get(qparams, "res_id_encoded"));
        void *usr_args[5] = {req, hdlr, node, qparams, err_info};
        aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=5}, .resource_id=_res_id_encoded,
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                .callback=_api_init_fstream__verify_resource_id_done };
        int err = app_acl_verify_resource_id (&cfg);
        if(err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if(json_object_size(err_info) > 0)
        _api_initiate_file_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
    return 0;
} // end of  api_acl_middleware__init_fstream


// TODO
// * check whether json file exists (users ACL), if not, create one; or if it exists, then still refresh the 
//   content if the last update is before certain time llmit.
// * refresh users ACL from database to local api server (saved in temp buffer)
//   (may improve the flow by sending message queue everytime when user ACL has been updaated)
// * examine user ACL, if it is NOT public, authenticate client JWT, then check the auth user
// has access to the file.


RESTAPI_ENDPOINT_HANDLER(initiate_file_stream, POST, hdlr, req)
{
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *qparams  = app_fetch_from_hashmap(node->data, "qparams");
    uint32_t  last_upld_seq = (uint32_t) json_integer_value(json_object_get(qparams, "last_upld_req"));
    uint32_t  res_owner_id  = (uint32_t) json_integer_value(json_object_get(qparams, "resource_owner_id"));
    const char *label = "hls"; // TODO, store stream types to database once there are more to support
    const char *storage_alias = "localfs";
    atfp_t  *processor = app_transcoder_file_processor(label);
    if(!processor) {
        req->res.status = 500;
        goto done;
    } {
        json_t *qp_labels = json_object(), *update_interval = json_object();
        json_object_set_new(qp_labels, "resource_id", json_string(API_QUERYPARAM_LABEL__RESOURCE_ID));
        // json_object_set_new(qp_labels, "version", json_string(API_QUERYPARAM_LABEL__RESOURCE_VERSION));
        json_object_set_new(qp_labels, "detail", json_string(API_QUERYPARAM_LABEL__DETAIL_ELEMENT));
        json_object_set_new(update_interval, "keyfile",   json_real(APP_UPDATE_INTERVAL_SECS_KEYFILE));
        json_object_set_new(qparams, "host", json_string(req->authority.base));  // h2o_iovec_t, domain name + port
        json_object_set_new(qparams, "query_param_label", qp_labels);
        json_object_set_new(qparams, "update_interval",  update_interval);
    }
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    // TODO, current implementation assumes the app server runs on hardware with  32-bit or 64-bit address mode
    // , if the sserver provides more computing cability e.g. 128-bit address mode, then the  code below has
    // to be adjusted accroding to max number of bits applied to address
    json_object_set_new(qparams, "_http_req",     json_integer((uint64_t)req)); // for backup purpose
    json_object_set_new(qparams, "_http_handler", json_integer((uint64_t)hdlr)); 
    json_object_set_new(qparams, "_middleware_node", json_integer((uint64_t)node));
    json_object_set_new(qparams, "loop", json_integer((uint64_t)req->conn->ctx->loop));
#pragma GCC diagnostic pop
    json_object_set_new(qparams, "db_alias", json_string("db_server_1"));
    json_object_set_new(qparams, "storage_alias", json_string(storage_alias));
    processor->data = (atfp_data_t) {.error=err_info, .spec=qparams, .callback=_api_atfp_init_stream__done_cb,
          .usr_id=res_owner_id, .upld_req_id=last_upld_seq, .storage={.handle=NULL}};
    processor->ops->init(processor);
    if(json_object_size(err_info) > 0) // 4xx or 5xx
        req->res.status = (int) json_integer_value(json_object_get(qparams, "http_resp_code"));
done:
    if(json_object_size(err_info) > 0)
        _api_initiate_file_stream__deinit_primitives (req, hdlr, node, qparams, err_info);
    return 0;
} // end of initiate_file_stream

