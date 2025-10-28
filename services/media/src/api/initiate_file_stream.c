#include "utils.h"
#include "api/setup.h"
#include "api/filefetch_common.h"
#include "storage/cfg_parser.h"
#include "transcoder/file_processor.h"

static void _api_atfp_init_stream__done_cb(atfp_t *processor) {
    json_t *resp_body = NULL;
    json_t *err_info = processor->data.error;
    json_t *spec = processor->data.spec;
    json_t *qparams = spec;

    h2o_req_t             *req = (h2o_req_t *)json_integer_value(json_object_get(spec, "_http_req"));
    h2o_handler_t         *hdlr = (h2o_handler_t *)json_integer_value(json_object_get(spec, "_http_handler"));
    app_middleware_node_t *node =
        (app_middleware_node_t *)json_integer_value(json_object_get(spec, "_middleware_node"));
    if (json_object_size(err_info) == 0) {
        json_decref(err_info);
        resp_body = json_object_get(spec, "return_data");
    } else {
        resp_body = err_info;
    }
    req->res.status = (int)json_integer_value(json_object_get(spec, "http_resp_code"));
    processor->data.error = NULL;
    processor->data.spec = NULL;
    api_init_filefetch__deinit_common(req, hdlr, node, qparams, resp_body);
} // end of _api_atfp_init_stream__done_cb

RESTAPI_ENDPOINT_HANDLER(initiate_file_stream, POST, hdlr, req) {
    json_t     *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t     *qparams = app_fetch_from_hashmap(node->data, "qparams");
    uint32_t    last_upld_seq = (uint32_t)json_integer_value(json_object_get(qparams, "last_upld_req"));
    uint32_t    res_owner_id = (uint32_t)json_integer_value(json_object_get(qparams, "resource_owner_id"));
    const char *label = "hls"; // TODO, store stream types to database once there are more to support
    atfp_t     *processor = app_transcoder_file_processor(label);
    if (!processor) {
        req->res.status = 500;
        goto done;
    }
    {
        json_t *qp_labels = json_object(), *update_interval = json_object();
        json_object_set_new(qp_labels, "resource_id", json_string(API_QPARAM_LABEL__STREAM_DOC_ID));
        json_object_set_new(qp_labels, "detail", json_string(API_QPARAM_LABEL__DOC_DETAIL));
        json_object_set_new(update_interval, "keyfile", json_real(APP_UPDATE_INTERVAL_SECS));
        json_object_set_new(
            qparams, "host", json_string(req->authority.base)
        ); // h2o_iovec_t, domain name + port
        json_object_set_new(qparams, "query_param_label", qp_labels);
        json_object_set_new(qparams, "update_interval", update_interval);
    }
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    // TODO, current implementation assumes the app server runs on hardware with  32-bit or 64-bit
    // address mode , if the sserver provides more computing cability e.g. 128-bit address mode,
    // then the  code below has to be adjusted accroding to max number of bits applied to address
    json_object_set_new(qparams, "_http_req", json_integer((uint64_t)req)); // for backup purpose
    json_object_set_new(qparams, "_http_handler", json_integer((uint64_t)hdlr));
    json_object_set_new(qparams, "_middleware_node", json_integer((uint64_t)node));
    json_object_set_new(qparams, "loop", json_integer((uint64_t)req->conn->ctx->loop));
#pragma GCC diagnostic pop
    json_object_set_new(qparams, "db_alias", json_string("db_server_1"));
    json_object_set_new(qparams, "storage_alias", json_string("persist_usr_asset"));
    processor->data = (atfp_data_t
    ){.error = err_info,
      .spec = qparams,
      .callback = _api_atfp_init_stream__done_cb,
      .usr_id = res_owner_id,
      .upld_req_id = last_upld_seq,
      .storage = {.handle = NULL}};
    processor->ops->init(processor);
    if (json_object_size(err_info) > 0) // 4xx or 5xx
        req->res.status = (int)json_integer_value(json_object_get(qparams, "http_resp_code"));
done:
    if (json_object_size(err_info) > 0)
        api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
    return 0;
} // end of initiate_file_stream
