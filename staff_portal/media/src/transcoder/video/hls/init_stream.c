#include <assert.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <sys/stat.h>
#include <sys/file.h>
#include <openssl/err.h>
#include <openssl/bn.h>

#include "app_cfg.h"
#include "transcoder/video/hls.h"

#define  ASA_SRC_BASEPATH_PATTERN  "%s/%d/%08x/%s"
// TODO, parameterize
#define  NBYTES_KEY        16
#define  NBYTES_IV         16
#define  KEY_ID_SZ         8
#define  MIN__RD_BUF_SZ    512

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static  ASA_RES_CODE atfp_hls__open_src_mst_plist (atfp_hls_t *);

static void _atfp_hls__final_dealloc(atfp_t *processor, uint8_t invoke_usr_cb) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t *asa_src   =  processor ->data.storage.handle;
    asa_op_base_cfg_t *asa_local = &hlsproc->asa_local.super;
    if(invoke_usr_cb)
        processor->data.callback(processor);
    if(asa_src) {
        DEINIT_IF_EXISTS(asa_src->op.scandir.path, free);
        if(asa_src->op.scandir.fileinfo.data) {
            for(int idx = 0; idx < asa_src->op.scandir.fileinfo.size; idx++) {
                asa_dirent_t  *e = &asa_src->op.scandir.fileinfo.data[idx];
                DEINIT_IF_EXISTS(e->name, free);
            }
        }
        DEINIT_IF_EXISTS(asa_src->op.scandir.fileinfo.data, free);
        asa_src->deinit(asa_src);
        processor->data.storage.handle = NULL;
    }
    DEINIT_IF_EXISTS(asa_local->op.mkdir.path.origin, free);
    DEINIT_IF_EXISTS(asa_local->op.open.dst_path, free);
    DEINIT_IF_EXISTS(processor, free);
} // end of _atfp_hls__final_dealloc


#define  INIT_CONSTRUCT_URL_PLAYLIST(_spec) \
    const char *host_domain = NULL, *host_path = NULL,  *res_id_label = NULL, \
         *version_label = NULL,  *detail_label = NULL; \
    { \
        json_t *hostinfo = json_object_get(_spec, "host"); \
        host_domain = json_string_value(json_object_get(hostinfo, "domain")); \
        host_path = json_string_value(json_object_get(hostinfo, "path"));  \
        json_t *qparam_obj = json_object_get(_spec, "query_param_label"); \
        res_id_label  = json_string_value(json_object_get(qparam_obj, "resource_id")); \
        version_label = json_string_value(json_object_get(qparam_obj, "version")); \
        detail_label  = json_string_value(json_object_get(qparam_obj, "detail")); \
    }


static  void atfp_hls__init_stream__finish_cb (atfp_t *processor)
{
    json_t *spec  =  processor->data.spec;
    json_t *return_data = json_object();
#define   STREAM_ENTRY_URL_PATTERN  "https://%s%s?%s=%s&%s=%s"
    const char *resource_id = json_string_value(json_object_get(spec, "id"));
    INIT_CONSTRUCT_URL_PLAYLIST(spec)
    size_t url_sz = sizeof(STREAM_ENTRY_URL_PATTERN) + strlen(host_domain) + strlen(host_path)
          + strlen(res_id_label) + strlen(resource_id) + strlen(detail_label) +
          sizeof(HLS_MASTER_PLAYLIST_FILENAME);
    char  entry_url[url_sz];
    size_t  nwrite = snprintf(&entry_url[0], url_sz, STREAM_ENTRY_URL_PATTERN, host_domain,
                host_path, res_id_label, resource_id, detail_label, HLS_MASTER_PLAYLIST_FILENAME);
    assert(url_sz > nwrite);
    json_object_set_new(return_data, "type",  json_string("hls"));
    json_object_set_new(return_data, "entry", json_string(&entry_url[0]));
    json_object_set_new(spec, "return_data", return_data);
    json_object_set_new(spec, "http_resp_code", json_integer(200));
#undef   STREAM_ENTRY_URL_PATTERN  
    _atfp_hls__final_dealloc(processor, 1);
} // end of  atfp_hls__init_stream__finish_cb


