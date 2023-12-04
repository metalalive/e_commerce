#include "app_cfg.h"
#include "utils.h"
#include "base64.h"
#include "api/setup.h"
#include "api/filefetch_common.h"

#define   API__DOC_ID_MAX_SZ     50
#define   API__DETAIL_MAX_SZ     200

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
        size_t filepath_sz = sizeof(PATTERN) + strlen(basepath) + sizeof(ATFP_CACHED_FILE_FOLDERNAME)
                + doc_id_sz + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, PATTERN, basepath,
                ATFP_CACHED_FILE_FOLDERNAME, doc_id);
        assert(filepath_sz >= nwrite); // `doc_basepath` stores file exposed to frontend client
        json_object_set_new(spec, "doc_basepath", json_string(&filepath[0]));
#undef   PATTERN
    }
    ret = api_filefetch_start_caching(req, self, node, spec,
            err_info, atfp_streamcache_init, atfp_streamcache_proceed_datablock);
done:
    if(ret != 0) {
        json_t *doc_id_err = json_object_get(err_info, API_QPARAM_LABEL__STREAM_DOC_ID);
        json_t *detail_err = json_object_get(err_info, API_QPARAM_LABEL__DOC_DETAIL);
        req->res.status = (doc_id_err || detail_err) ? 400: 500;
        api_init_filefetch__deinit_common(req, self, node, spec, err_info);
    }
    return 0;
} // end of  fetch_file_streaming_element
