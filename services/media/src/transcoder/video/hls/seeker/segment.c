#include "app_cfg.h"
#include "transcoder/video/hls.h"
#define CIPHER_CTX_IDX__IN_ASA_USRARG (ATFP_INDEX__IN_ASA_USRARG + 1)
#define NUM_USRARGS_ASA_SRC           (CIPHER_CTX_IDX__IN_ASA_USRARG + 1)

static void _atfp_hls__deinit_asasrc_final(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    EVP_CIPHER_CTX *cipher_ctx = asa_src->cb_args.entries[CIPHER_CTX_IDX__IN_ASA_USRARG];
    if (cipher_ctx)
        EVP_CIPHER_CTX_free(cipher_ctx);
    free(asa_src);
}

static void _atfp_hls__stream_seeker_asasrc_deinit(asa_op_base_cfg_t *asa_src) {
    asa_src->op.close.cb = _atfp_hls__deinit_asasrc_final;
    ASA_RES_CODE result = asa_src->storage->ops.fn_close(asa_src);
    if (result != ASTORAGE_RESULT_ACCEPT)
        _atfp_hls__deinit_asasrc_final(asa_src, result);
}

static void _atfp_hls_stream__segfile_read_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread) {
    atfp_hls_t     *hlsproc = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    EVP_CIPHER_CTX *cipher_ctx = asa_src->cb_args.entries[CIPHER_CTX_IDX__IN_ASA_USRARG];
    atfp_t         *processor = &hlsproc->super;
    json_t         *err_info = processor->data.error;
    int             num_encrypt = 0, tot_num_encrypt = 0, success = 0;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint8_t _eof_reached = nread < asa_src->op.read.dst_sz;
        processor->transfer.streaming_dst.flags.eof_reached = _eof_reached;
        processor->transfer.streaming_dst.flags.is_final = _eof_reached;
        assert(asa_src->op.read.dst_sz >= nread);
        asa_src->op.read.dst[nread] = 0x0;
        unsigned char *wr_ptr = (unsigned char *)processor->transfer.streaming_dst.block.data;
        success = EVP_EncryptUpdate(
            cipher_ctx, wr_ptr, &num_encrypt, (const unsigned char *)asa_src->op.read.dst, nread
        );
        if (success) {
            tot_num_encrypt += num_encrypt;
            wr_ptr += num_encrypt;
            if (_eof_reached) {
                success = EVP_EncryptFinal_ex(cipher_ctx, wr_ptr, &num_encrypt);
                if (success) {
                    tot_num_encrypt += num_encrypt;
                    wr_ptr += num_encrypt;
                } else {
                    fprintf(stderr, "[hls][segm] line:%d, failed to finalize encryption \r\n", __LINE__);
                }
            }
            int    block_octet_sz = EVP_CIPHER_CTX_block_size(cipher_ctx);
            size_t _rdbuf_max_sz = asa_src->op.read.dst_max_nbytes - 1;
            size_t _rdbuf_max_aligned_sz = _rdbuf_max_sz - (_rdbuf_max_sz % block_octet_sz);
            size_t ct_max_sz = _rdbuf_max_aligned_sz + block_octet_sz + 1;
            assert(tot_num_encrypt < ct_max_sz);
            // not necessary, in case app caller will treat this as NULL-terminating octet string
            *wr_ptr = 0; // tot_num_encrypt += 1;
            processor->transfer.streaming_dst.block.len = tot_num_encrypt;
        } else {
            fprintf(stderr, "[hls][segm] line:%d, encryption error \r\n", __LINE__);
        }
    } else {
        fprintf(stderr, "[hls][segm] line:%d, error on reading src segment \r\n", __LINE__);
    }
    if (!success)
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    processor->data.callback(processor);
} // end of _atfp_hls_stream__segfile_read_cb