static  void  _atfp_hls__close_crypto_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_t *processor = (atfp_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    json_t *err_info  =  processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE && json_object_size(err_info) == 0) {
        atfp_hls__init_stream__finish_cb (processor);
    } else {
        json_t *spec  =  processor->data.spec;
        if(!json_object_get(spec, "http_resp_code")) {
            json_object_set_new(spec, "http_resp_code", json_integer(500));
            fprintf(stderr, "[hls] line:%d, unknown error on closing crypto keyfile \r\n", __LINE__);
        }
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__close_crypto_keyfile_cb


static  int  _atfp_hls__stream__crypto_key_rotation(json_t *keyinfo,  float update_interval_secs, json_t *err_info)
{
    int updated = 0, ret = 0;
    json_t *item = NULL;
    const char *key_id = NULL;
    char *del_key_id = NULL, *key_hex = NULL, *iv_hex = NULL;
    time_t  most_recent_ts = 0,  earlist_ts = 0, curr_ts = time(NULL);
    json_object_foreach(keyinfo, key_id, item) {
        const char *algo = json_string_value(json_object_get(item, "alg"));
        json_t *keyitem = json_object_get(item, "key");
        int  nbytes = (int) json_integer_value(json_object_get(keyitem, "nbytes"));
        // currently, HLS encryption only accept AES-128-CBC, would refactor code when there's more to support
        // Also note that `nbytes` above means the size of original key bytes, not the hex string
        if(strncmp(algo, "aes", 3) || nbytes != NBYTES_KEY) // not match
            continue;
        // not good implementation, but it is required to store timestamp 
        time_t  _ts = (time_t) json_integer_value(json_object_get(item, "timestamp"));
        if(_ts > most_recent_ts)
            most_recent_ts = _ts;
        if(earlist_ts == 0.f || _ts < earlist_ts) {
            earlist_ts = _ts;
            del_key_id = (char *) key_id;
        }
    } // end of key item iteration
    key_id = NULL;
    if(most_recent_ts > 0) {
        double  num_seconds = difftime(curr_ts, most_recent_ts);
        if(num_seconds < update_interval_secs) {
            fprintf(stderr, "[hls] skip rotatation, interval too short \r\n");
            goto done;
        }
    }
    BIGNUM *_bignum = BN_new();
    ret = BN_rand(_bignum, NBYTES_KEY << 3, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    if(!ret) {
        char buf[256] = {0};
        unsigned long err_code = ERR_get_error();
        ERR_error_string_n(err_code, &buf[0], 256);
        fprintf(stderr, "[hls][openssl] failed to rotate key, reason:%s \r\n", &buf[0]);
        json_object_set_new(err_info, "transcoder", json_string("[hls] rotation failure"));
        goto done;
    }
    key_hex = BN_bn2hex(_bignum);
    BN_rand(_bignum, NBYTES_IV << 3, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    iv_hex = BN_bn2hex(_bignum);
    BN_rand(_bignum, KEY_ID_SZ << (3 - 1), BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    key_id = BN_bn2hex(_bignum);
    BN_free(_bignum);
    if ((strlen(key_hex) == (NBYTES_KEY << 1)) && (strlen(iv_hex) == (NBYTES_IV << 1)) 
            && (strlen(key_id) == KEY_ID_SZ)) { // new key item
        json_t *key_item = json_object();
        json_t *iv_item  = json_object();
        json_object_set_new(key_item, "nbytes", json_integer(NBYTES_KEY));
        json_object_set_new(key_item, "data",  json_string(key_hex));
        json_object_set_new(iv_item, "nbytes", json_integer(NBYTES_IV));
        json_object_set_new(iv_item, "data",   json_string(iv_hex));
        item = json_object();
        json_object_set_new(item, "key", key_item);  
        json_object_set_new(item, "iv",  iv_item);  
        json_object_set_new(item, "alg", json_string("aes"));  
        json_object_set_new(item, "timestamp", json_integer(curr_ts));  
        json_object_deln(keyinfo, key_id, KEY_ID_SZ);
        json_object_set_new(keyinfo, key_id, item);  
    } else {
        fprintf(stderr, "[hls][openssl] error on  generated rand bytes \r\n");
        json_object_set_new(err_info, "transcoder", json_string("[hls] rotation failure"));
        goto done;
    }
    if(del_key_id) {
        double  num_seconds = difftime(curr_ts, earlist_ts);
        double  del_threshold = update_interval_secs * 4;
        if(num_seconds > del_threshold)
            json_object_deln(keyinfo, del_key_id, KEY_ID_SZ);
    }
    updated = 1;
done:
    if(key_id)
        free((char *)key_id);
    if(key_hex)
        free(key_hex);
    if(iv_hex)
        free(iv_hex);
    return updated;
} // end of _atfp_hls__stream__crypto_key_rotation


static  void  _atfp_hls__open_crypto_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *_spec     =  processor->data.spec;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        int fd = hlsproc->asa_local.file.file;
        json_error_t  j_err = {0};   // load entire file, it shouldn't be that large in most cases
        json_t *keyinfo = json_loadfd(fd, JSON_REJECT_DUPLICATES, &j_err);
        if(!keyinfo)
            keyinfo = json_object();
        json_t *update_interval = json_object_get(_spec, "update_interval");
        float  keyfile_update_interval  = json_real_value(json_object_get(update_interval, "keyfile"));
        int updated = _atfp_hls__stream__crypto_key_rotation(keyinfo, keyfile_update_interval, err_info);
        if(updated) {
            ftruncate(fd, (off_t)0);
            lseek(fd, 0, SEEK_SET);
            json_dumpfd((const json_t *)keyinfo, fd, JSON_COMPACT);
        }
        if(json_object_size(err_info) > 0)
            json_object_set_new(_spec, "http_resp_code", json_integer(503));
        json_decref(keyinfo);
        result = _asa_local->storage->ops.fn_close(_asa_local);
    } // succeeded to open the key file
    if(result != ASTORAGE_RESULT_ACCEPT) {
        int ret = json_object_set_new(_spec, "http_resp_code", json_integer(500));
        assert(ret == 0);
        json_object_set_new(err_info, "transcoder", json_string("[hls] failed to update playlist"));
        fprintf(stderr, "[hls] line:%d, failed to open crypto key file \r\n", __LINE__);
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__open_crypto_keyfile_cb


static  void atfp_hls__init_stream__open_crypto_keyfile (atfp_hls_t *hlsproc)
{   // check whether key file exists, if not, create one (json key-value pair)
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    asa_op_base_cfg_t *_asa_local = &hlsproc->asa_local.super;
    size_t  filepath_sz = strlen(_asa_local->op.mkdir.path.origin) + 1 + sizeof(HLS_CRYPTO_KEY_FILENAME) + 1;
    char    filepath[filepath_sz];
    size_t  nwrite = snprintf(&filepath[0], filepath_sz, "%s/%s", _asa_local->op.mkdir.path.origin,
            HLS_CRYPTO_KEY_FILENAME);
    assert(filepath_sz >= nwrite);
    if(_asa_local->op.open.dst_path) {
        free(_asa_local->op.open.dst_path);
        _asa_local->op.open.dst_path = strdup(&filepath[0]);
    }
    _asa_local->op.open.mode  = S_IRUSR | S_IWUSR;
    _asa_local->op.open.flags = O_RDWR | O_CREAT;
    _asa_local->op.open.cb  = _atfp_hls__open_crypto_keyfile_cb;
    _asa_local->op.close.cb = _atfp_hls__close_crypto_keyfile_cb;
    ASA_RES_CODE  result =  _asa_local->storage->ops.fn_open(_asa_local);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[hls] line:%d, failed to open crypto key file \r\n", __LINE__);
        json_object_set_new(spec, "http_resp_code", json_integer(500));
        json_object_set_new(err_info, "transcoder", json_string("[hls] failed to update playlist"));
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  atfp_hls__init_stream__open_crypto_keyfile


static  void  _atfp_hls__close_dst_mst_plist_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    processor->transfer.dst.flags.asalocal_open = 0;
    assert(json_object_size(spec) > 0);
    uint8_t proceeding =  !hlsproc->internal.local_plist_lock_owner || hlsproc->internal.num_plist_merged > 0;
    if(proceeding) {
        atfp_hls__init_stream__open_crypto_keyfile(hlsproc);
    } else {
        fprintf(stderr, "[hls] line:%d,  master playlist not found from source \r\n", __LINE__);
        if(!json_object_get(spec, "http_resp_code")) {
            json_object_set_new(spec, "http_resp_code", json_integer(404));
            json_object_set_new(err_info, "storage", json_string("[hls] source master playlist not found"));
        }
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__close_dst_mst_plist_cb


#define  DEINIT_CLOSE_DST_PLIST(_hlsproc, _result) \
{ \
    uint8_t next_version_exist = _hlsproc->super.transfer.dst.flags.version_exists; \
    asa_op_base_cfg_t *_asa_local = &_hlsproc->asa_local.super; \
    int  dst_fd = _hlsproc->asa_local.file.file; \
    if(next_version_exist) \
        flock(dst_fd, LOCK_UN | LOCK_NB); \
    _asa_local->op.close.cb = _atfp_hls__close_dst_mst_plist_cb; \
    _result = _asa_local->storage->ops.fn_close(_asa_local); \
    if(_result != ASTORAGE_RESULT_ACCEPT) \
        _atfp_hls__final_dealloc(&_hlsproc->super, 1); \
}

#define  DEINIT_CLOSE_SRC_DST_PLIST(_hlsproc, _result) \
{ \
    asa_op_base_cfg_t *_asa_src = _hlsproc->super.data.storage.handle; \
    _asa_src->op.close.cb = _atfp_hls__close_src_mst_plist_cb; \
    _result = _asa_src->storage->ops.fn_close(_asa_src); \
    if(_result != ASTORAGE_RESULT_ACCEPT) \
        DEINIT_CLOSE_DST_PLIST(_hlsproc, _result); \
}

static  void  _atfp_hls__close_src_mst_plist_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = & hlsproc->super;
    json_t *spec  =  processor->data.spec;
    processor ->transfer.dst.flags.asaremote_open = 0;
    result = atfp_hls__open_src_mst_plist(hlsproc);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(spec, "http_resp_code", json_integer(500));
        DEINIT_CLOSE_DST_PLIST(hlsproc, result);
    }
} // end of  _atfp_hls__close_src_mst_plist_cb


static  void  _atfp_hls__write_dst_mst_plist_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result, size_t nwrite)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    if(result == ASTORAGE_RESULT_COMPLETE) {
        _asa_local->op.write.offset += nwrite;
        hlsproc->internal.num_plist_merged++;
    }
    DEINIT_CLOSE_SRC_DST_PLIST(hlsproc, result);
} // end of  _atfp_hls__write_dst_mst_plist_cb


static  void  _atfp_hls__read_src_mst_plist_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread)
{ // read ext-x-stream-inf tag, then write it to collected master playlist
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    // NOTE, this application assumes the read buffer is sufficient to read whole beginning
    // part of ext-x tags, so there is only one read operation to the source playlist
    int entry_idx = asa_src->op.scandir.fileinfo.rd_idx - 1;
    char  *wr_buf = NULL;
    size_t  wr_sz = 0;
    if (result != ASTORAGE_RESULT_COMPLETE)
        goto done;
    char *stream_inf_start = strstr(asa_src->op.read.dst, "\n" "#EXT-X-STREAM-INF");
    if(!stream_inf_start) {
        fprintf(stderr, "[hls] line:%d, invalid content in source master playlist \r\n", __LINE__);
        goto done;
    }
    char *stream_inf_end = strstr(stream_inf_start + 1, "\n"); // skip new-line chars
    if(!stream_inf_end)
        goto done;
    stream_inf_end += 1; // including the new-line chars
    { // construct URL of each media playlist, write it to the end of copied data in read buffer
        const char *resource_id = json_string_value(json_object_get(processor->data.spec, "id"));
        INIT_CONSTRUCT_URL_PLAYLIST(processor->data.spec);
        asa_dirent_t *ver_entry = & asa_src->op.scandir.fileinfo.data[ entry_idx ];
        size_t nb_buf_avail = asa_src->op.read. dst_max_nbytes - ((size_t)stream_inf_end
                - (size_t)asa_src->op.read.dst);
        size_t  nwrite = snprintf(stream_inf_end, nb_buf_avail, "https://%s%s?%s=%s&%s=%s&%s=%s\n",
                host_domain, host_path, res_id_label, resource_id, version_label,
                ver_entry->name, detail_label, HLS_PLAYLIST_FILENAME);
        assert(nb_buf_avail > nwrite);
        stream_inf_end += nwrite; // including the new generaated URL
    }
    wr_buf = (entry_idx == 0) ? asa_src->op.read.dst: stream_inf_start;
    wr_sz  = (size_t)stream_inf_end - (size_t)wr_buf;
done:
    if(wr_buf && wr_sz > 0) {
        if(entry_idx == 0)
            ftruncate(hlsproc->asa_local.file.file, 0);
        asa_op_base_cfg_t *_asa_local = &hlsproc->asa_local.super;
        _asa_local->op.write.src_max_nbytes = wr_sz;
        _asa_local->op.write.src_sz = wr_sz;
        _asa_local->op.write.src = wr_buf;
        _asa_local->op.write.cb = _atfp_hls__write_dst_mst_plist_cb;
        result = _asa_local->storage->ops.fn_write(_asa_local);
    }
    if(result != ASTORAGE_RESULT_ACCEPT)
        DEINIT_CLOSE_SRC_DST_PLIST(hlsproc, result);
} // end of  _atfp_hls__read_src_mst_plist_cb


static  ASA_RES_CODE atfp_hls__read_src_mst_plist (atfp_hls_t *hlsproc)
{
    atfp_t *processor = &hlsproc->super;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_src->op.read.dst_sz = asa_src->op.read.dst_max_nbytes;
    asa_src->op.read.cb = _atfp_hls__read_src_mst_plist_cb;
    ASA_RES_CODE result = asa_src->storage->ops.fn_read(asa_src);
    if(result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(processor->data.error, "storage", json_string(
                    "[storage] failed to read src master playlist"));
    return result;
} // end of atfp_hls__read_src_mst_plist


static  void  _atfp_hls__open_src_mst_plist_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        processor->transfer.dst.flags.asaremote_open = 1;
        result = atfp_hls__read_src_mst_plist(hlsproc);
    } else { // it is possible to have other video quality encoded with non-HLS format
        fprintf(stderr, "[hls] line:%d, error on opening src master playlist \r\n", __LINE__);
        result = atfp_hls__open_src_mst_plist(hlsproc);
    }
    if(result != ASTORAGE_RESULT_ACCEPT) { // close remote src playlist
        json_object_set_new(hlsproc->super.data.spec, "http_resp_code", json_integer(500));
        if(processor->transfer.dst.flags.asaremote_open) {
            DEINIT_CLOSE_SRC_DST_PLIST(hlsproc, result);
        } else {
            DEINIT_CLOSE_DST_PLIST(hlsproc, result);
        }
    }
} // end of  _atfp_hls__open_src_mst_plist_cb


