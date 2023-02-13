#include <sys/file.h>
#include <errno.h>

#include "storage/cfg_parser.h"
#include "transcoder/file_processor.h"

// TODO, implementation for evicting expired cached files

#define  INVOKE_INIT_USR_CALLBACK(asaobj, _result) { \
    asa_cch_usrdata_t  *_usrdata = ((asa_op_localfs_cfg_t *)(asaobj))->file.data; \
    _usrdata->callback.init((asaobj), _result); \
}

#define  INVOKE_DEINIT_USR_CALLBACK(asaobj, _result) { \
    asa_cch_usrdata_t  *_usrdata = ((asa_op_localfs_cfg_t *)(asaobj))->file.data; \
    _usrdata->callback.deinit((asaobj), _result); \
}

#define  INVOKE_PROCEED_USR_CALLBACK(asaobj, _result, _buf, _is_final) { \
    asa_cch_usrdata_t  *_usrdata = ((asa_op_localfs_cfg_t *)(asaobj))->file.data; \
    _usrdata->callback.proceed((asaobj), _result,  _buf, _is_final); \
}

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }


static void  _atfp_cachecommon_localfile_closed_cb(asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    asa_op_localfs_cfg_t  *asa_cch_local = (asa_op_localfs_cfg_t *) _asa_cch_local;
    INVOKE_DEINIT_USR_CALLBACK(_asa_cch_local, result);
    DEINIT_IF_EXISTS(_asa_cch_local->op.mkdir.path.origin, free);
    _asa_cch_local->op.mkdir.path.prefix = NULL;
    _asa_cch_local->op.mkdir.path.curr_parent = NULL;
    _asa_cch_local->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = NULL;
    _asa_cch_local->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = NULL;
    DEINIT_IF_EXISTS(asa_cch_local->file.data, free); 
    DEINIT_IF_EXISTS(_asa_cch_local, free);
}

static void  _atfp_nonstreamcache_srcfile_remote_closed_cb(asa_op_base_cfg_t *_asa_src, ASA_RES_CODE result)
{ DEINIT_IF_EXISTS(_asa_src, free); }

static void  atfp_cachecommon_deinit (asa_op_base_cfg_t *_asa_cch_local)
{
    asa_op_localfs_cfg_t  *asa_cch_local = (asa_op_localfs_cfg_t *) _asa_cch_local;
    atfp_t *processor    = _asa_cch_local->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = _asa_cch_local->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    if(processor) {
        processor->data.error = NULL;
        processor->data.spec = NULL;
        processor->ops->deinit(processor);
    }
    if(_map) {
        asa_op_base_cfg_t  *asa_src_remote = atfp_asa_map_get_source(_map);
        asa_src_remote->op.close.cb = _atfp_nonstreamcache_srcfile_remote_closed_cb;
        ASA_RES_CODE result = asa_src_remote->storage->ops.fn_close(asa_src_remote);
        if(result != ASTORAGE_RESULT_ACCEPT)
            _atfp_nonstreamcache_srcfile_remote_closed_cb(asa_src_remote,result);
        atfp_asa_map_set_source(_map, NULL);
        atfp_asa_map_set_localtmp(_map, NULL);
        atfp_asa_map_deinit(_map);
    }
    int fd =  asa_cch_local->file.file;
    if(fd >= 0) {
        asa_cch_usrdata_t  *usrdata = asa_cch_local->file.data;
        if(usrdata->flags.locked) {
            flock(fd, LOCK_UN | LOCK_NB);
            usrdata->flags.locked = 0;
        }
        _asa_cch_local->op.close.cb = _atfp_cachecommon_localfile_closed_cb;
        ASA_RES_CODE result  = _asa_cch_local->storage->ops.fn_close(_asa_cch_local);
        if(result != ASTORAGE_RESULT_ACCEPT)
            _atfp_cachecommon_localfile_closed_cb(_asa_cch_local, result);
    } else {
        _atfp_cachecommon_localfile_closed_cb(_asa_cch_local, ASTORAGE_RESULT_COMPLETE);
    }
} // end of  atfp_cachecommon_deinit


