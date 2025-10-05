#include "base64.h"
#include "api/setup.h"
#include "api/filefetch_common.h"
#include "models/pool.h"
#include "storage/cfg_parser.h"

#define ASA_USRARG_INDEX__AFTP                 ATFP_INDEX__IN_ASA_USRARG
#define ASA_USRARG_INDEX__SPEC                 SPEC_INDEX__IN_ASA_USRARG
#define ASA_USRARG_INDEX__ERROR_INFO           ERRINFO_INDEX__IN_ASA_USRARG
#define ASA_USRARG_INDEX__H2GENER              (ERRINFO_INDEX__IN_ASA_USRARG + 1)
#define ASA_USRARG_INDEX__H2REQ                (ERRINFO_INDEX__IN_ASA_USRARG + 2)
#define ASA_USRARG_INDEX__H2HDLR               (ERRINFO_INDEX__IN_ASA_USRARG + 3)
#define ASA_USRARG_INDEX__MIDDLEWARE           (ERRINFO_INDEX__IN_ASA_USRARG + 4)
#define ASA_USRARG_INDEX__CACHE_PROCEED_FN_PTR (ERRINFO_INDEX__IN_ASA_USRARG + 5)
#define NUM_USRARGS_ASA_LOCAL                  (ASA_USRARG_INDEX__CACHE_PROCEED_FN_PTR + 1)

#define ASA_LOCAL_RD_BUF_SZ 512
#define ASA_LOCAL_WR_BUF_SZ ASA_LOCAL_RD_BUF_SZ

typedef struct {
    h2o_generator_t       super;
    asa_op_localfs_cfg_t *asa_cache;
    size_t                nbytes_transferred;
    uint8_t               is_final : 1;
} api_stream_resp_t;

static void _api_filefetch__errmsg_response(h2o_req_t *req, json_t *err_info) {
    // NOTE, do not send message if error happened in the middle of transfer.
    size_t nrequired = json_dumpb((const json_t *)err_info, NULL, 0, 0) + 1;
    char   body_raw[nrequired];
    size_t nwrite = json_dumpb((const json_t *)err_info, &body_raw[0], nrequired, JSON_COMPACT);
    body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    if (req->res.status == 0)
        req->res.status = 500;
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json")
    );
    h2o_send_inline(req, &body_raw[0], strlen(&body_raw[0]));
}

static void _api_filefetch__deinit_primitives(
    h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node, json_t *spec, json_t *err_info
) {
    json_decref(spec);
    json_decref(err_info);
    app_run_next_middleware(hdlr, req, node);
}

static void _api_common_cachefile_deinit_done(asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result) {
    json_t                *spec = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__SPEC];
    json_t                *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    h2o_req_t             *req = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    h2o_handler_t         *hdlr = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2HDLR];
    app_middleware_node_t *node = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE];
    if (spec == NULL)
        spec = app_fetch_from_hashmap(node->data, "qparams");
    if (err_info == NULL)
        err_info = app_fetch_from_hashmap(node->data, "err_info");
    _api_filefetch__deinit_primitives(req, hdlr, node, spec, err_info);
} // end of  _api_common_cachefile_deinit_done

void api_init_filefetch__deinit_common(
    h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node, json_t *qparams, json_t *res_body
) {
    _api_filefetch__errmsg_response(req, res_body);
    _api_filefetch__deinit_primitives(req, hdlr, node, qparams, res_body);
} // end of  api_init_filefetch__deinit_common

static void _api_filefetch__dispose_resp_generator(void *self) {
    api_stream_resp_t *st_resp = self;
    asa_op_base_cfg_t *_asa_cch_local = &st_resp->asa_cache->super;
    h2o_req_t         *req = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    if (!st_resp->is_final) {
        req->res.status = 500;
        h2o_send(req, NULL, 0, H2O_SEND_STATE_ERROR);
    }
    _asa_cch_local->deinit(_asa_cch_local);
}

static void _api_filefetch__terminate_resp_abruptly(h2o_generator_t *self, h2o_req_t *req) {
    api_stream_resp_t *st_resp = (api_stream_resp_t *)self;
    h2o_mem_release_shared(st_resp);
}