static  ASA_RES_CODE atfp_hls__open_src_mst_plist (atfp_hls_t *hlsproc)
{
    ASA_RES_CODE result;
    atfp_t *processor = &hlsproc->super;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    json_t *err_info  = processor->data.error;
    uint32_t max_num_files = asa_src->op.scandir.fileinfo.size;
    uint32_t curr_rd_idx   = asa_src->op.scandir.fileinfo.rd_idx;
    asa_dirent_t *entry = NULL;
    int idx = 0;
    for(idx = curr_rd_idx; (!entry) && (idx < max_num_files); idx++) {
        asa_dirent_t *e = & asa_src->op.scandir.fileinfo.data[ idx ];
        if(e->type != ASA_DIRENT_DIR)
            continue;
        if(strlen(e->name) != APP_TRANSCODED_VERSION_SIZE)
            continue;
        entry = e;
    } // end of loop
    asa_src->op.scandir.fileinfo.rd_idx = idx;
    processor->transfer.dst.flags.version_exists = entry != NULL;
    if(entry) {
        size_t basepath_sz = strlen(asa_src->op.scandir.path);
        size_t filename_sz   = sizeof(HLS_MASTER_PLAYLIST_FILENAME);
        size_t filepath_sz = basepath_sz + 1 + APP_TRANSCODED_VERSION_SIZE + 1 + filename_sz + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, "%s/%s/%s", asa_src->op.scandir.path,
                entry->name, HLS_MASTER_PLAYLIST_FILENAME);
        assert(filepath_sz >= nwrite);
        asa_src->op.open.dst_path = &filepath[0];
        asa_src->op.open.mode  = S_IRUSR;
        asa_src->op.open.flags = O_RDONLY;
        asa_src->op.open.cb  = _atfp_hls__open_src_mst_plist_cb;
        result = asa_src->storage->ops.fn_open(asa_src);
    } else { // end of video version iteration, no more media playlist
        asa_op_base_cfg_t *_asa_local = &hlsproc->asa_local.super;
        int  dst_fd = hlsproc->asa_local.file.file;
        flock(dst_fd, LOCK_UN | LOCK_NB);
        result = _asa_local->storage->ops.fn_close(_asa_local);
        fprintf(stderr, "[hls] line:%d, end of scendir iteration at source path \r\n", __LINE__);
    }
    if(result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(err_info, "storage", json_string("[storage] failed to open src master playlist"));
    return result;
} // end of  atfp_hls__open_src_mst_plist