int  atfp_cache_save_metadata(const char *basepath, const char *mimetype, atfp_data_t *fp_data)
{ // new file to save key ID applied to this video, TODO, async operation
    if(!basepath || !mimetype || !fp_data || !fp_data->spec || fp_data->usr_id == 0
            || fp_data->upld_req_id == 0)
        return  1;
    const char *key_id = json_string_value(json_object_get(fp_data->spec, "crypto_key_id"));
    if(!key_id)
        return  1;
    int skipped = 0;
#define  PATTERN  "%s/%s"
    size_t filepath_sz = sizeof(PATTERN) + strlen(basepath) + sizeof(ATFP_ENCRYPT_METADATA_FILENAME) + 1;
    char filepath[filepath_sz];
    size_t nwrite = snprintf(&filepath[0], filepath_sz, PATTERN, basepath, ATFP_ENCRYPT_METADATA_FILENAME);
    assert(filepath_sz >= nwrite);
#undef   PATTERN
    int ret = access(&filepath[0], F_OK);
    if(ret != 0) { // not exist
        json_t *info = json_object();
        json_object_set_new(info, "mimetype", json_string(mimetype));
        json_object_set_new(info, "key_id", json_string(key_id));
        json_object_set_new(info, "usr_id", json_integer(fp_data->usr_id));
        json_object_set_new(info, "upld_req", json_integer(fp_data->upld_req_id));
        int  fd = open(&filepath[0], O_WRONLY | O_CREAT, S_IWUSR | S_IRUSR);
        json_dumpfd((const json_t *)info, fd, JSON_COMPACT);
        close(fd);
        json_decref(info);
    }
    skipped = ret == 0;
#if  1
    if(skipped)
        fprintf(stderr, "[atfp] line:%d, skip updating key ID to the path:%s \r\n",
                __LINE__, basepath);
#endif
    return skipped;
} // end of  atfp_cache_save_metadata


static void  _atfp_cachecommon_ensure_localfile_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    asa_op_localfs_cfg_t  *asa_cch_local = (asa_op_localfs_cfg_t *) _asa_cch_local;
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        // lock the cache file before writing to it, prevent other concurrent requests
        // from writing the same file
        int fd = asa_cch_local->file.file;
        int ret = flock(fd, LOCK_EX | LOCK_NB);
        if(ret == 0) {
            asa_cch_usrdata_t  *usrdata = asa_cch_local->file.data;
            usrdata->flags.locked = 1;
            ftruncate(fd, 0);
        } else { // error check
            json_object_set_new(err_info, "storage", json_string("internal error"));
            if(errno == EWOULDBLOCK) {
                fprintf(stderr, "[atfp][cache] line:%d, cache file already locked \r\n", __LINE__);
                json_object_set_new(err_info, "_http_resp_code", json_integer(409));
            } else {
                fprintf(stderr, "[atfp][cache] line:%d, error (%d) when locking cache file \r\n", __LINE__, errno);
            }
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to create new cache file \r\n", __LINE__);
    }
    INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
} // end of  _atfp_cachecommon_ensure_localfile_cb


static ASA_RES_CODE _atfp_cachecommon_new_localfile (asa_op_base_cfg_t *_asa_cch_local)
{
    json_t  *spec = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    const char *_resource_path = json_string_value(json_object_get(spec, "doc_basepath"));
    const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
#define  PATTERN  "%s/%s"
    size_t _fullpath_sz = sizeof(PATTERN) + strlen(_resource_path) + strlen(_detail) + 1;
    char _fullpath[_fullpath_sz];
    size_t nwrite = snprintf(&_fullpath[0], _fullpath_sz, PATTERN, _resource_path, _detail);
    assert(_fullpath_sz > nwrite);
#undef   PATTERN
    _asa_cch_local->op.open.dst_path = &_fullpath[0];
    _asa_cch_local->op.open.mode  = S_IWUSR | S_IRUSR;
    _asa_cch_local->op.open.flags = O_WRONLY | O_CREAT;
    _asa_cch_local->op.open.cb = _atfp_cachecommon_ensure_localfile_cb;
    ASA_RES_CODE  result = _asa_cch_local->storage->ops.fn_open(_asa_cch_local);
    _asa_cch_local->op.open.dst_path = NULL;
    return result;
} // end of _atfp_cachecommon_new_localfile


