#include <assert.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <ctype.h>
#include <sys/stat.h>
#include <sys/file.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/evp.h>

#include "app_cfg.h"
#include "views.h"
#include "transcoder/video/hls.h"

// TODO, parameterize
#define  MIN__RD_BUF_SZ    512

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static  ASA_RES_CODE atfp_hls__open_src_mst_plist (atfp_hls_t *);

static void _atfp_hls__final_dealloc(atfp_t *processor, uint8_t invoke_usr_cb) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t *asa_local = &hlsproc->asa_local.super;
    if(invoke_usr_cb)
        processor->data.callback(processor);
    DEINIT_IF_EXISTS(asa_local->op.mkdir.path.origin, free);
    DEINIT_IF_EXISTS(asa_local->op.open.dst_path, free);
    DEINIT_IF_EXISTS(processor, free);
} // end of _atfp_hls__final_dealloc


#define  INIT_CONSTRUCT_URL_PLAYLIST(_spec) \
    const char *host_domain = NULL, *res_id_label = NULL,  *detail_label = NULL; \
    { \
        host_domain = json_string_value(json_object_get(_spec, "host")); \
        json_t *qparam_obj = json_object_get(_spec, "query_param_label"); \
        res_id_label  = json_string_value(json_object_get(qparam_obj, "resource_id")); \
        detail_label  = json_string_value(json_object_get(qparam_obj, "detail")); \
    }


static  void atfp_hls__init_stream__finish_cb (atfp_t *processor)
{
    json_t *spec  =  processor->data.spec;
    json_t *return_data = json_object();
    INIT_CONSTRUCT_URL_PLAYLIST(spec)
    const char *_encrypted_doc_id = json_string_value(json_object_get(spec, "encrypted_doc_id"));
    json_object_set_new(return_data, "type",  json_string("hls"));
    json_object_set_new(return_data, "host",  json_string(host_domain));
    json_object_set_new(return_data, res_id_label, json_string(_encrypted_doc_id));
    json_object_set_new(return_data, detail_label,  json_string(HLS_MASTER_PLAYLIST_FILENAME));
    json_object_set_new(spec, "return_data", return_data);
    json_object_set_new(spec, "http_resp_code", json_integer(200));
    _atfp_hls__final_dealloc(processor, 1);
} // end of  atfp_hls__init_stream__finish_cb


static  void  _atfp_hls__ensure_encrypted_basepath_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{ // update master playlist, in case the user add the same video with different resolution
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *_spec = processor->data.spec;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_cache_save_metadata(_asa_local->op.mkdir.path.origin, "hls", &processor->data);
        atfp_hls__init_stream__finish_cb (processor);
    } else {
        json_object_set_new(err_info, "storage", json_string("[hls] failed to init stream"));
        if(!json_object_get(_spec, "http_resp_code"))
            json_object_set_new(_spec, "http_resp_code", json_integer(500));
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__ensure_encrypted_basepath_cb


static  void  _atfp_hls__close_crypto_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = &hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    app_cfg_t *acfg = app_get_global_cfg();
    if (result == ASTORAGE_RESULT_COMPLETE && json_object_size(err_info) == 0) {
        const char *_enc_doc_id = json_string_value(json_object_get(spec, "encrypted_doc_id"));
        size_t  doc_id_sz = strlen(_enc_doc_id);
        size_t  max_path_sz = strlen(acfg->tmp_buf.path) + 3 + doc_id_sz + sizeof(ATFP_ENCRYPTED_FILE_FOLDERNAME);
        char    path[max_path_sz];
        size_t  path_sz = atfp_get_encrypted_file_basepath(acfg->tmp_buf.path, &path[0],
                max_path_sz, _enc_doc_id, doc_id_sz);
        if(path_sz == 0) {
            fprintf(stderr, "[hls][init_stream] line:%d, memory error, path_sz:%ld not sufficient \r\n", __LINE__, path_sz);
        } else {
            DEINIT_IF_EXISTS(_asa_local->op.mkdir.path.origin , free);
            char *ptr = calloc((++path_sz << 1), sizeof(char));
            _asa_local->op.mkdir.path.prefix = NULL;
            _asa_local->op.mkdir.path.origin = ptr;
            _asa_local->op.mkdir.path.curr_parent = ptr + path_sz;
            strncpy(_asa_local->op.mkdir.path.origin, &path[0], path_sz - 1);
            _asa_local->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
            _asa_local->op.mkdir.cb  = _atfp_hls__ensure_encrypted_basepath_cb;
            result =  _asa_local->storage->ops.fn_mkdir(_asa_local, 1);
        }
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("[hls] failed to init stream"));
    } else {
        fprintf(stderr, "[hls][init_stream] line:%d, unknown error on closing crypto keyfile \r\n", __LINE__);
    }
    if (json_object_size(err_info) > 0)
        if(!json_object_get(spec, "http_resp_code"))
            json_object_set_new(spec, "http_resp_code", json_integer(500));
    if(result != ASTORAGE_RESULT_ACCEPT)
        _atfp_hls__final_dealloc(processor, 1);
} // end of  _atfp_hls__close_crypto_keyfile_cb