void atfp_hls_stream__encrypt_segment__continue(atfp_hls_t *hlsproc) {
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    processor->transfer.streaming_dst.block.len = 0;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    EVP_CIPHER_CTX    *cipher_ctx = asa_src->cb_args.entries[CIPHER_CTX_IDX__IN_ASA_USRARG];
    int                block_octet_sz = EVP_CIPHER_CTX_block_size(cipher_ctx); // 16 octet in HLS protocol
    // reserve last byte for NULL-terminating char
    size_t _rdbuf_max_sz = asa_src->op.read.dst_max_nbytes - 1;
    size_t _rdbuf_max_aligned_sz = _rdbuf_max_sz - (_rdbuf_max_sz % block_octet_sz);
    if (!processor->transfer.streaming_dst.block.data) {
        size_t ct_max_sz = _rdbuf_max_aligned_sz + block_octet_sz + 1;
        processor->transfer.streaming_dst.block.data = calloc(ct_max_sz, sizeof(char));
    }
    asa_src->op.read.offset = asa_src->op.seek.pos;
    asa_src->op.read.dst_sz = _rdbuf_max_aligned_sz;
    asa_src->op.read.cb = _atfp_hls_stream__segfile_read_cb;
    ASA_RES_CODE result = asa_src->storage->ops.fn_read(asa_src);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[hls][segm] line:%d, error on reading src segment \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    }
} // end of atfp_hls_stream__encrypt_segment__continue

static EVP_CIPHER_CTX *_atfp_hls_stream__init_cipher(json_t *spec, json_t *err_info) {
    unsigned char  *key_octet = NULL, *iv_octet = NULL;
    int             success = 0;
    EVP_CIPHER_CTX *cipher_ctx = EVP_CIPHER_CTX_new();
    if (!cipher_ctx) {
        fprintf(stderr, "[hls][segm] line:%d, malloc failure on cipher ctx \r\n", __LINE__);
        goto done;
    }
    success = EVP_EncryptInit_ex(cipher_ctx, EVP_aes_128_cbc(), NULL, NULL, NULL);
    if (!success) {
        fprintf(stderr, "[hls][segm] line:%d, failed to init cipher ctx \r\n", __LINE__);
        goto done;
    }
    json_t *kitem = json_object_get(spec, "_crypto_key");
    json_t *key_obj = json_object_get(kitem, "key");
    json_t *iv_obj = json_object_get(kitem, "iv");
    size_t  expect_nbytes_iv = (size_t)json_integer_value(json_object_get(iv_obj, "nbytes"));
    size_t  expect_nbytes_key = (size_t)json_integer_value(json_object_get(key_obj, "nbytes"));
    long    actual_nbytes_key = 0, actual_nbytes_iv = 0;
    success = expect_nbytes_iv == HLS__NBYTES_IV;
    if (!success) {
        fprintf(
            stderr, "[hls][segm] line:%d, IV length mismatch in key item, expect=%d, actual=%ld \r\n",
            __LINE__, HLS__NBYTES_IV, expect_nbytes_iv
        );
        goto done;
    }
    success = expect_nbytes_key == HLS__NBYTES_KEY;
    if (!success) {
        fprintf(
            stderr, "[hls][segm] line:%d, key length mismatch in key item, expect=%d, actual=%ld \r\n",
            __LINE__, HLS__NBYTES_KEY, expect_nbytes_key
        );
        goto done;
    }
    const char *key_hex = json_string_value(json_object_get(key_obj, "data"));
    const char *iv_hex = json_string_value(json_object_get(iv_obj, "data"));
    key_octet = OPENSSL_hexstr2buf(key_hex, &actual_nbytes_key);
    iv_octet = OPENSSL_hexstr2buf(iv_hex, &actual_nbytes_iv);
    if (!key_octet || !iv_octet || actual_nbytes_key != expect_nbytes_key ||
        actual_nbytes_iv != expect_nbytes_iv) {
        fprintf(
            stderr,
            "[hls][segm] line:%d, hex string cannot be converted to octet array,"
            " key=%ld, IV=%ld \r\n",
            __LINE__, actual_nbytes_key, actual_nbytes_iv
        );
        success = 0;
        goto done;
    }
    success = EVP_EncryptInit_ex(
        cipher_ctx, NULL, NULL, (const unsigned char *)&key_octet[0], (const unsigned char *)&iv_octet[0]
    );
    if (success)
        assert(EVP_CIPHER_CTX_block_size(cipher_ctx) == HLS__NBYTES_IV);
done:
    if (!success) {
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        EVP_CIPHER_CTX_free(cipher_ctx);
        cipher_ctx = NULL;
    }
    if (key_octet)
        OPENSSL_free(key_octet);
    if (iv_octet)
        OPENSSL_free(iv_octet);
    return cipher_ctx;
} // end of  _atfp_hls_stream__init_cipher