static void  _atfp_cachecommon_ensure_detailpath_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{ // TODO, reuse
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    DEINIT_IF_EXISTS(_asa_cch_local->op.mkdir.path.origin, free);
    _asa_cch_local->op.mkdir.path.prefix = NULL;
    _asa_cch_local->op.mkdir.path.curr_parent = NULL;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        result = _atfp_cachecommon_new_localfile(_asa_cch_local);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("internal error"));
    } else {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to create detial path \r\n", __LINE__);
    }
    if(result != ASTORAGE_RESULT_ACCEPT)
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
} // end of  _atfp_cachecommon_ensure_detailpath_cb


static ASA_RES_CODE _atfp_streamcache_new_detailpath (asa_op_base_cfg_t *_asa_cch_local, const char *_path_end_pos)
{
    json_t  *spec = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    const char *_doc_basepath = json_string_value(json_object_get(spec, "doc_basepath"));
    const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
    size_t _detail_path_sz = ((size_t)_path_end_pos - (size_t)_detail);
    size_t _fullpath_sz = strlen(_doc_basepath) + 2 + strlen(_detail);
    char *ptr = calloc((_fullpath_sz << 1), sizeof(char));
    strncpy(ptr, _detail, _detail_path_sz);
    _asa_cch_local->op.mkdir.path.prefix = (char *)_doc_basepath;
    _asa_cch_local->op.mkdir.path.origin = ptr;
    _asa_cch_local->op.mkdir.path.curr_parent = ptr + _fullpath_sz;
    _asa_cch_local->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
    _asa_cch_local->op.mkdir.cb = _atfp_cachecommon_ensure_detailpath_cb;
    return  _asa_cch_local->storage->ops.fn_mkdir(_asa_cch_local, 1);
} // end of  _atfp_streamcache_new_detailpath


static void  _atfp_cache_processor_setup_ready_cb (atfp_t *processor)
{
    ASA_RES_CODE result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    json_t  *spec = processor->data.spec;
    json_t  *err_info = processor->data.error;
    asa_op_base_cfg_t *_asa_cch_local = (asa_op_base_cfg_t *) json_integer_value(
            json_object_get(spec, "_asa_cache_local"));
    if (json_object_size(err_info) == 0) {
        const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
        const char *_path_end_pos = strrchr(_detail, '/');
        if(_path_end_pos) {
            result = _atfp_streamcache_new_detailpath(_asa_cch_local, _path_end_pos);
        } else {
            result = _atfp_cachecommon_new_localfile(_asa_cch_local);
        }  // check whether all parent folders in the path exist
        if(result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("internal error"));
            fprintf(stderr, "[atfp][cache] line:%d, failed to create new cache entry \r\n", __LINE__);
        }
    }
    if (json_object_size(err_info) > 0)
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
} // end of _atfp_cache_processor_setup_ready_cb


