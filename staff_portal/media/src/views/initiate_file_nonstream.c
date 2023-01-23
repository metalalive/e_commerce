#include "app_cfg.h"
#include "utils.h"
#include "views.h"
#include "views/filefetch_common.h"

#define  API__DETAIL_MAX_SZ   20

RESTAPI_ENDPOINT_HANDLER(initiate_file_nonstream, GET, hdlr, req)
{ // for fetching non-stream single file e.g. pictures
    int err = 0;
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec  = app_fetch_from_hashmap(node->data, "qparams");
    const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
    const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
    size_t  detail_sz = strlen(_detail),  _res_id_encoded_sz = strlen(_res_id_encoded);
    if(detail_sz > API__DETAIL_MAX_SZ) {
        json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("exceeding limit"));
    } else {
        err = app_verify_printable_string(_detail, detail_sz);
        if(err || strstr(_detail, "../")) // prevent users from switching to parent folder
            json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("invalid characters"));
    }
    if(json_object_size(err_info) == 0) {
        uint32_t  last_upld_seq = (uint32_t) json_integer_value(json_object_get(spec, "last_upld_req"));
        uint32_t  res_owner_id  = (uint32_t) json_integer_value(json_object_get(spec, "resource_owner_id"));
        assert(last_upld_seq != 0);
        assert(res_owner_id != 0);
#define  PATTERN  "%s/%s/%s"
        app_cfg_t *acfg = app_get_global_cfg();
        size_t filepath_sz = sizeof(PATTERN) + strlen(acfg->tmp_buf.path) +
            sizeof(ATFP_CACHED_FILE_FOLDERNAME) + _res_id_encoded_sz + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, PATTERN, acfg->tmp_buf.path,
                ATFP_CACHED_FILE_FOLDERNAME, _res_id_encoded);
        if(filepath[nwrite - 1] == 0xa) // next line
            filepath[--nwrite] = 0;
        assert(filepath_sz >= nwrite); // `doc_basepath` stores file exposed to frontend client
        json_object_set_new(spec, "doc_basepath", json_string(&filepath[0]));
#undef   PATTERN
        err = api_filefetch_start_caching (req, hdlr, node, spec, err_info,
            atfp_cache_nonstream_init, atfp_nonstreamcache_proceed_datablock);
    }
    if(json_object_size(err_info) > 0)
        api_init_filefetch__deinit_common(req, hdlr, node, spec, err_info);
    return 0;
} // end of  initiate_file_nonstream
