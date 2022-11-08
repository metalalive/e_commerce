#include "app_cfg.h"
#include "utils.h"
#include "transcoder/video/hls.h"

void  atfp_hls_stream__acquire_key__final (atfp_hls_t *hlsproc)
{
    atfp_t *processor = & hlsproc->super;
    json_t  *keyitem   = json_object_get(json_object_get(processor->data.spec, "_crypto_key"), "key");
    const char *key_hex = json_string_value(json_object_get(keyitem, "data"));
    size_t key_rawbytes_sz   = json_integer_value(json_object_get(keyitem, "nbytes"));
    char  *key_rawbytes = calloc(key_rawbytes_sz + 1, sizeof(char)); // ensure NULL-terminating string
    int err = app_hexstr_to_chararray(key_rawbytes, key_rawbytes_sz, key_hex, strlen(key_hex));
    if(err) {
        fprintf(stderr, "[hls][crypto_key] line:%d, byte convert error, code:%d \r\n", __LINE__, err);
        key_rawbytes_sz = 0;
    }
    processor->transfer.streaming_dst.block.data = key_rawbytes;
    processor->transfer.streaming_dst.block.len  = key_rawbytes_sz;
    processor->transfer.streaming_dst.flags.is_final  = 1;
    processor->data.callback(processor);
} // end of  atfp_hls_stream__acquire_key__final

static  void _atfp_hls__close_local_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t  *processor = & hlsproc->super;
    hlsproc->internal.op.acquire_key = atfp_hls_stream__acquire_key__final;
    processor->data.callback(processor);
} // end of _atfp_hls__close_local_keyfile_cb


static  void _atfp_hls__open_local_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_hls_stream__load_crypto_key (hlsproc, hlsproc->asa_local.file.file);
        _asa_local->op.close.cb = _atfp_hls__close_local_keyfile_cb;
        result = _asa_local->storage->ops.fn_close(_asa_local);
    } else {
        fprintf(stderr, "[hls][crypto_key] line:%d, failed to open crypto key file \r\n", __LINE__);
        json_object_set_new(err_info, "_http_resp_code", json_integer(400));
        json_object_set_new(err_info, "storage", json_string("[hls] document outdated"));
    } // TODO, more advanced error handling, separate errors to client side 4xx or server side 5xx
    if(result != ASTORAGE_RESULT_ACCEPT)
        processor->data.callback(processor);
} // end of  _atfp_hls__open_local_keyfile_cb


void  atfp_hls_stream__acquire_key(atfp_hls_t *hlsproc)
{
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    uint32_t  _usr_id = processor->data.usr_id;
    uint32_t  _upld_req_id = processor->data.upld_req_id;
    void *loop = (void *) json_integer_value(json_object_get(spec, "loop"));
    hlsproc->asa_local.loop = loop;
    asa_op_base_cfg_t  *_asa_local = &hlsproc->asa_local.super;
    app_cfg_t *acfg = app_get_global_cfg();
#define  PATH_PATTERN  "%s/%d/%08x/%s"
    size_t filepath_sz = sizeof(PATH_PATTERN) + strlen(acfg->tmp_buf.path) + USR_ID_STR_SIZE +
          UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(HLS_CRYPTO_KEY_FILENAME);
    char filepath[filepath_sz];
    size_t nwrite = snprintf (&filepath[0], filepath_sz, PATH_PATTERN, acfg->tmp_buf.path,
             _usr_id, _upld_req_id, HLS_CRYPTO_KEY_FILENAME);
#undef  PATH_PATTERN
    assert(filepath_sz >= nwrite);
    _asa_local->op.open.dst_path = &filepath[0];
    _asa_local->op.open.mode  = S_IRUSR;
    _asa_local->op.open.flags = O_RDONLY;
    _asa_local->op.open.cb  = _atfp_hls__open_local_keyfile_cb;
    ASA_RES_CODE result = _asa_local->storage->ops.fn_open(_asa_local);
    _asa_local->op.open.dst_path = NULL;
    _asa_local->deinit = _atfp_hls__stream_seeker_asalocal_deinit;
    if(result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[hls][crypto_key] line:%d, error on opening crypto key file \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    }
} // end of atfp_hls_stream__acquire_key