static  int  _atfp_hls__stream__crypto_key_rotation (json_t *keyinfo, json_t *err_info)
{
    int updated = 0, ret = 0, max_num_items = 3;
    json_t *item = NULL;
    const char *key_id = NULL;
    char *del_key_id = NULL, *key_hex = NULL, *iv_hex = NULL;
    time_t  most_recent_ts = 0,  earlist_ts = 0, curr_ts = time(NULL);
    BIGNUM *_bignum = BN_new();
    json_object_foreach(keyinfo, key_id, item) {
        const char *algo = json_string_value(json_object_get(item, "alg"));
        json_t *keyitem = json_object_get(item, "key");
        int  nbytes = (int) json_integer_value(json_object_get(keyitem, "nbytes"));
        // currently, HLS encryption only accept AES-128-CBC, would refactor code when there's more to support
        // Also note that `nbytes` above means the size of original key bytes, not the hex string
        if(strncmp(algo, "aes", 3) || nbytes != HLS__NBYTES_KEY) // not match
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
    ret = BN_rand(_bignum, HLS__NBYTES_KEY << 3, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    if(!ret) {
        char buf[256] = {0};
        unsigned long err_code = ERR_get_error();
        ERR_error_string_n(err_code, &buf[0], 256);
        fprintf(stderr, "[hls][init_stream] line:%d, failed to rotate key, reason:%s \r\n",__LINE__, &buf[0]);
        json_object_set_new(err_info, "transcoder", json_string("[hls] rotation failure"));
        goto done;
    }
    key_hex = BN_bn2hex(_bignum);
    BN_rand(_bignum, HLS__NBYTES_IV << 3, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    iv_hex = BN_bn2hex(_bignum);
    BN_rand(_bignum, HLS__NBYTES_KEY_ID << (3 - 1), BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY);
    key_id = BN_bn2hex(_bignum);
    uint32_t  actual_key_hex_sz = strlen(key_hex), actual_iv_hex_sz = strlen(iv_hex),
              actual_key_id_sz = strlen(key_id);
    if ((actual_key_hex_sz == (HLS__NBYTES_KEY << 1)) && (actual_iv_hex_sz == (HLS__NBYTES_IV << 1)) 
            && (actual_key_id_sz == HLS__NBYTES_KEY_ID)) { // new key item
        json_t *key_item = json_object(),  *iv_item  = json_object();
        json_object_set_new(key_item, "nbytes", json_integer(HLS__NBYTES_KEY));
        json_object_set_new(key_item, "data",  json_stringn(key_hex, actual_key_hex_sz));
        json_object_set_new(iv_item, "nbytes", json_integer(HLS__NBYTES_IV));
        json_object_set_new(iv_item, "data",   json_stringn(iv_hex, actual_iv_hex_sz));
        item = json_object();
        json_object_set_new(item, "key", key_item);
        json_object_set_new(item, "iv",  iv_item);
        json_object_set_new(item, "alg", json_string("aes"));
        json_object_set_new(item, "timestamp", json_integer(curr_ts));
        json_object_deln(keyinfo, key_id, HLS__NBYTES_KEY_ID);
        json_object_setn_new(keyinfo, key_id, HLS__NBYTES_KEY_ID, item);
    } else { // TODO, figure out how error happenes 
        fprintf(stderr, "[hls][init_stream] line:%d, key:sz=%u,data=0x%s, IV:sz=%u,data=0x%s, keyID:sz=%u,data=0x%s \r\n"
                , __LINE__, actual_key_hex_sz, key_hex, actual_iv_hex_sz, iv_hex, actual_key_id_sz, key_id);
        json_object_set_new(err_info, "transcoder", json_string("[hls] rotation failure"));
        goto done;
    }
    if(del_key_id && json_object_size(keyinfo) > max_num_items)
        json_object_deln(keyinfo, del_key_id, HLS__NBYTES_KEY_ID);
    updated = 1;
done:
    BN_free(_bignum);
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
    json_t *keyinfo = NULL, *chosen_keyitem = NULL;
    const char *chosen_key_id = NULL;
    unsigned char  *encrypted_doc_id = NULL;
    size_t  enc_doc_id_sz = 0;
    if (result == ASTORAGE_RESULT_COMPLETE) { // succeeded to open the key file
        int fd = hlsproc->asa_local.file.file , refresh_req = 0;
        json_error_t  j_err = {0};   // load entire file, it shouldn't be that large in most cases
        keyinfo = json_loadfd(fd, JSON_REJECT_DUPLICATES, &j_err);
        if(keyinfo) {
            json_t *update_interval = json_object_get(processor->data.spec, "update_interval");
            float  keyfile_secs  = json_real_value(json_object_get(update_interval, "keyfile"));
            app_cfg_t *acfg = app_get_global_cfg();
            refresh_req = atfp_check_fileupdate_required(&processor->data, acfg->tmp_buf.path,
                    HLS_CRYPTO_KEY_FILENAME, keyfile_secs);
        } else {
            keyinfo = json_object();
            refresh_req = 1;
        }
        if(refresh_req && _atfp_hls__stream__crypto_key_rotation(keyinfo, err_info)) {
            ftruncate(fd, (off_t)0);
            lseek(fd, 0, SEEK_SET);
            json_dumpfd((const json_t *)keyinfo, fd, JSON_COMPACT);
        } else {
            fprintf(stderr, "[hls][init_stream] line:%d, key rotation skipped \r\n", __LINE__);
        }
        if(json_object_size(err_info) > 0)
            goto done;
    } else {
        json_object_set_new(err_info, "transcoder", json_string("[hls] internal error"));
        fprintf(stderr, "[hls][init_stream] line:%d,failed to open crypto key file \r\n", __LINE__);
        goto done;
    }
    chosen_key_id = hlsproc->internal.op.get_crypto_key(keyinfo, ATFP__CRYPTO_KEY_MOST_RECENT, &chosen_keyitem);
    if(!chosen_key_id || !chosen_keyitem) {
        json_object_set_new(err_info, "transcoder", json_string("[hls] internal error"));
        fprintf(stderr, "[hls][init_stream] line:%d, failed to get crypto key \r\n", __LINE__);
        goto done;
    }
    hlsproc->internal.op.encrypt_document_id (&processor->data, chosen_keyitem,
            &encrypted_doc_id, &enc_doc_id_sz);
    if(!encrypted_doc_id || enc_doc_id_sz == 0) {
        json_object_set_new(err_info, "transcoder", json_string("[hls] internal error"));
        fprintf(stderr, "[hls][init_stream] line:%d,failed to encrypt document ID \r\n", __LINE__);
        goto done;
    } // TODO, might read IV so it can be written to playlist later
    json_object_set_new(_spec, "crypto_key_id", json_string(chosen_key_id));
    json_object_set_new(_spec, "encrypted_doc_id", json_string((const char *)encrypted_doc_id));
done:
    if(json_object_size(err_info) > 0)
        json_object_set_new(_spec, "http_resp_code", json_integer(503));
    DEINIT_IF_EXISTS(keyinfo, json_decref);
    DEINIT_IF_EXISTS(encrypted_doc_id, free);
    _asa_local->op.close.cb = _atfp_hls__close_crypto_keyfile_cb;
    result = _asa_local->storage->ops.fn_close(_asa_local);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        int ret = json_object_set_new(_spec, "http_resp_code", json_integer(500));
        assert(ret == 0);
        json_object_set_new(err_info, "storage", json_string("[hls] failed to update playlist"));
        fprintf(stderr, "[hls][init_stream] line:%d, failed to open crypto key file \r\n", __LINE__);
        _atfp_hls__final_dealloc(processor, 1);
    }
} // end of  _atfp_hls__open_crypto_keyfile_cb


static  ASA_RES_CODE  atfp_hls__init_stream__crypto_keyfile (atfp_hls_t *hlsproc)
{   // check whether key file exists, if not, create one (json key-value pair)
    asa_op_base_cfg_t *_asa_local = &hlsproc->asa_local.super;
    size_t  filepath_sz = strlen(_asa_local->op.mkdir.path.origin) + 1 + sizeof(HLS_CRYPTO_KEY_FILENAME) + 1;
    char    filepath[filepath_sz];
    size_t  nwrite = snprintf(&filepath[0], filepath_sz, "%s/%s", _asa_local->op.mkdir.path.origin,
            HLS_CRYPTO_KEY_FILENAME);
    assert(filepath_sz >= nwrite);
    DEINIT_IF_EXISTS(_asa_local->op.open.dst_path , free);
    _asa_local->op.open.dst_path = strdup(&filepath[0]);
    _asa_local->op.open.mode  = S_IRUSR | S_IWUSR;
    _asa_local->op.open.flags = O_RDWR | O_CREAT;
    _asa_local->op.open.cb  = _atfp_hls__open_crypto_keyfile_cb;
    ASA_RES_CODE  result =  _asa_local->storage->ops.fn_open(_asa_local);
    if(result != ASTORAGE_RESULT_ACCEPT)
        fprintf(stderr, "[hls][init_stream] line:%d, failed to open crypto key file \r\n", __LINE__);
    return result;
} // end of  atfp_hls__init_stream__crypto_keyfile


static  void  _atfp_hls__ensure_local_basepath_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{ // update master playlist, in case the user add the same video with different resolution
    int _http_resp_code = 500;
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *_err_info = processor->data.error;
    json_t *_spec = processor->data.spec;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        result = atfp_hls__init_stream__crypto_keyfile(hlsproc);
    } else {
        fprintf(stderr, "[hls][init_stream] line:%d, failed to mkdir \r\n", __LINE__);
    }
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(_spec,  "http_resp_code", json_integer(_http_resp_code));
        json_object_set_new(_err_info, "storage", json_string("[hls]  internal error"));
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
    asa_op_base_cfg_t  *asa_local = & hlsproc->asa_local.super;
    uint32_t  _usr_id = processor->data.usr_id;
    uint32_t  _upld_req_id = processor->data.upld_req_id;
    if(!_err_info || !_spec || _usr_id==0 || _upld_req_id==0) {
        _http_resp_code = 400;
        json_object_set_new(_err_info, "transcoder", json_string("[hls] missing argument during init stream"));
        goto done;
    }
    INIT_CONSTRUCT_URL_PLAYLIST(_spec);
    void *loop = (void *) json_integer_value(json_object_get(_spec, "loop"));
    json_t *update_interval = json_object_get(_spec, "update_interval");
    float  keyfile_update_interval  = json_real_value(json_object_get(update_interval, "keyfile"));
    if (!loop || !update_interval || keyfile_update_interval < 1.0f || !host_domain ||
            !res_id_label || !detail_label) {
        _http_resp_code = 400;
        json_object_set_new(_err_info, "transcoder", json_string("[hls] missing arguments in spec for constructing playlist"));
        goto done;
    }
    hlsproc->asa_local.loop = loop;
#define  ASA_SRC_BASEPATH_PATTERN  "%s/%d/%08x"
    { // ensure unencrypted path of collected master playlist and crypto key file
        app_cfg_t *acfg = app_get_global_cfg();
        size_t filepath_sz = sizeof(ASA_SRC_BASEPATH_PATTERN) + strlen(acfg->tmp_buf.path) +
            USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1;
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, ASA_SRC_BASEPATH_PATTERN,
                acfg->tmp_buf.path, _usr_id, _upld_req_id);
        assert(filepath_sz >= nwrite);
        char *ptr = calloc((filepath_sz << 1), sizeof(char));
        asa_local->op.mkdir.path.prefix = NULL;
        asa_local->op.mkdir.path.origin = ptr;
        asa_local->op.mkdir.path.curr_parent = ptr + filepath_sz;
        strncpy(asa_local->op.mkdir.path.origin, &filepath[0], filepath_sz - 1);
        asa_local->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local->op.mkdir.cb  = _atfp_hls__ensure_local_basepath_cb;
        asa_result =  asa_local->storage->ops.fn_mkdir(asa_local, 1);
    }
#undef  ASA_SRC_BASEPATH_PATTERN
    if(asa_result != ASTORAGE_RESULT_ACCEPT) {
        _http_resp_code = 500;
        json_object_set_new(_err_info, "storage", json_string("[hls] unable to send command for updating master playlist"));
        fprintf(stderr, "[hls][init_stream] line:%d, failed to send mkdir cmd \r\n", __LINE__);
    }
done:
    if(json_object_size(_err_info) > 0) {
        json_object_set_new(_spec, "http_resp_code", json_integer(_http_resp_code));
        _atfp_hls__final_dealloc(processor, 0);
    }
} // end of atfp__video_hls__init_stream


