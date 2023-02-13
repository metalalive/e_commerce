#include "app_cfg.h"
#include "utils.h"
#include "storage/cfg_parser.h"
#include "api/setup.h"
#include "api/filefetch_common.h"

#define  API__DETAIL_MAX_SZ   20

static void _init_f_nonstream__cached_doc_basepath (json_t *spec, const char *_res_id_encoded,
        size_t _res_id_encoded_sz)
{
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
}

static void _init_f_nonstream__asa_src_basepath (json_t *spec, const char *_storage_alias, uint8_t req_transcoded)
{
    asa_cfg_t *storage = app_storage_cfg_lookup(_storage_alias); // for remote source
    uint32_t  last_upld_seq = (uint32_t) json_integer_value(json_object_get(spec, "last_upld_req"));
    uint32_t  res_owner_id  = (uint32_t) json_integer_value(json_object_get(spec, "resource_owner_id"));
    assert(last_upld_seq != 0);
    assert(res_owner_id != 0);
    size_t  fullpath_sz = strlen(storage->base_path) + USR_ID_STR_SIZE +
        UPLOAD_INT2HEX_SIZE(last_upld_seq) + 1;
#define  PATTERN_ORIG_FILE   "%s/%d/%08x"
#define  PATTERN_TRANSCODED  PATTERN_ORIG_FILE "/%s"
    if(req_transcoded) {
        fullpath_sz += sizeof(PATTERN_TRANSCODED) + sizeof(ATFP__COMMITTED_FOLDER_NAME);
    } else {
        fullpath_sz += sizeof(PATTERN_ORIG_FILE);
    }
    char  fullpath[fullpath_sz];
    size_t  nwrite = 0;
    if(req_transcoded) {
        nwrite = snprintf(&fullpath[0], fullpath_sz, PATTERN_TRANSCODED, storage->base_path,
            res_owner_id, last_upld_seq, ATFP__COMMITTED_FOLDER_NAME);
    } else {
        nwrite = snprintf(&fullpath[0], fullpath_sz, PATTERN_ORIG_FILE, storage->base_path,
            res_owner_id, last_upld_seq);
    }
    assert(nwrite < fullpath_sz);
    json_object_set_new(spec, "asa_src_basepath", json_string(&fullpath[0]));
#undef  PATTERN_ORIG_FILE
#undef  PATTERN_TRANSCODED
} // end of  _init_f_nonstream__asa_src_basepath


RESTAPI_ENDPOINT_HANDLER(initiate_file_nonstream, GET, hdlr, req)
{ // for fetching non-stream single file e.g. pictures
    int err = 0;
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec  = app_fetch_from_hashmap(node->data, "qparams");
    const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
    const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
    size_t  detail_sz = 0,  _res_id_encoded_sz = strlen(_res_id_encoded);
    if(_detail != NULL) {
        detail_sz = strlen(_detail);
    } else { // TODO, let frontend clients decide which file chunk to fetch
        json_object_set_new(spec, API_QPARAM_LABEL__DOC_DETAIL, json_string("1"));
        detail_sz = 1;
    }
    if(detail_sz > API__DETAIL_MAX_SZ) {
        json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("exceeding limit"));
    } else if(detail_sz > 1) {
        err = app_verify_printable_string(_detail, detail_sz);
        if(err || strstr(_detail, "../")) // prevent users from switching to parent folder
            json_object_set_new(err_info, API_QPARAM_LABEL__DOC_DETAIL, json_string("invalid characters"));
    }
    if(json_object_size(err_info) == 0) {
        const char *_asa_src_remote_alias = "localfs";
        json_object_set_new(spec, "storage_alias", json_string(_asa_src_remote_alias));
        _init_f_nonstream__cached_doc_basepath (spec, _res_id_encoded, _res_id_encoded_sz);
        _init_f_nonstream__asa_src_basepath (spec, _asa_src_remote_alias, _detail != NULL);
        err = api_filefetch_start_caching (req, hdlr, node, spec, err_info,
            atfp_cache_nonstream_init, atfp_nonstreamcache_proceed_datablock);
    } else {
        req->res.status = 400;
    }
    if(json_object_size(err_info) > 0)
        api_init_filefetch__deinit_common(req, hdlr, node, spec, err_info);
    return 0;
} // end of  initiate_file_nonstream