static void _api_cache__proceed_datablock_done(
    asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result, h2o_iovec_t *buf, uint8_t _is_final
) {
    h2o_req_t         *req = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    api_stream_resp_t *st_resp = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2GENER];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t           bufcnt = buf ? 1 : 0;
        h2o_send_state_t send_state = _is_final ? H2O_SEND_STATE_FINAL : H2O_SEND_STATE_IN_PROGRESS;
        h2o_send(req, buf, bufcnt, send_state); // TODO, need to send any headers at the end  ?
        if (buf)
            st_resp->nbytes_transferred += buf->len;
        st_resp->is_final = _is_final;
        if (_is_final)
            h2o_mem_release_shared(st_resp);
    } else {
        fprintf(
            stderr,
            "[api][stream_file_lookup] line:%d, cache file access failure"
            " during response transfer \r\n",
            __LINE__
        );
        h2o_mem_release_shared(st_resp);
    }
}

static void _api_filefetch__cache_proceed_response(h2o_generator_t *self, h2o_req_t *req) {
    // due to the issue https://github.com/h2o/h2o/issues/142
    // , h2o_send() can be sent asynchronously once generator.proceed callback is invoked
    api_stream_resp_t *st_resp = (api_stream_resp_t *)self;
    asa_op_base_cfg_t *_asa_cch_local = &st_resp->asa_cache->super;
    json_t            *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    cache_proceed_fn_t proceed_fn = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__CACHE_PROCEED_FN_PTR];
    proceed_fn(_asa_cch_local, _api_cache__proceed_datablock_done);
    if (json_object_size(err_info) > 0) {
        fprintf(stderr, "[api][filefetch_common] line:%d, failed to proceed \r\n", __LINE__);
        h2o_mem_release_shared(st_resp);
    }
}

static void _api_filefetch__set_cachectrl_header(
    h2o_req_t *req, json_t *spec
) { // TODO, change caching strategy, to cache only popular resources
#define RESOURCE_CACHEABLE        "max-age=" APP_UPDATE_INTERVAL_SECS_STR
#define RESOURCE_NON_CACHEABLE    "private,no-cache"
#define RESOURCE_CACHEABLE_SZ     sizeof(RESOURCE_CACHEABLE) - 1
#define RESOURCE_NON_CACHEABLE_SZ sizeof(RESOURCE_NON_CACHEABLE) - 1
    uint8_t     _cchable_flg = json_boolean_value(json_object_get(spec, "http_cacheable"));
    const char *hdr_value = NULL;
    size_t      hdr_value_sz = 0;
    if (_cchable_flg) {
        hdr_value = RESOURCE_CACHEABLE;
        hdr_value_sz = RESOURCE_CACHEABLE_SZ;
    } else {
        // NOTE: differnt CDN interprets this header differently. For example, Nginx does NOT
        //  cache such response, while other HTTP servers might do it.
        hdr_value = RESOURCE_NON_CACHEABLE;
        hdr_value_sz = RESOURCE_NON_CACHEABLE_SZ;
    }
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CACHE_CONTROL, NULL, hdr_value, hdr_value_sz);
#undef RESOURCE_CACHEABLE
#undef RESOURCE_NON_CACHEABLE
#undef RESOURCE_CACHEABLE_SZ
#undef RESOURCE_NON_CACHEABLE_SZ
} // end of _api_filefetch__set_cachectrl_header

static void _api_filefetch__init_sendfile(asa_op_base_cfg_t *_asa_cch_local) {
    h2o_req_t         *req = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    json_t            *spec = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__SPEC];
    api_stream_resp_t *st_resp =
        h2o_mem_alloc_shared(NULL, sizeof(api_stream_resp_t), _api_filefetch__dispose_resp_generator);
    st_resp->super.proceed = _api_filefetch__cache_proceed_response;
    st_resp->super.stop = _api_filefetch__terminate_resp_abruptly;
    st_resp->asa_cache = (asa_op_localfs_cfg_t *)_asa_cch_local;
    st_resp->nbytes_transferred = 0;
    st_resp->is_final = 0;
    _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2GENER] = st_resp;
    // status code  must be sent in the first frame of http response,
    // TODO, how to handle error in the middle of transmission ?
    req->res.status = 200;
    // req->res.content_length = SIZE_MAX; // the size can not be known in advance, if it hasn't
    // been cached setup header that can be sent in advance
    _api_filefetch__set_cachectrl_header(req, spec);
    h2o_add_header(
        &req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/octet-stream")
    );
    h2o_start_response(req, &st_resp->super);
    _api_filefetch__cache_proceed_response(&st_resp->super, req);
} // end of  _api_filefetch__init_sendfile

