#include "app_cfg.h"
#include "utils.h"
#include "base64.h"
#include "views.h"
#include "transcoder/file_processor.h"

#define   API__DOC_ID_MAX_SZ     50
#define   API__DETAIL_MAX_SZ     200

#define   ASA_USRARG_INDEX__AFTP      ATFP_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__SPEC         SPEC_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__ERROR_INFO   ERRINFO_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__H2GENER     (ERRINFO_INDEX__IN_ASA_USRARG + 1)
#define   ASA_USRARG_INDEX__H2REQ       (ERRINFO_INDEX__IN_ASA_USRARG + 2)
#define   ASA_USRARG_INDEX__H2HDLR      (ERRINFO_INDEX__IN_ASA_USRARG + 3)
#define   ASA_USRARG_INDEX__MIDDLEWARE  (ERRINFO_INDEX__IN_ASA_USRARG + 4)
#define   NUM_USRARGS_ASA_LOCAL      (ASA_USRARG_INDEX__MIDDLEWARE + 1)

#define   ASA_LOCAL_RD_BUF_SZ        512
#define   ASA_LOCAL_WR_BUF_SZ        ASA_LOCAL_RD_BUF_SZ

typedef struct {
    h2o_generator_t        super;
    asa_op_localfs_cfg_t  *asa_cache;
    size_t    nbytes_transferred;
    uint8_t   is_final:1;
} api_stream_resp_t;

static void  _api_find_stream_elm__errmsg_response (h2o_req_t *req, json_t *err_info) {
    // NOTE, do not send message if error happened in the middle of transfer.
    size_t  nrequired = json_dumpb((const json_t *)err_info, NULL, 0, 0) + 1;
    char    body_raw[nrequired] ;
    size_t  nwrite = json_dumpb((const json_t *)err_info, &body_raw[0], nrequired, JSON_COMPACT);
    body_raw[nwrite++] = 0;
    assert(nwrite <= nrequired);
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    h2o_send_inline(req, &body_raw[0], strlen(&body_raw[0]));
}

static void  _api_find_stream_elm__deinit_primitives (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info)
{
    json_decref(spec);
    json_decref(err_info);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_find_stream_elm__deinit_primitives

static void  _api_stream_elm_cachefile_deinit_done_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    json_t  *spec     = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__SPEC];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    h2o_req_t      *req  = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    h2o_handler_t  *hdlr = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2HDLR];
    app_middleware_node_t *node = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE];
    _api_find_stream_elm__deinit_primitives (req, hdlr, node, spec, err_info);
} // end of  _api_stream_elm_cachefile_deinit_done_cb


static  void _api_dispose_stream_resp_generator(void *self)
{
    api_stream_resp_t  *st_resp = self;
    asa_op_base_cfg_t *_asa_cch_local = &st_resp->asa_cache->super;
    h2o_req_t  *req  = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    if(!st_resp->is_final) {
        req->res.status = 500;
        h2o_send(req, NULL, 0, H2O_SEND_STATE_ERROR);
    }
    _asa_cch_local->deinit(_asa_cch_local);
}

static  __attribute__((optimize("O0")))  void  _api_response_terminate_abruptly(h2o_generator_t *self, h2o_req_t *req)
{
    api_stream_resp_t *st_resp = (api_stream_resp_t *) self;
    h2o_mem_release_shared(st_resp);
} // end of  _api_response_terminate_abruptly

static void  _api_send_response_datablock (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result,
        h2o_iovec_t  *buf, uint8_t  _is_final)
{
    h2o_req_t  *req  = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    api_stream_resp_t  *st_resp = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2GENER];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        size_t bufcnt = buf? 1: 0;
        h2o_send_state_t  send_state = _is_final ? H2O_SEND_STATE_FINAL: H2O_SEND_STATE_IN_PROGRESS;
        h2o_send(req, buf, bufcnt, send_state); // TODO, need to send any headers at the end  ?
        if(buf)
            st_resp->nbytes_transferred += buf->len;
        st_resp->is_final = _is_final;
        if(_is_final)
            h2o_mem_release_shared(st_resp);
    } else {
        fprintf(stderr, "[api][stream_file_lookup] line:%d, cache file access failure"
                " during response transfer \r\n", __LINE__);
        h2o_mem_release_shared(st_resp);
    }
} // end of  _api_send_response_datablock


static  __attribute__((optimize("O0")))  void  _api_response_proceeding (h2o_generator_t *self, h2o_req_t *req)
{
    // due to the issue https://github.com/h2o/h2o/issues/142
    // , h2o_send() can be sent asynchronously once generator.proceed callback is invoked
    api_stream_resp_t *st_resp = (api_stream_resp_t *) self;
    asa_op_base_cfg_t *_asa_cch_local = & st_resp->asa_cache->super;
    json_t  *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    atfp_streamcache_proceed_datablock (_asa_cch_local, _api_send_response_datablock);
    if(json_object_size(err_info) > 0) {
        fprintf(stderr, "[api][stream_file_lookup] line:%d, failed to proceed \r\n", __LINE__);
        h2o_mem_release_shared(st_resp);
    }
} // end of _api_response_proceeding


static void  _api_stream_elm_file__start_response(asa_op_base_cfg_t *_asa_cch_local)
{
    h2o_req_t  *req  = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    api_stream_resp_t  *st_resp = h2o_mem_alloc_shared(NULL, sizeof(api_stream_resp_t),
            _api_dispose_stream_resp_generator);
    st_resp->super.proceed = _api_response_proceeding;
    st_resp->super.stop = _api_response_terminate_abruptly;
    st_resp->asa_cache = (asa_op_localfs_cfg_t *) _asa_cch_local;
    st_resp->nbytes_transferred = 0;
    st_resp->is_final = 0;
    _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2GENER] = st_resp;
    // status code  must be sent in the first frame of http response,
    // TODO, error handling if it happenes in the middle of transmission ?
    req->res.status = 200;
    // req->res.content_length = SIZE_MAX; // the size can not be known in advance, if it hasn't been cached
    // setup header that can be sent in advance
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/octet-stream"));    
    h2o_start_response(req, &st_resp->super);
    _api_response_proceeding (&st_resp->super, req);
} // end of  _api_stream_elm_file__start_response