static void  _atfp_streamcache_metadata_close_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    json_t  *spec     = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    size_t  buf_max_sz =  _asa_cch_local->op.write.src_max_nbytes;
    if (result == ASTORAGE_RESULT_COMPLETE && json_object_size(err_info) == 0) {
        asa_op_localfs_cfg_t  *asa_cch_local = (asa_op_localfs_cfg_t *) _asa_cch_local;
        json_t  *_metadata = json_object_get(spec, "metadata");
        const char *label = json_string_value(json_object_get(_metadata, "mimetype"));
        uint32_t  res_owner_id  = json_integer_value(json_object_get(_metadata, "usr_id"));
        uint32_t  last_upld_seq = json_integer_value(json_object_get(_metadata, "upld_req"));
        atfp_t *processor = app_transcoder_file_processor(label);
        _asa_cch_local->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = processor;
        json_object_set_new(spec, "loop", json_integer((uint64_t)asa_cch_local->loop));
        json_object_set_new(spec, "_asa_cache_local", json_integer((uint64_t)_asa_cch_local));
        json_object_set_new(spec, "buf_max_sz", json_integer((size_t)buf_max_sz));
        processor->data = (atfp_data_t) {.error=err_info, .spec=spec, .callback=_atfp_cache_processor_setup_ready_cb,
              .usr_id=res_owner_id, .upld_req_id=last_upld_seq, .storage={.handle=NULL}};
        processor->ops->processing(processor);
        if (json_object_size(err_info) > 0)
            fprintf(stderr, "[atfp][cache] line:%d, failed to setup file processor"
                    ", type:%s \r\n",  __LINE__ , label);
    } else {
        fprintf(stderr, "[atfp][cache] line:%d, failed to close metadata file \r\n", __LINE__);
    }
    if (json_object_size(err_info) > 0)
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
} // end of  _atfp_streamcache_metadata_close_cb


static void  _atfp_streamcache_metadata_open_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    asa_op_localfs_cfg_t  *asa_cch_local = (asa_op_localfs_cfg_t *) _asa_cch_local;
    json_t  *spec     = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        int fd = asa_cch_local->file.file;
        json_t *metadata = json_loadfd(fd, JSON_REJECT_DUPLICATES, NULL);
        if(metadata) {
            json_object_set_new(spec, "metadata", metadata);
        } else {
            json_object_set_new(err_info, "storage", json_string("internal error"));
            json_object_set_new(err_info, "_http_resp_code", json_integer(404));
            fprintf(stderr, "[atfp][cache] line:%d, metadata corrupted \r\n", __LINE__);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to open metadata file \r\n", __LINE__);
    }
    _asa_cch_local->op.close.cb = _atfp_streamcache_metadata_close_cb;
    result  = _asa_cch_local->storage->ops.fn_close(_asa_cch_local);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to close metadata file \r\n", __LINE__);
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
    }
} // end of _atfp_streamcache_metadata_open_cb


/* TODO, implement timeout attribute in cache so existing cache file can be evicted after specified expiry time
 *   int  UPDATE_INTERVAL_SECS = 30 // from metadata file
 *   atfp_data_t  data = {.usr_id=usr_id, .upld_req_id=upld_req_id};
 *   int refresh_req = atfp_check_fileupdate_required(&data, acfg->tmp_buf.path,
 *          detail_filepath, UPDATE_INTERVAL_SECS);
 *   if(refresh_req) {
 *       // start file processor
 *   }
*/
static  void  _atfp_streamcache_existence_check (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    json_t  *spec     = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
    } else { // setup expected path to metadata
#define  PATTERN  "%s/%s"
        const char *_cached_path = json_string_value(json_object_get(spec, "doc_basepath"));
        size_t filepath_sz = sizeof(PATTERN) + strlen(_cached_path) + sizeof(ATFP_ENCRYPT_METADATA_FILENAME);
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, PATTERN, _cached_path,
                  ATFP_ENCRYPT_METADATA_FILENAME);
        assert(filepath_sz >= nwrite);
#undef   PATTERN
        _asa_cch_local->op.open.dst_path = (char *)&filepath[0];
        _asa_cch_local->op.open.mode  = S_IRUSR;
        _asa_cch_local->op.open.flags = O_RDONLY;
        _asa_cch_local->op.open.cb = _atfp_streamcache_metadata_open_cb;
        result  = _asa_cch_local->storage->ops.fn_open(_asa_cch_local);
        _asa_cch_local->op.open.dst_path = NULL;
        if(result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("internal error"));
            fprintf(stderr, "[atfp][cache] line:%d, failed to open metadata file \r\n", __LINE__);
            INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
        }
    }
} // end of  _atfp_streamcache_existence_check