static void _atfp_hls__close_local_keyfile_cb(asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t     *processor = &hlsproc->super;
    json_t     *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
        EVP_CIPHER_CTX    *cipher_ctx = _atfp_hls_stream__init_cipher(processor->data.spec, err_info);
        asa_src->cb_args.entries[CIPHER_CTX_IDX__IN_ASA_USRARG] = cipher_ctx;
    } else {
        fprintf(stderr, "[hls][segm] line:%d, error on closing crypto key file \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    }
    if (json_object_size(err_info) == 0)
        hlsproc->internal.op.encrypt_segment = atfp_hls_stream__encrypt_segment__continue;
    processor->data.callback(processor);
} // end of _atfp_hls__close_local_keyfile_cb

static void _atfp_hls__open_src_segfile_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &hlsproc->super;
    json_t     *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        result = atfp_hls_stream__load_crypto_key__async(hlsproc, _atfp_hls__close_local_keyfile_cb);
        if (result != ASTORAGE_RESULT_ACCEPT) {
            fprintf(stderr, "[hls][segm] line:%d, error on opening crypto key file \r\n", __LINE__);
            json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        }
    } else { // it is possible to have other video quality encoded with non-HLS format
        fprintf(stderr, "[hls][segm] line:%d, error on opening src secondary playlist \r\n", __LINE__);
        json_object_set_new(err_info, "_http_resp_code", json_integer(404));
        json_object_set_new(err_info, "storage", json_string("[hls] error on fetching segment"));
    } // TODO, more advanced error handling, separate errors to client side 4xx or server side 5xx
    if (json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of  _atfp_hls__open_src_segfile_cb

static ASA_RES_CODE atfp_hls_stream__enc_seg__init_asasrc(asa_op_base_cfg_t *asa_src, atfp_t *processor) {
    json_t     *spec = processor->data.spec;
    uint32_t    _usr_id = processor->data.usr_id;
    uint32_t    _upld_req_id = processor->data.upld_req_id;
    const char *_detail_path = json_string_value(json_object_get(spec, API_QPARAM_LABEL__DOC_DETAIL));
#define PATH_PATTERN "%d/%08x/%s/%s"
    size_t filepath_sz = sizeof(PATH_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id) +
                         sizeof(ATFP__COMMITTED_FOLDER_NAME) + strlen(_detail_path);
    char   filepath[filepath_sz];
    size_t nwrite = snprintf(
        &filepath[0], filepath_sz, PATH_PATTERN, _usr_id, _upld_req_id, ATFP__COMMITTED_FOLDER_NAME,
        _detail_path
    );
#undef PATH_PATTERN
    assert(filepath_sz >= nwrite);
    asa_src->op.open.dst_path = &filepath[0];
    asa_src->op.open.mode = S_IRUSR;
    asa_src->op.open.flags = O_RDONLY;
    asa_src->op.open.cb = _atfp_hls__open_src_segfile_cb;
    ASA_RES_CODE result = asa_src->storage->ops.fn_open(asa_src);
    asa_src->op.open.dst_path = NULL;
    // not apply the deinit function in seeker/common.c
    asa_src->deinit = _atfp_hls__stream_seeker_asasrc_deinit;
    return result;
} // end of  atfp_hls_stream__enc_seg__init_asasrc

void atfp_hls_stream__encrypt_segment__start(atfp_hls_t *hlsproc) {
    atfp_t *processor = &hlsproc->super;
    json_t *spec = processor->data.spec;
    size_t  _rdbuf_max_sz = json_integer_value(json_object_get(spec, "buf_max_sz"));
    json_object_set_new(spec, "wrbuf_max_sz", json_integer(_rdbuf_max_sz)); // TODO, parameterize
    json_object_set_new(spec, "num_usrargs_asa_src", json_integer(NUM_USRARGS_ASA_SRC));
    atfp_hls_stream_seeker__init_common(hlsproc, atfp_hls_stream__enc_seg__init_asasrc);
    json_object_del(spec, "num_usrargs_asa_src");
}
