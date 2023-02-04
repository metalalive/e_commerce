#include <search.h>

#include "utils.h"
#include "base64.h"
#include "acl.h"
#include "api/setup.h"
#include "models/pool.h"

static void  _api_edit_filelvl_acl__deinit_primitives ( h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info )
{
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    json_t *resp_body =  json_object_size(err_info) > 0 ?  err_info: json_object_get(spec, "_http_resp_body");
    size_t  nb_required = json_dumpb(resp_body, NULL, 0, 0);
    if(req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[api][edit_filelvl_acl] line:%d \n", __LINE__ );
    }
    if(nb_required > 0) {
        char  body[nb_required + 1];
        size_t  nwrite = json_dumpb(resp_body, &body[0], nb_required, JSON_COMPACT);
        body[nwrite++] = 0x0;
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, strlen(&body[0]));
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    if(_res_id_encoded) {
        free(_res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    json_decref(err_info);
    json_decref(spec);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_edit_filelvl_acl__deinit_primitives


static void _api_abac_pdp__verify_resource_owner (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *spec     = usr_args[3];
    json_t *err_info = usr_args[4];
    req->res.status = api_http_resp_status__verify_resource_id (result, err_info);
    if(json_object_size(err_info) == 0) {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
        if(curr_usr_id == result->owner_usr_id) {
            if(result->flag.acl_exists) {
                json_t *prev_saved_acl = json_object();
                json_object_set_new(prev_saved_acl, "visible", json_boolean(result->flag.acl_visible));
                json_object_set_new(spec, "_saved_acl", prev_saved_acl);
            }
            app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
            app_save_ptr_to_hashmap(node->data, "spec", (void *)spec);
            app_run_next_middleware(hdlr, req, node);
        } else {
            req->res.status = 403;
            _api_edit_filelvl_acl__deinit_primitives (req, hdlr, node, spec, err_info);
        } // currently file-level access control can be modified only by the resource owner
    } else {
        _api_edit_filelvl_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    }
} // end of  _api_abac_pdp__verify_resource_owner


static int api_abac_pep__edit_acl (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node)
{ // this middleware can be seen as Policy Enforcement Point (PEP) of an ABAC implementation
    json_t *err_info = json_object(), *spec = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], spec);
    const char *resource_id = app_resource_id__url_decode(spec, err_info);
    if(!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t out_len = 0;
        unsigned char *_res_id_encoded = base64_encode((const unsigned char *)resource_id,
                 strlen(resource_id), &out_len);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)_res_id_encoded);
        void *usr_args[5] = {req, hdlr, node, spec, err_info};
        aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=5}, .resource_id=(char *)_res_id_encoded,
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                .fetch_acl=1, .callback=_api_abac_pdp__verify_resource_owner };
        int err = app_acl_verify_resource_id (&cfg);
        if(err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if(json_object_size(err_info) > 0)
        _api_edit_filelvl_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    return 0;
} // end of  api_abac_pep__edit_acl


static void  _api_save_acl__done_cb (aacl_result_t *result, void **usr_args)
{
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec = app_fetch_from_hashmap(node->data, "spec");
    if(result->flag.error || !result->flag.write_ok) {
        h2o_error_printf("[api][edit_flvl_acl] line:%d, error on saving ACL context \n", __LINE__);
    } else {
        req->res.status = 200;
    }
    _api_edit_filelvl_acl__deinit_primitives (req, hdlr, node, spec, err_info);
}


RESTAPI_ENDPOINT_HANDLER(edit_filelvl_acl, PATCH, hdlr, req)
{
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec = app_fetch_from_hashmap(node->data, "spec");
    json_t *req_body = json_loadb(req->entity.base, req->entity.len, 0, NULL);
    if(req_body && json_object_size(req_body) > 0) {
        char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
        json_t *prev_saved_acl = json_object_get(spec, "_saved_acl");
        void *usr_args[3] = {req,hdlr,node};
        aacl_cfg_t  aclcfg = {.usr_args={.entries=&usr_args[0], .size=3}, .db_pool=app_db_pool_get_pool("db_server_1"),
            .resource_id=_res_id_encoded, .loop=req->conn->ctx->loop, .callback=_api_save_acl__done_cb };
        int err = app_filelvl_acl_save(&aclcfg, prev_saved_acl, req_body);
        if(err) {
            if(err == 1) // skipped
                req->res.status = 200;
            json_object_set_new(err_info, "body", json_string("change not made"));
        }
    } else {
        json_object_set_new(err_info, "body", json_string("decode error"));
    }
    if(req_body)
        json_decref(req_body);
    if(json_object_size(err_info) > 0)
        _api_edit_filelvl_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    return 0;
} // end of edit_filelvl_acl