static void  _atfp_nonstreamcache_remotesrc_open_cb(asa_op_base_cfg_t *_asa_src, ASA_RES_CODE result)
{ // TODO, ensure path in local cache folder
    json_t  *err_info = _asa_src->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = _asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *tmp = atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t  *_asa_cch_local = &tmp->super;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        json_t  *spec = _asa_src->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
        const char *_doc_basepath = json_string_value(json_object_get(spec, "doc_basepath"));
        size_t _fullpath_sz = strlen(_doc_basepath) + 1;
        char *ptr = calloc((_fullpath_sz << 1), sizeof(char));
        strncpy(ptr, _doc_basepath, _fullpath_sz - 1);
        _asa_cch_local->op.mkdir.path.prefix = NULL;
        _asa_cch_local->op.mkdir.path.origin = ptr;
        _asa_cch_local->op.mkdir.path.curr_parent = ptr + _fullpath_sz;
        _asa_cch_local->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        _asa_cch_local->op.mkdir.cb = _atfp_cachecommon_ensure_detailpath_cb;
        result = _asa_cch_local->storage->ops.fn_mkdir(_asa_cch_local, 1);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("internal error"));
            fprintf(stderr, "[atfp][cache][storage] line:%d, failed to open file \r\n", __LINE__);
            INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache][storage] line:%d, failed to open file \r\n", __LINE__);
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
    }
} // end of  _atfp_nonstreamcache_remotesrc_open_cb

static  void  _atfp_nonstreamcache_existence_check (
        asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result)
{
    json_t  *spec     = _asa_cch_local->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG];
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
    } else {
        const char *_appstorage_alias = json_string_value(json_object_get(spec, "storage_alias"));
        asa_cfg_t *storage = app_storage_cfg_lookup(_appstorage_alias);
        asa_op_base_cfg_t  *asa_src_remote = (asa_op_base_cfg_t *) app_storage__init_asaobj_helper (
            storage,  _asa_cch_local->cb_args.size , 0, 0);
        atfp_asa_map_t *_map = atfp_asa_map_init(0);
        atfp_asa_map_set_source(_map, asa_src_remote);
        atfp_asa_map_set_localtmp(_map, (asa_op_localfs_cfg_t *)_asa_cch_local);
        _asa_cch_local->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = _map;
        asa_src_remote->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = _map;
        asa_src_remote->cb_args.entries[SPEC_INDEX__IN_ASA_USRARG] = spec;
        asa_src_remote->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG] = err_info;
        asa_src_remote->op.read.dst = _asa_cch_local->op.write.src; // reuse the same buffer
        asa_src_remote->op.read.dst_max_nbytes = _asa_cch_local->op.write.src_max_nbytes;
        if(strncmp(_appstorage_alias, "localfs", sizeof("localfs") - 1) == 0) // TODO
            ((asa_op_localfs_cfg_t *)asa_src_remote)->loop = ((asa_op_localfs_cfg_t *)_asa_cch_local)->loop;
        // ----------
        const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
        const char *_asasrc_path = json_string_value(json_object_get(spec, "asa_src_basepath"));
#define  PATTERN  "%s/%s"
        size_t  fullpath_sz = sizeof(PATTERN) + strlen(_asasrc_path) + strlen(_detail) + 1;
        char  fullpath[fullpath_sz];
        size_t  nwrite = snprintf(&fullpath[0], fullpath_sz, PATTERN, _asasrc_path, _detail);
        assert(nwrite <= fullpath_sz);
#undef  PATTERN
        asa_src_remote->op.open.dst_path = (char *)&fullpath[0];
        asa_src_remote->op.open.mode  = S_IRUSR;
        asa_src_remote->op.open.flags = O_RDONLY;
        asa_src_remote->op.open.cb = _atfp_nonstreamcache_remotesrc_open_cb;
        result  = asa_src_remote->storage->ops.fn_open(asa_src_remote);
        asa_src_remote->op.open.dst_path = NULL;
        if(result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("internal error"));
            fprintf(stderr, "[atfp][cache][storage] line:%d, failed to open file, path:%s \r\n",
                    __LINE__, &fullpath[0]);
            INVOKE_INIT_USR_CALLBACK(_asa_cch_local, result);
        }
    }
} // end of  _atfp_nonstreamcache_existence_check


