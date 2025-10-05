#include "app_cfg.h"
#include "utils.h"
#include "transcoder/video/hls.h"

void atfp_hls_stream__acquire_key__final(atfp_hls_t *hlsproc) {
    atfp_t     *processor = &hlsproc->super;
    json_t     *keyitem = json_object_get(json_object_get(processor->data.spec, "_crypto_key"), "key");
    const char *key_hex = json_string_value(json_object_get(keyitem, "data"));
    size_t      key_rawbytes_sz = json_integer_value(json_object_get(keyitem, "nbytes"));
    char       *key_rawbytes = calloc(key_rawbytes_sz + 1, sizeof(char)); // ensure NULL-terminating string
    int         err = app_hexstr_to_chararray(key_rawbytes, key_rawbytes_sz, key_hex, strlen(key_hex));
    if (err) {
        fprintf(stderr, "[hls][crypto_key] line:%d, byte convert error, code:%d \r\n", __LINE__, err);
        key_rawbytes_sz = 0;
    }
    processor->transfer.streaming_dst.block.data = key_rawbytes;
    processor->transfer.streaming_dst.block.len = key_rawbytes_sz;
    processor->transfer.streaming_dst.flags.is_final = 1;
    processor->data.callback(processor);
} // end of  atfp_hls_stream__acquire_key__final

static void _atfp_hls__close_local_keyfile_cb(asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t     *processor = &hlsproc->super;
    hlsproc->internal.op.acquire_key = atfp_hls_stream__acquire_key__final;
    processor->data.callback(processor);
} // end of _atfp_hls__close_local_keyfile_cb

void atfp_hls_stream__acquire_key(atfp_hls_t *hlsproc) {
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    json_t *spec = processor->data.spec;
    void   *loop = (void *)json_integer_value(json_object_get(spec, "loop"));
    hlsproc->asa_local.loop = loop;
    asa_op_base_cfg_t *_asa_local = &hlsproc->asa_local.super;
    _asa_local->deinit = _atfp_hls__stream_seeker_asalocal_deinit;
    ASA_RES_CODE result = atfp_hls_stream__load_crypto_key__async(hlsproc, _atfp_hls__close_local_keyfile_cb);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[hls][crypto_key] line:%d, error on opening crypto key file \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    }
} // end of atfp_hls_stream__acquire_key