static void _api_common_cachefile_init_done(asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result) {
    h2o_req_t *req = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    json_t    *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if (json_object_size(err_info) == 0) {
        _api_filefetch__init_sendfile(_asa_cch_local);
    } else {
        int http_resp_code = json_integer_value(json_object_get(err_info, "_http_resp_code"));
        req->res.status = http_resp_code == 0 ? 500 : http_resp_code;
        _api_filefetch__errmsg_response(req, err_info);
        _asa_cch_local->deinit(_asa_cch_local);
    }
}

#define HTTP_HEADER_NAME_PROXYSERVER "x-proxy-host"

static void _api_filefetch__determine_host_domain(h2o_req_t *req, json_t *spec) {
    h2o_iovec_t *value = NULL;
    for (size_t idx = 0; idx < req->headers.size; idx++) {
        h2o_iovec_t *name = req->headers.entries[idx].name;
        int          ret = strncmp(HTTP_HEADER_NAME_PROXYSERVER, name->base, name->len);
        if (ret == 0) {
            value = &req->headers.entries[idx].value;
            json_object_set_new(spec, "proxy_host_domain", json_stringn(value->base, value->len));
            break;
        }
    }
    value = &req->hostconf->authority.hostport;
    // req->authority.base  might not terminate with NULL char in HTTP/1.x request
    json_object_set_new(spec, "host_domain", json_stringn(value->base, value->len));
    // h2o_iovec_t, domain name + port
}

int api_filefetch_start_caching(
    h2o_req_t *req, h2o_handler_t *hdlr, app_middleware_node_t *node, json_t *spec, json_t *err_info,
    cache_init_fn_t init_fn, cache_proceed_fn_t proceed_fn
) { // look for cached file
    asa_op_localfs_cfg_t *asa_cached_local = init_fn(
        req->conn->ctx->loop, spec, err_info, NUM_USRARGS_ASA_LOCAL, ASA_LOCAL_RD_BUF_SZ,
        _api_common_cachefile_init_done, _api_common_cachefile_deinit_done
    );
    if (asa_cached_local) {
        size_t endpoint_path_sz = req->query_at + 1;
        char   endpoint_path[endpoint_path_sz];
        memcpy(&endpoint_path[0], req->path.base, req->query_at);
        endpoint_path[endpoint_path_sz - 1] = 0x0;
        json_t *qp_labels = json_object();
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2GENER] = NULL;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2REQ] = req;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2HDLR] = hdlr;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE] = node;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__CACHE_PROCEED_FN_PTR] = proceed_fn;
        if (json_object_get(spec, "storage_alias") == NULL)
            json_object_set_new(spec, "storage_alias", json_string("localfs")); // for source storage
        json_object_set_new(spec, "db_alias", json_string("db_server_1"));
        _api_filefetch__determine_host_domain(req, spec);
        json_object_set_new(spec, "host_path", json_string(&endpoint_path[0]));
        json_object_set_new(qp_labels, "doc_id", json_string(API_QPARAM_LABEL__STREAM_DOC_ID));
        json_object_set_new(qp_labels, "detail", json_string(API_QPARAM_LABEL__DOC_DETAIL));
        json_object_set_new(spec, "query_param_label", qp_labels);
    }
    return json_object_size(err_info) > 0;
} // end of api_filefetch_start_caching

#define TOWARD_NEXT_MIDDLEWARE_CODE \
    app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info); \
    app_save_ptr_to_hashmap(node->data, "qparams", (void *)qparams); \
    app_run_next_middleware(hdlr, req, node);

static void _api_abac_pdp__try_match_rule(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *qparams = usr_args[3];
    json_t                *err_info = usr_args[4];
    if (result->flag.error) {
        req->res.status = 503;
    } else if (result->data.size != 1 || !result->data.entries) {
        req->res.status = 403;
        json_object_set_new(err_info, "usr_id", json_string("operation denied"));
    } // a record fetched from database implicitly means read access to the file
    if (req->res.status == 0) {
        TOWARD_NEXT_MIDDLEWARE_CODE
    } else {
        api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
    }
} // end of  _api_abac_pdp__try_match_rule