static int  atfp_hls__encrypt_document_id (atfp_data_t *fp_data, json_t *kitem, unsigned char **out, size_t *out_sz)
{
    int success = 0;
    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    // NOTE, there is alternative cipher named  EVP_aes_128_cbc_hmac_sha1(), unfortunately it is
    // only intended for usage of TLS protocol.
    success = EVP_EncryptInit_ex (ctx, EVP_aes_128_cbc(), NULL, NULL, NULL);
    if(!success)
        goto done;
    json_t *iv_obj  = json_object_get(kitem, "iv");
    size_t  nbytes_iv  = (size_t) json_integer_value(json_object_get(iv_obj, "nbytes"));
    if(nbytes_iv != HLS__NBYTES_IV)
        goto done;
#if  0 // not used for AES-CBC
    success = EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, nbytes_iv, NULL);
    if(!success)
        goto done;
#endif
    success = atfp_encrypt_document_id (ctx, fp_data, kitem, out, out_sz);
done:
    if(ctx)
        EVP_CIPHER_CTX_free(ctx);
    return success;
} // end of atfp_hls__encrypt_document_id


atfp_t  *atfp__video_hls__instantiate_stream(void)
{
    atfp_t  *out = atfp__video_hls__instantiate();
    if(out) {
        atfp_hls_t *hlsproc = (atfp_hls_t *)out;
        hlsproc->internal.op.get_crypto_key = atfp_get_crypto_key;
        hlsproc->internal.op.encrypt_document_id = atfp_hls__encrypt_document_id;
        hlsproc->internal.op.build_master_playlist = atfp_hls_stream__build_mst_plist;
        hlsproc->internal.op.build_secondary_playlist = atfp_hls_stream__build_lvl2_plist;
        hlsproc->internal.op.acquire_key = atfp_hls_stream__acquire_key;
        hlsproc->internal.op.encrypt_segment = atfp_hls_stream__encrypt_segment__start;
    }
    return out;
} // end of  atfp__video_hls__instantiate_stream



