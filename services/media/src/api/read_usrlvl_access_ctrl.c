#include <search.h>

#include "utils.h"
#include "base64.h"
#include "acl.h"
#include "api/setup.h"
#include "models/pool.h"

static void _api_read_usrlvl_acl__deinit_primitives(
    h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node, json_t *spec, json_t *err_info
) {
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json")
    );
    json_t *resp_body = json_object_size(err_info) > 0 ? err_info : json_object_get(spec, "_http_resp_body");
    size_t  nb_required = json_dumpb(resp_body, NULL, 0, 0);
    if (req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[api][read_usrlvl_acl] line:%d \n", __LINE__);
    }
    if (nb_required > 0) {
        char   body[nb_required + 1];
        size_t nwrite = json_dumpb(resp_body, &body[0], nb_required, JSON_COMPACT);
        body[nwrite++] = 0x0;
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, strlen(&body[0]));
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    char *res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    if (res_id_encoded) {
        free(res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    json_decref(err_info);
    json_decref(spec);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_read_usrlvl_acl__deinit_primitives

static void _api_read_acl__final(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *spec = usr_args[3];
    json_t                *err_info = usr_args[4];
    if (result->flag.error) {
        req->res.status = 503;
    } else { // TODO, pagination, to avoid from loading large dataset
        req->res.status = 200;
        json_t *outputs = json_array();
        for (int idx = 0; idx < result->data.size; idx++) {
            aacl_data_t *d = &result->data.entries[idx];
            json_t      *item = json_object();
            json_object_set_new(item, "usr_id", json_integer(d->usr_id));
            json_object_set_new(item, "edit_acl", json_boolean(d->capability.edit_acl));
            json_object_set_new(item, "transcode", json_boolean(d->capability.transcode));
            json_array_append_new(outputs, item);
        } // end of loop
        json_object_set_new(err_info, "size", json_integer(result->data.size));
        json_object_set_new(err_info, "data", outputs);
    }
    _api_read_usrlvl_acl__deinit_primitives(req, hdlr, node, spec, err_info);
} // end of  _api_read_acl__final

static void _api_read_acl__verify_resource_exists(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *spec = usr_args[3];
    json_t                *err_info = usr_args[4];
    req->res.status = api_http_resp_status__verify_resource_id(result, err_info);
    if (json_object_size(err_info) == 0) {
        char      *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
        void      *usr_args[5] = {req, hdlr, node, spec, err_info};
        aacl_cfg_t cfg = {
            .usr_args = {.entries = &usr_args[0], .size = 5},
            .resource_id = _res_id_encoded,
            .db_pool = app_db_pool_get_pool("db_server_1"),
            .loop = req->conn->ctx->loop,
            .callback = _api_read_acl__final
        };
        int err = app_resource_acl_load(&cfg); // seen as Policy Information Point in ABAC
        if (err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if (json_object_size(err_info) > 0)
        _api_read_usrlvl_acl__deinit_primitives(req, hdlr, node, spec, err_info);
} // end of  _api_read_acl__verify_resource_exists

RESTAPI_ENDPOINT_HANDLER(read_usrlvl_acl, GET, hdlr, req) {
    json_t *err_info = json_object(), *spec = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], spec);
    const char *resource_id = app_resource_id__url_decode(spec, err_info);
    if (!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t         out_len = 0;
        unsigned char *_res_id_encoded =
            base64_encode((const unsigned char *)resource_id, strlen(resource_id), &out_len);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)_res_id_encoded);
        void      *usr_args[5] = {req, hdlr, node, spec, err_info};
        aacl_cfg_t cfg = {
            .usr_args = {.entries = &usr_args[0], .size = 5},
            .resource_id = (char *)_res_id_encoded,
            .db_pool = app_db_pool_get_pool("db_server_1"),
            .loop = req->conn->ctx->loop,
            .callback = _api_read_acl__verify_resource_exists
        };
        int err = app_acl_verify_resource_id(&cfg);
        if (err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if (json_object_size(err_info) > 0)
        _api_read_usrlvl_acl__deinit_primitives(req, hdlr, node, spec, err_info);
    return 0;
} // end of  read_usrlvl_acl