#define KEEP_RESOURCE_ATTRIBUTES_CODE \
    json_object_set_new(qparams, "http_cacheable", json_boolean(result->flag.acl_visible)); \
    json_object_set_new(qparams, "last_upld_req", json_integer(result->upld_req)); \
    json_object_set_new(qparams, "resource_owner_id", json_integer(result->owner_usr_id));

static void _api_abac_pdp__verify_resource_owner(aacl_result_t *result, void **usr_args) {
    h2o_req_t             *req = usr_args[0];
    h2o_handler_t         *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t                *qparams = usr_args[3];
    json_t                *err_info = usr_args[4];
    int                    _resp_status = api_http_resp_status__verify_resource_id(result, err_info);
    if (json_object_size(err_info) == 0) {
        if (result->flag.acl_exists && result->flag.acl_visible) { // everyone can access
            KEEP_RESOURCE_ATTRIBUTES_CODE
            TOWARD_NEXT_MIDDLEWARE_CODE
        } else { // limited to authorized users, load jwt token
            json_t *jwt_claims = app_auth_httphdr_decode_jwt(req);
            if (jwt_claims) {
                app_save_ptr_to_hashmap(node->data, "auth", (void *)jwt_claims);
                KEEP_RESOURCE_ATTRIBUTES_CODE
                uint32_t curr_usr_id = (uint32_t)json_integer_value(json_object_get(jwt_claims, "profile"));
                if (curr_usr_id == result->owner_usr_id) {
                    TOWARD_NEXT_MIDDLEWARE_CODE
                } else { // further check access rules
                    const char *_res_id_encoded =
                        json_string_value(json_object_get(qparams, "res_id_encoded"));
                    void      *usr_args[5] = {req, hdlr, node, qparams, err_info};
                    aacl_cfg_t cfg = {
                        .usr_args = {.entries = &usr_args[0], .size = 5},
                        .resource_id = (char *)_res_id_encoded,
                        .db_pool = app_db_pool_get_pool("db_server_1"),
                        .loop = req->conn->ctx->loop,
                        .usr_id = curr_usr_id,
                        .callback = _api_abac_pdp__try_match_rule
                    };
                    int err = app_resource_acl_load(&cfg);
                    if (err)
                        api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
                }
            } else {
                req->res.status = 401;
                api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
            }
        }
    } else {
        req->res.status = _resp_status;
        api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
    }
} // end of  _api_abac_pdp__verify_resource_owner
#undef KEEP_RESOURCE_ATTRIBUTES_CODE
#undef TOWARD_NEXT_MIDDLEWARE_CODE

// TODO
// * check whether json file exists (users ACL), if not, create one; or if it exists, then still
// refresh the
//   content if the last update is before certain time llmit.
// * refresh users ACL from database to local api server (saved in temp buffer)
//   (may improve the flow by sending message queue everytime when user ACL has been updaated)
int api_abac_pep__init_filefetch(h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node) {
    json_t *err_info = json_object(), *qparams = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], qparams);
    const char *resource_id = app_resource_id__url_decode(qparams, err_info);
    if (!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t out_len = 0;
        char  *_res_id_encoded =
            (char *)base64_encode((const unsigned char *)resource_id, strlen(resource_id), &out_len);
        json_object_set_new(qparams, "res_id_encoded", json_string(_res_id_encoded));
        free(_res_id_encoded);
        _res_id_encoded = (char *)json_string_value(json_object_get(qparams, "res_id_encoded"));
        void      *usr_args[5] = {req, hdlr, node, qparams, err_info};
        aacl_cfg_t cfg = {
            .usr_args = {.entries = &usr_args[0], .size = 5},
            .resource_id = _res_id_encoded,
            .db_pool = app_db_pool_get_pool("db_server_1"),
            .loop = req->conn->ctx->loop,
            .fetch_acl = 1,
            .callback = _api_abac_pdp__verify_resource_owner
        };
        int err = app_acl_verify_resource_id(&cfg);
        if (err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if (json_object_size(err_info) > 0)
        api_init_filefetch__deinit_common(req, hdlr, node, qparams, err_info);
    return 0;
} // end of  api_abac_pep__init_filefetch