uint8_t atfp__video_hls__deinit_stream_element(atfp_t *processor)
{
    atfp_hls_t  *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t  *_asa_local = &hlsproc->asa_local.super;
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    if(asa_src) {
        processor->data.storage.handle = NULL;
        asa_src->deinit(asa_src);
    }
    DEINIT_IF_EXISTS(processor->data.version, free);
    if(_asa_local->deinit) {
        _asa_local->deinit(_asa_local);
    } else {
        DEINIT_IF_EXISTS(processor, free);
    }
    return 0;
} // end of  atfp__video_hls__deinit_stream_element

void   atfp__video_hls__seek_stream_element (atfp_t *processor)
{
    json_t *_err_info = processor->data.error;
    json_t *_spec = processor->data.spec;
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    void  (*_fn)(atfp_hls_t *) = NULL;
    const char *detail = json_string_value(json_object_get(_spec, API_QPARAM_LABEL__DOC_DETAIL));
#define  CHECK_ELEMENT_FILE(fn0, fn1_name,  _pattern, _prefix_sz , _version_required, _extra_cond) \
    if (!fn0) { \
        size_t  pattern_sz = sizeof(_pattern) - 1; \
        int  ret = strncmp(&detail[_prefix_sz], _pattern, pattern_sz); \
        if((ret == 0) && (_extra_cond)) \
            fn0 = hlsproc->internal.op.fn1_name; \
        if(fn0 && _version_required && !processor->data.version) { \
            size_t  uint_sz = sizeof(uint32_t); \
            size_t  alloc_sz = uint_sz + APP_TRANSCODED_VERSION_SIZE - (APP_TRANSCODED_VERSION_SIZE % uint_sz); \
            processor->data.version = calloc(alloc_sz, sizeof(char)); \
            strncpy((char *)processor->data.version, &detail[0], APP_TRANSCODED_VERSION_SIZE); \
        } \
    }
    if(detail) {
        CHECK_ELEMENT_FILE(_fn, acquire_key,  HLS_REQ_KEYFILE_LABEL, 0, 0, 1)
        CHECK_ELEMENT_FILE(_fn, build_master_playlist,  HLS_MASTER_PLAYLIST_FILENAME, 0, 0, 1)
        CHECK_ELEMENT_FILE(_fn, build_secondary_playlist, HLS_PLAYLIST_FILENAME, APP_TRANSCODED_VERSION_SIZE + 1, 1,
                isalnum(detail[0]) && isalnum(detail[1]) && detail[APP_TRANSCODED_VERSION_SIZE] == '/')
        CHECK_ELEMENT_FILE(_fn, encrypt_segment, HLS_SEGMENT_FILENAME_PREFIX,  APP_TRANSCODED_VERSION_SIZE + 1, 0,
                isalnum(detail[0]) && isalnum(detail[1]) && detail[APP_TRANSCODED_VERSION_SIZE] == '/')
        CHECK_ELEMENT_FILE(_fn, encrypt_segment, HLS_FMP4_FILENAME, APP_TRANSCODED_VERSION_SIZE + 1, 0,
                isalnum(detail[0]) && isalnum(detail[1]) && detail[APP_TRANSCODED_VERSION_SIZE] == '/')
    }
    if (_fn) {
        _fn(hlsproc);
    } else {
        json_object_set_new(_err_info, "transcoder", json_string("[hls] invalid path"));
        json_object_set_new(_err_info, "_http_resp_code", json_integer(400));
    }
#undef  CHECK_ELEMENT_FILE
} // end of  atfp__video_hls__seek_stream_element