static void  _api_stream_elm_cachefile_init_done_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    h2o_req_t  *req  = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__H2REQ];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if (json_object_size(err_info) == 0) {
        _api_stream_elm_file__start_response(_asa_cch_local);
    } else {
        int http_resp_code = json_integer_value(json_object_get(err_info, "_http_resp_code"));
        req->res.status = http_resp_code == 0 ? 500: http_resp_code;
        _api_find_stream_elm__errmsg_response (req, err_info);
        _asa_cch_local->deinit(_asa_cch_local);
    }
} // end of  _api_stream_elm_cachefile_init_done_cb


static void  _api_try_open_stream_element_file (h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info)
{ // look for cached file
    asa_op_localfs_cfg_t  *asa_cached_local = atfp_streamcache_init (req->conn->ctx->loop, spec, err_info,
            NUM_USRARGS_ASA_LOCAL,  ASA_LOCAL_RD_BUF_SZ,  _api_stream_elm_cachefile_init_done_cb, 
            _api_stream_elm_cachefile_deinit_done_cb);
    if(asa_cached_local) {
        size_t endpoint_path_sz = req->query_at + 1;
        char endpoint_path[endpoint_path_sz] ;
        memcpy(&endpoint_path[0], req->path.base, req->query_at);
        endpoint_path[endpoint_path_sz - 1] = 0x0;
        json_t *qp_labels = json_object();
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2GENER] = NULL;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2REQ] = req;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__H2HDLR] = hdlr;
        asa_cached_local->super.cb_args.entries[ASA_USRARG_INDEX__MIDDLEWARE] = node;
        json_object_set_new(spec, "db_alias", json_string("db_server_1"));
        json_object_set_new(spec, "storage_alias", json_string("localfs")); // for source storage
        json_object_set_new(spec, "host_domain", json_string(req->authority.base));  // h2o_iovec_t, domain name + port
        json_object_set_new(spec, "host_path", json_string( &endpoint_path[0] ));
        json_object_set_new(qp_labels, "doc_id", json_string(API_QPARAM_LABEL__STREAM_DOC_ID));
        json_object_set_new(qp_labels, "detail", json_string(API_QPARAM_LABEL__DOC_DETAIL));
        json_object_set_new(spec, "query_param_label", qp_labels);
    }
} // end of _api_try_open_stream_element_file


RESTAPI_ENDPOINT_HANDLER(fetch_file_streaming_element, GET, self, req)
{
    int  ret = 0;
    json_t *err_info = json_object();
    json_t *spec = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], spec);
    const char *doc_id = json_string_value(json_object_get(spec, API_QPARAM_LABEL__STREAM_DOC_ID));
    const char *detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
    if(!doc_id)
        json_object_set_new(err_info, API_QPARAM_LABEL__STREAM_DOC_ID, json_string("not exist"));
    if(!detail)
        json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("not exist"));
    if(json_object_size(err_info) > 0)
        goto done;
    size_t  doc_id_sz = strlen(doc_id),  detail_sz = strlen(detail);
    if(doc_id_sz > API__DOC_ID_MAX_SZ)
        json_object_set_new(err_info, API_QPARAM_LABEL__STREAM_DOC_ID, json_string("exceeding limit"));
    if(detail_sz > API__DETAIL_MAX_SZ)
        json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("exceeding limit"));
    if(json_object_size(err_info) > 0)
        goto done;
    ret = is_base64_encoded((const unsigned char *)doc_id,  doc_id_sz);
    if(!ret)
        json_object_set_new(err_info, API_QPARAM_LABEL__STREAM_DOC_ID, json_string("contains invalid character"));
    ret = app_verify_printable_string(detail, detail_sz);
    if(ret || strstr(detail, "../")) // prevent users from switching to parent folder
        json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("contains invalid character"));
    if(json_object_size(err_info) > 0)
        goto done;
    app_cfg_t *acfg = app_get_global_cfg();
    const char *basepath = acfg->tmp_buf.path;
    { // setup expected path to cached file
#define  PATTERN  "%s/%s/%s"
        size_t filepath_sz = sizeof(PATTERN) + strlen(basepath) + sizeof(ATFP_ENCRYPTED_FILE_FOLDERNAME)
                + doc_id_sz + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, PATTERN, basepath, ATFP_ENCRYPTED_FILE_FOLDERNAME, doc_id);
        assert(filepath_sz >= nwrite); // `doc_basepath` stores file exposed to frontend client
        json_object_set_new(spec, "doc_basepath", json_string(&filepath[0]));
#undef   PATTERN
    }
    _api_try_open_stream_element_file(req, self, node, spec, err_info);
done:
    if(json_object_size(err_info) > 0) {
        json_t *doc_id_err = json_object_get(err_info, API_QPARAM_LABEL__STREAM_DOC_ID);
        json_t *detail_err = json_object_get(err_info, API_QPARAM_LABEL__DOC_DETAIL);
        req->res.status = (doc_id_err || detail_err) ? 400: 500;
        _api_find_stream_elm__errmsg_response (req, err_info);
        _api_find_stream_elm__deinit_primitives(req, self, node, spec, err_info);
    }
    return 0;
} // end of  fetch_file_streaming_element