static  void atfp_hls__scandir_versions_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    json_t *err_info  = processor->data.error;
    int _http_resp_code = 0;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        int err = atfp_scandir_load_fileinfo(asa_src, err_info);
        if(!err) {
            result = atfp_hls__open_src_mst_plist(hlsproc);
            if(result != ASTORAGE_RESULT_ACCEPT)
                _http_resp_code = 500;
        } else {
            _http_resp_code = 500;
            fprintf(stderr, "[hls] line:%d, error when loading file info from the path:%s \r\n",
                    __LINE__, asa_src->op.scandir.path);
        }
    } else {
        _http_resp_code = 400;
        json_object_set_new(err_info, "storage", json_string("[hls] unknown source path"));
        fprintf(stderr, "[hls] line:%d, error on scandir, versions unknown \r\n", __LINE__);
    }
    if(json_object_size(err_info) > 0) {
        json_object_set_new(hlsproc->super.data.spec, "http_resp_code", json_integer(_http_resp_code));
        DEINIT_CLOSE_DST_PLIST(hlsproc, result);
    }
} // end of  atfp_hls__scandir_versions_cb

#undef   DEINIT_CLOSE_SRC_DST_PLIST
#undef   DEINIT_CLOSE_DST_PLIST


static  void  _atfp_hls__open_dst_mst_plist_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    uint8_t  f_opened = result == ASTORAGE_RESULT_COMPLETE;
    processor->transfer.dst.flags.asalocal_open = f_opened;
    hlsproc->internal.num_plist_merged = 0;
    hlsproc->internal.local_plist_lock_owner = 0;
    if (f_opened) {
        int fd = hlsproc->asa_local.file.file;
        int ret = flock(fd, LOCK_EX | LOCK_NB);
        if(ret == 0) {
            asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
            asa_src->op.scandir.cb =  atfp_hls__scandir_versions_cb;
            result =  asa_src->storage->ops.fn_scandir(asa_src);
            if(result == ASTORAGE_RESULT_ACCEPT) { // pass
                hlsproc->internal.local_plist_lock_owner = 1;
            } else {
                flock(fd, LOCK_UN | LOCK_NB);
            }
        } else { // error check
            if(errno == EWOULDBLOCK) {
                fprintf(stderr, "[hls] line:%d, dst master playlist already locked \r\n", __LINE__);
                result = _asa_local->storage->ops.fn_close(_asa_local);
            } else {
                result = ASTORAGE_RESULT_OS_ERROR;
                fprintf(stderr, "[hls] line:%d, error (%d) when locking dst mst playlist \r\n", __LINE__, errno);
            } // TODO, logging
        }
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("[hls] failed to check video qualities"));
    } else { // local file NOT opened
        json_object_set_new(err_info, "storage", json_string("[hls] unable to update master playlist"));
        fprintf(stderr, "[hls] line:%d, failed to open dst mst playlist \r\n", __LINE__);
    }
    if(json_object_size(err_info) > 0) {
        json_object_set_new(spec, "http_resp_code", json_integer(500));
        if(f_opened)
            result = _asa_local->storage->ops.fn_close(_asa_local);
        if((!f_opened) || (result != ASTORAGE_RESULT_ACCEPT))
            _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__open_dst_mst_plist_cb


static  void  _atfp_hls__ensure_local_basepath_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{ // update master playlist, in case the user add the same video with different resolution
    if (result == ASTORAGE_RESULT_COMPLETE) {
        _asa_local->op.open.mode  = S_IRUSR | S_IWUSR;
        _asa_local->op.open.flags = O_WRONLY | O_CREAT;
        _asa_local->op.open.cb  = _atfp_hls__open_dst_mst_plist_cb;
        _asa_local->op.close.cb = _atfp_hls__close_dst_mst_plist_cb;
        result =  _asa_local->storage->ops.fn_open(_asa_local);
        if(result != ASTORAGE_RESULT_ACCEPT)
            fprintf(stderr, "[hls] line:%d, failed to send open-file cmd \r\n", __LINE__);
    } else {
        fprintf(stderr, "[hls] line:%d, failed to mkdir \r\n", __LINE__);
    }
    if(result != ASTORAGE_RESULT_ACCEPT) {
        atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
        atfp_t *processor = & hlsproc->super;
        json_object_set_new(processor->data.spec,  "http_resp_code", json_integer(500));
        json_object_set_new(processor->data.error, "storage", json_string("[hls]  internal error"));
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__ensure_local_basepath_cb


void   atfp__video_hls__init_stream(atfp_t *processor)
{
    int _http_resp_code = 0;
    ASA_RES_CODE  asa_result;
    json_t *_err_info = processor->data.error;
    json_t *_spec = processor->data.spec;
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    asa_op_base_cfg_t  *asa_local = & hlsproc->asa_local.super;
    uint32_t  _usr_id = processor->data.usr_id;
    uint32_t  _upld_req_id = processor->data.upld_req_id;
    if(!_err_info || !_spec || !asa_src ||  !asa_src->storage || _usr_id==0 || _upld_req_id==0) {
        _http_resp_code = 400;
        json_object_set_new(_err_info, "transcoder", json_string("[hls] missing argument during init stream"));
        goto done;
    } else if(asa_src->op.read.dst_max_nbytes < MIN__RD_BUF_SZ) {
        _http_resp_code = 400;
        json_object_set_new(_err_info, "transcoder", json_string("[hls] insufficient read buffer"));
        goto done;
    }
    INIT_CONSTRUCT_URL_PLAYLIST(_spec);
    void *loop = (void *) json_integer_value(json_object_get(_spec, "loop"));
    json_t *update_interval = json_object_get(_spec, "update_interval");
    float  playlist_update_interval = json_real_value(json_object_get(update_interval, "playlist"));
    float  keyfile_update_interval  = json_real_value(json_object_get(update_interval, "keyfile"));
    if(!loop || !update_interval || playlist_update_interval < 1.0f || keyfile_update_interval < 1.0f ||
            !host_domain || !host_path || !res_id_label || !version_label || !detail_label) {
        _http_resp_code = 400;
        json_object_set_new(_err_info, "transcoder", json_string("[hls] missing arguments in spec for constructing playlist"));
        goto done;
    }
    hlsproc->asa_local.loop = loop;
    app_cfg_t *acfg = app_get_global_cfg();
    { // set up scandir path first, will be referenced several times later
        size_t  scan_path_sz = sizeof(ASA_SRC_BASEPATH_PATTERN) + strlen(asa_src->storage->base_path)
            + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(ATFP__COMMITTED_FOLDER_NAME) + 1;
        char  *scanning_path = calloc(scan_path_sz, sizeof(char));
        size_t  nwrite = snprintf(&scanning_path[0], scan_path_sz, ASA_SRC_BASEPATH_PATTERN,
                asa_src->storage->base_path, _usr_id, _upld_req_id, ATFP__COMMITTED_FOLDER_NAME);
        assert(scan_path_sz >= nwrite);
        asa_src->op.scandir.path =  scanning_path;
    } { // check whether the master playlist has been updated just a short while ago
        size_t filepath_sz = sizeof(ASA_SRC_BASEPATH_PATTERN) + strlen(acfg->tmp_buf.path) +
            USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(HLS_MASTER_PLAYLIST_FILENAME) + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, ASA_SRC_BASEPATH_PATTERN,
                acfg->tmp_buf.path, _usr_id, _upld_req_id, HLS_MASTER_PLAYLIST_FILENAME);
        assert(filepath_sz >= nwrite);
        struct stat  statbuf = {0};
        int ret = stat(&filepath[0], &statbuf); // TODO, invoke stat() asynchronously
        if(!ret) {
            time_t  last_update = statbuf.st_mtime;
            time_t  curr_tm = time(NULL);
            double  num_seconds = difftime(curr_tm, last_update);
            if((playlist_update_interval > num_seconds) || (statbuf.st_size == 0)) {
                _http_resp_code = 429;
                json_object_set_new(_err_info, "transcoder", json_string("[hls] playlist update interval too short"));
                fprintf(stderr, "[hls] line:%d, playlist update interval too short \r\n", __LINE__);
                goto done;
            }
        }
        asa_local->op.open.dst_path = strdup(&filepath[0]);
        // ensure the file path of collected master playlist
        char *last_folder_pos = strrchr(&filepath[0], (int)'/');
        filepath_sz = ((size_t)last_folder_pos - (size_t)&filepath[0]) + 1;
        char *ptr = calloc((filepath_sz << 1), sizeof(char));
        asa_local->op.mkdir.path.prefix = NULL;
        asa_local->op.mkdir.path.origin = ptr;
        asa_local->op.mkdir.path.curr_parent = ptr + filepath_sz;
        strncpy(asa_local->op.mkdir.path.origin, &filepath[0], filepath_sz - 1);
        asa_local->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local->op.mkdir.cb  = _atfp_hls__ensure_local_basepath_cb;
        asa_result =  asa_local->storage->ops.fn_mkdir(asa_local, 1);
    }
    if(asa_result != ASTORAGE_RESULT_ACCEPT) {
        _http_resp_code = 500;
        json_object_set_new(_err_info, "storage", json_string("[hls] unable to send command for updating master playlist"));
        fprintf(stderr, "[hls] line:%d, failed to send mkdir cmd \r\n", __LINE__);
    }
done:
    if(json_object_size(_err_info) > 0) {
        json_object_set_new(_spec, "http_resp_code", json_integer(_http_resp_code));
        _atfp_hls__final_dealloc(processor, 0);
    }
} // end of atfp__video_hls__init_stream