static asa_op_localfs_cfg_t  * _atfp_cache_init_common (void *loop, json_t *spec, json_t *err_info,
        uint8_t num_cb_args, uint32_t buf_sz, asa_open_cb_t  usr_init_cb, asa_close_cb_t  usr_deinit_cb,
        asa_open_cb_t  nxt_evt_cb)
{
    if(num_cb_args <= ERRINFO_INDEX__IN_ASA_USRARG) {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, insufficient number of callback arguments \r\n", __LINE__);
        return NULL;
    }
    asa_cfg_t *storage = app_storage_cfg_lookup("localfs");
    asa_op_localfs_cfg_t *asa_cached_local = (asa_op_localfs_cfg_t *) app_storage__init_asaobj_helper (
            storage, num_cb_args, buf_sz, 0);
    if(!asa_cached_local) {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        return NULL;
    }
    asa_cch_usrdata_t *usrdata = calloc(1, sizeof(asa_cch_usrdata_t));
    usrdata->callback.init   = usr_init_cb;
    usrdata->callback.deinit = usr_deinit_cb;
    asa_cached_local->file.data = usrdata;
    asa_cached_local->super.cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = NULL;
    asa_cached_local->super.cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = NULL;
    asa_cached_local->super.cb_args.entries[SPEC_INDEX__IN_ASA_USRARG] = spec;
    asa_cached_local->super.cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG] = err_info;
    // share the same buffer, a REST endpoint will NOT write cache file in parallel with
    // reading the same file
    asa_cached_local->super.op.write.src            = asa_cached_local->super.op.read.dst;
    asa_cached_local->super.op.write.src_max_nbytes = asa_cached_local->super.op.read.dst_max_nbytes;
    asa_cached_local->loop = loop;
    const char *_cached_path = json_string_value(json_object_get(spec, "doc_basepath"));
    const char *_detail = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
#define  PATTERN  "%s/%s"
    size_t _fullpath_sz = sizeof(PATTERN) + strlen(_cached_path) + strlen(_detail) + 1;
    char _fullpath[_fullpath_sz];
    size_t nwrite = snprintf(&_fullpath[0], _fullpath_sz, PATTERN, _cached_path, _detail);
    assert(_fullpath_sz > nwrite);
#undef   PATTERN
    asa_cached_local->super.op.open.dst_path = &_fullpath[0];
    asa_cached_local->super.op.open.mode  = S_IRUSR;
    asa_cached_local->super.op.open.flags = O_RDONLY;
    asa_cached_local->super.op.open.cb = nxt_evt_cb;
    ASA_RES_CODE result = asa_cached_local->super.storage->ops.fn_open(&asa_cached_local->super);
    // the storage operation function above should internally copy the path
    asa_cached_local->super.op.open.dst_path = NULL;
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to open file \r\n", __LINE__);
        free(asa_cached_local->file.data);
        free(asa_cached_local);
        asa_cached_local = NULL;
    }
    return asa_cached_local;
} // end of  _atfp_cache_init_common

asa_op_localfs_cfg_t  * atfp_streamcache_init (void *loop, json_t *spec, json_t *err_info, uint8_t num_cb_args,
       uint32_t buf_sz, asa_open_cb_t  usr_init_cb, asa_close_cb_t  usr_deinit_cb)
{
    asa_op_localfs_cfg_t  *out = _atfp_cache_init_common (loop, spec, err_info, num_cb_args,
            buf_sz, usr_init_cb, usr_deinit_cb, _atfp_streamcache_existence_check);
    if(out)
        out->super.deinit = atfp_cachecommon_deinit;
    return  out;
}

asa_op_localfs_cfg_t  * atfp_cache_nonstream_init (void *loop, json_t *spec, json_t *err_info, uint8_t num_cb_args,
       uint32_t buf_sz, asa_open_cb_t  usr_init_cb, asa_close_cb_t  usr_deinit_cb)
{
    asa_op_localfs_cfg_t  *out = _atfp_cache_init_common (loop, spec, err_info, num_cb_args,
            buf_sz, usr_init_cb, usr_deinit_cb, _atfp_nonstreamcache_existence_check);
    if(out)
        out->super.deinit = atfp_cachecommon_deinit;
    return  out;
}


static void  _atfp_read_from_cachedfile_cb (asa_op_base_cfg_t *_asa_cch_local, ASA_RES_CODE result, size_t nread)
{
    h2o_iovec_t  buf = {.base=_asa_cch_local->op.read.dst, .len=nread};
    uint8_t  is_final = nread < _asa_cch_local->op.read.dst_sz;
    if(is_final)
        buf.base[nread] = 0;
    INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, result, &buf, is_final);
} // end of  _atfp_read_from_cachedfile_cb

static void  _atfp_write_to_cachedfile_cb (asa_op_base_cfg_t *_asa_cch_local,  ASA_RES_CODE result, size_t nwrite)
{
    _asa_cch_local->op.write.offset += nwrite;
    atfp_t *processor = _asa_cch_local->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = _asa_cch_local->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    h2o_iovec_t  buf = {.base=_asa_cch_local->op.write.src, .len=nwrite};
    uint8_t  is_final = 1;
    if(processor)
        is_final = processor->transfer.streaming_dst.flags.is_final;
    else if(_map)
        is_final = nwrite < _asa_cch_local->op.write.src_max_nbytes;
    INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, result, &buf, is_final);
} // end of  _atfp_write_to_cachedfile_cb

static void _atfp_s_proceed_cachedata_ready_cb (atfp_t *processor)
{
    json_t  *spec = processor->data.spec;
    json_t  *err_info = processor->data.error;
    asa_op_base_cfg_t *_asa_cch_local = (asa_op_base_cfg_t *) json_integer_value(
            json_object_get(spec, "_asa_cache_local"));
    if (json_object_size(err_info) == 0) {
        uint8_t  is_final = processor->transfer.streaming_dst.flags.is_final;
        const char *src_bytes = processor->transfer.streaming_dst.block.data;
        size_t  src_bytes_sz = processor->transfer.streaming_dst.block.len;
        if(!src_bytes || src_bytes_sz == 0) {
            INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, ASTORAGE_RESULT_COMPLETE, NULL, is_final);
        } else {
            assert(src_bytes_sz < _asa_cch_local->op.write.src_max_nbytes);
            memcpy(_asa_cch_local->op.write.src, src_bytes, sizeof(char) * src_bytes_sz);
            _asa_cch_local->op.write.src[src_bytes_sz] = 0;
            _asa_cch_local->op.write.src_sz  = src_bytes_sz;
            _asa_cch_local->op.write.cb =  _atfp_write_to_cachedfile_cb;
            ASA_RES_CODE  result = _asa_cch_local->storage->ops.fn_write(_asa_cch_local);
            if(result != ASTORAGE_RESULT_ACCEPT) {
                json_object_set_new(err_info, "storage", json_string("internal error"));
                INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, result, NULL, 1);
            }
        }
    } else {
        fprintf(stderr, "[atfp][cache] line:%d, file processor failed to produce "
                "response data \r\n",  __LINE__ );
        INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, ASTORAGE_RESULT_UNKNOWN_ERROR, NULL, 1);
    }
} // end of  _atfp_s_proceed_cachedata_ready_cb

static void _atfp_ns_proceed_cachedata_ready_cb (asa_op_base_cfg_t *_asa_src, ASA_RES_CODE result, size_t nread)
{
    json_t  *err_info = _asa_src->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = _asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *tmp = atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t  *_asa_cch_local = &tmp->super;
    if (result == ASTORAGE_RESULT_COMPLETE) {
#if  1
        assert(nread <= _asa_cch_local->op.write.src_max_nbytes);
        assert(_asa_src->op.read.dst == _asa_cch_local->op.write.src);
        assert(_asa_src->op.read.dst_max_nbytes == _asa_cch_local->op.write.src_max_nbytes);
        if(nread < _asa_src->op.read.dst_max_nbytes)
            _asa_src->op.read.dst[nread] = 0;
        _asa_cch_local->op.write.src_sz  = nread;
        _asa_cch_local->op.write.cb =  _atfp_write_to_cachedfile_cb;
        ASA_RES_CODE  result = _asa_cch_local->storage->ops.fn_write(_asa_cch_local);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            fprintf(stderr, "[atfp][cache] line:%d, failed to issue write opeation to"
                "local file \r\n",  __LINE__ );
            json_object_set_new(err_info, "storage", json_string("internal error"));
            INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, result, NULL, 1);
        }
#else
        h2o_iovec_t  buf = {.base=_asa_src->op.read.dst, .len=nread};
        uint8_t  is_final = nread < (_asa_src->op.read.dst_max_nbytes - 1);
        INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, result, &buf, is_final);
#endif
    } else {
        json_object_set_new(err_info, "storage", json_string("internal error"));
        fprintf(stderr, "[atfp][cache] line:%d, failed to load data block from source file"
                "\r\n",  __LINE__ );
        INVOKE_PROCEED_USR_CALLBACK(_asa_cch_local, ASTORAGE_RESULT_UNKNOWN_ERROR, NULL, 1);
    }
} // end of  _atfp_ns_proceed_cachedata_ready_cb


#define  _CACHE_COMMON__PROCEED_START(process_cond, process_op_code) { \
    json_t  *err_info = _asa_cch_local->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG]; \
    asa_cch_usrdata_t  *usrdata = ((asa_op_localfs_cfg_t *)_asa_cch_local)->file.data; \
    usrdata->callback.proceed = cb_p; \
    if(process_cond) { \
        process_op_code \
    } else { \
        _asa_cch_local->op.read.offset = _asa_cch_local->op.seek.pos; \
        _asa_cch_local->op.read.dst_sz = _asa_cch_local->op.read.dst_max_nbytes; \
        _asa_cch_local->op.read.cb = _atfp_read_from_cachedfile_cb; \
        ASA_RES_CODE _result = _asa_cch_local->storage->ops.fn_read(_asa_cch_local); \
        if(_result != ASTORAGE_RESULT_ACCEPT) \
            json_object_set_new(err_info, "storage", json_string("internal error")); \
    } \
}

void  atfp_streamcache_proceed_datablock (asa_op_base_cfg_t  *_asa_cch_local, asa_cch_proceed_cb_t  cb_p)
{
    atfp_t *processor = _asa_cch_local->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
#define  RUN_CODE  { \
    processor->data.callback = _atfp_s_proceed_cachedata_ready_cb; \
    processor->ops->processing(processor); \
}
    _CACHE_COMMON__PROCEED_START(processor != NULL,RUN_CODE)
#undef   RUN_CODE
}

void  atfp_nonstreamcache_proceed_datablock (asa_op_base_cfg_t  *_asa_cch_local, asa_cch_proceed_cb_t  cb_p)
{
    atfp_asa_map_t *_map = _asa_cch_local->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
#define  RUN_CODE  { \
    asa_op_base_cfg_t *_asa_src = atfp_asa_map_get_source(_map); \
    _asa_src->op.read.offset = _asa_src->op.seek.pos; \
    _asa_src->op.read.dst_sz = _asa_src->op.read.dst_max_nbytes; \
    _asa_src->op.read.cb = _atfp_ns_proceed_cachedata_ready_cb; \
    ASA_RES_CODE result = _asa_src->storage->ops.fn_read(_asa_src); \
    if(result != ASTORAGE_RESULT_ACCEPT) \
        json_object_set_new(err_info, "storage", json_string("internal error")); \
}
    _CACHE_COMMON__PROCEED_START(_map != NULL, RUN_CODE)
#undef   RUN_CODE
}
