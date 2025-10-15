#include <search.h>
#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "utils.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"
#include "../test/unit/transcoder/test.h"

#define UTEST_FILE_BASEPATH            "tmp/utest"
#define UTEST_ASASRC_BASEPATH          UTEST_FILE_BASEPATH "/asasrc"
#define RUNNER_CREATE_FOLDER(fullpath) mkdir(fullpath, S_IRWXU)
#define UTEST_ASALOCAL_BASEPATH        UTEST_FILE_BASEPATH "/asalocal"

#define DONE_FLAG_INDEX__IN_ASA_USRARG (ATFP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ             (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define MOCK_STORAGE_ALIAS             "localfs"

#define MOCK_USER_ID          104
#define MOCK_UPLD_REQ_1_ID    0xdee5d1d0
#define MOCK_VERSION_STR      "OB"
#define MOCK_ENCRYPTED_DOC_ID "0YL2y+asirW7tG="
#define UTEST_SEG_NUM         "0012"
#define UTEST_QPARAM__DETAIL  MOCK_VERSION_STR "/" HLS_SEGMENT_FILENAME_PREFIX UTEST_SEG_NUM

#define MOCK_STORAGE_SRC_USRBUF_PATH      UTEST_ASASRC_BASEPATH "/104"
#define MOCK_STORAGE_SRC_UPLD_REQ_PATH    MOCK_STORAGE_SRC_USRBUF_PATH "/dee5d1d0"
#define MOCK_STORAGE_SRC_UPLD_COMMIT_PATH MOCK_STORAGE_SRC_UPLD_REQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME

#define MOCK_STORAGE_LOCAL_USRBUF_PATH   UTEST_ASALOCAL_BASEPATH "/104"
#define MOCK_STORAGE_LOCAL_UPLD_REQ_PATH MOCK_STORAGE_LOCAL_USRBUF_PATH "/dee5d1d0"

static void _utest_hls_enc_segm__common_done_cb(atfp_t *processor) {
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    json_t            *err_info = processor->data.error;
    size_t             err_cnt = json_object_size(err_info);
    char              *out_chunkbytes = processor->transfer.streaming_dst.block.data;
    size_t             out_chunkbytes_sz = processor->transfer.streaming_dst.block.len;
    uint8_t            is_final = processor->transfer.streaming_dst.flags.is_final;
    uint8_t            eof_reached = processor->transfer.streaming_dst.flags.eof_reached;
    mock(asa_src, err_cnt, out_chunkbytes, out_chunkbytes_sz, is_final, eof_reached);
    asa_op_base_cfg_t *_asa_local = &((atfp_hls_t *)processor)->asa_local.super;
    if (_asa_local->cb_args.entries) {
        uint8_t *done_flg_p = _asa_local->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if (done_flg_p)
            *done_flg_p = 1;
    }
} // end of _utest_hls_enc_segm__common_done_cb

#define HLS__ENCRYPT_SEGMENT_START__SETUP \
    uint8_t     mock_done_flag = 0; \
    void       *mock_asalocal_cb_args[NUM_CB_ARGS_ASAOBJ] = {NULL, &mock_done_flag}; \
    int         mock_cipher_ctx = 0, mock_cipher_aes = 0; \
    uv_loop_t  *loop = uv_default_loop(); \
    json_t     *mock_spec = json_object(), *mock_err_info = json_object(); \
    json_t     *mock_doc_metadata = json_object(); \
    app_cfg_t  *mock_appcfg = app_get_global_cfg(); \
    const char *sys_basepath = mock_appcfg->env_vars.sys_base_path; \
    asa_cfg_t   mock_storage_src_cfg = { \
          .alias = MOCK_STORAGE_ALIAS, \
          .base_path = PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, strdup), \
          .ops = \
            {.fn_read = app_storage_localfs_read, \
               .fn_open = app_storage_localfs_open, \
               .fn_close = app_storage_localfs_close, \
               .fn_typesize = app_storage_localfs_typesize} \
    }; \
    asa_cfg_t mock_storage_local_cfg = { \
        .alias = NULL, \
        .base_path = sys_basepath, \
        .ops = \
            {.fn_read = app_storage_localfs_read, \
             .fn_open = app_storage_localfs_open, \
             .fn_close = app_storage_localfs_close, \
             .fn_typesize = app_storage_localfs_typesize} \
    }; \
    mock_appcfg->storages.size = 1; \
    mock_appcfg->storages.capacity = 1; \
    mock_appcfg->storages.entries = &mock_storage_src_cfg; \
    mock_appcfg->tmp_buf.path = UTEST_ASALOCAL_BASEPATH; \
    atfp_hls_t mock_fp = { \
        .super = \
            {.data = \
                 {.callback = _utest_hls_enc_segm__common_done_cb, \
                  .spec = mock_spec, \
                  .error = mock_err_info, \
                  .usr_id = MOCK_USER_ID, \
                  .upld_req_id = MOCK_UPLD_REQ_1_ID, \
                  .version = NULL, \
                  .storage = {.handle = NULL}}}, \
        .asa_local = \
            {.super = \
                 {.storage = &mock_storage_local_cfg, \
                  .cb_args = {.entries = mock_asalocal_cb_args, .size = NUM_CB_ARGS_ASAOBJ}}}, \
        .internal = \
            {.op = \
                 {.encrypt_segment = atfp_hls_stream__encrypt_segment__start, \
                  .get_crypto_key = atfp_get_crypto_key}} \
    }; \
    json_object_set_new(mock_spec, API_QPARAM_LABEL__DOC_DETAIL, json_string(UTEST_QPARAM__DETAIL)); \
    json_object_set_new(mock_spec, "loop", json_integer((uint64_t)loop)); \
    json_object_set_new(mock_spec, "buf_max_sz", json_integer(RD_BUF_MAX_SZ)); \
    json_object_set_new(mock_spec, "storage_alias", json_string(MOCK_STORAGE_ALIAS)); \
    json_object_set_new(mock_spec, "metadata", mock_doc_metadata); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASALOCAL_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_USRBUF_PATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_USRBUF_PATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_REQ_PATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_UPLD_REQ_PATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_COMMIT_PATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN( \
        sys_basepath, MOCK_STORAGE_SRC_UPLD_COMMIT_PATH "/" MOCK_VERSION_STR, RUNNER_CREATE_FOLDER \
    ); \
    { \
        const char *_wr_buf = UTEST__SEGM_ORIGIN_CONTENT; \
        size_t      _wr_buf_sz = sizeof(UTEST__SEGM_ORIGIN_CONTENT) - 1; \
        UTEST_RUN_OPERATION_WITH_PATH( \
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            ATFP__COMMITTED_FOLDER_NAME "/" UTEST_QPARAM__DETAIL, UTEST_OPS_WRITE2FILE \
        ); \
    }

#define HLS__ENCRYPT_SEGMENT_START__TEARDOWN \
    { \
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle; \
        if (asa_src) \
            asa_src->deinit(asa_src); \
        uv_run(loop, UV_RUN_ONCE); \
    }; \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_COMMIT_PATH "/" UTEST_QPARAM__DETAIL, unlink); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_COMMIT_PATH "/" MOCK_VERSION_STR, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_COMMIT_PATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_UPLD_REQ_PATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_UPLD_REQ_PATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_SRC_USRBUF_PATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_USRBUF_PATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASALOCAL_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, rmdir); \
    mock_appcfg->storages.size = 0; \
    mock_appcfg->storages.capacity = 0; \
    mock_appcfg->storages.entries = NULL; \
    mock_appcfg->tmp_buf.path = NULL; \
    free(mock_storage_src_cfg.base_path); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

Ensure(atfp_hls_test__enc_seg__start_ok) {
#define UTEST__CRYPTOKEY_CHOSEN_ID   "8134EADF"
#define UTEST__CRYPTO_CHOSEN_IV_HEX  "5D4A368331751A3844A383131751A384"
#define UTEST__CRYPTO_CHOSEN_KEY_HEX "D4A38331751A3845D4A38331751A384E"
#define UTEST__SEGM_ORIGIN_CONTENT   "expected content of given segment file"
#define UTEST__CRYPTOKEY_MIN_CONTENT \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \"" UTEST__CRYPTOKEY_CHOSEN_ID "\":{" \
    "\"iv\": {\"nbytes\":16,\"data\":\"" UTEST__CRYPTO_CHOSEN_IV_HEX "\"}," \
    "\"key\":{\"nbytes\":16,\"data\":\"" UTEST__CRYPTO_CHOSEN_KEY_HEX "\"}," \
    " \"alg\":\"aes\"}}"
#define RD_BUF_MAX_SZ (sizeof(UTEST__SEGM_ORIGIN_CONTENT) + 1)
    HLS__ENCRYPT_SEGMENT_START__SETUP
    long           expect_key_octet_sz = HLS__NBYTES_KEY, expect_iv_octet_sz = HLS__NBYTES_IV;
    unsigned char *mock_key_octet =
        (unsigned char *)"\x5D\x4A\x36\x83\x31\x75\x1A\x38\x44\xA3\x83\x13\x17\x51\xA3\x84";
    unsigned char *mock_iv_octet =
        (unsigned char *)"\xD4\xA3\x83\x31\x75\x1A\x38\x45\xD4\xA3\x83\x31\x75\x1A\x38\x4E";
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID));
    {
        const char *_wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        size_t      _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, HLS_CRYPTO_KEY_FILENAME,
            UTEST_OPS_WRITE2FILE
        );
    }
    mock_fp.internal.op.encrypt_segment(&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if (err_cnt == 0) {
        expect(EVP_aes_128_cbc, will_return(&mock_cipher_aes));
        expect(EVP_CIPHER_CTX_new, will_return(&mock_cipher_ctx));
        expect(
            EVP_EncryptInit_ex, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(key, is_equal_to(NULL)), when(iv, is_equal_to(NULL))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(mock_key_octet),
            when(str, is_equal_to_string(UTEST__CRYPTO_CHOSEN_KEY_HEX)),
            will_set_contents_of_parameter(len, &expect_key_octet_sz, sizeof(long))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(mock_iv_octet),
            when(str, is_equal_to_string(UTEST__CRYPTO_CHOSEN_IV_HEX)),
            will_set_contents_of_parameter(len, &expect_iv_octet_sz, sizeof(long))
        );
        expect(
            EVP_EncryptInit_ex, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(key, is_not_equal_to(NULL)), when(iv, is_not_equal_to(NULL))
        );
        expect(CRYPTO_free, when(addr, is_equal_to(mock_key_octet)));
        expect(CRYPTO_free, when(addr, is_equal_to(mock_iv_octet)));
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            _utest_hls_enc_segm__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_equal_to(0)),
            when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(0)),
            when(out_chunkbytes_sz, is_equal_to(0)), when(out_chunkbytes, is_null)
        );
        while (!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle;
        assert_that(asa_src->cb_args.entries, is_not_null);
        assert_that(asa_src->cb_args.entries[1], is_equal_to(&mock_cipher_ctx));
        assert_that(
            mock_fp.internal.op.encrypt_segment, is_equal_to(atfp_hls_stream__encrypt_segment__continue)
        );
    }
    expect(EVP_CIPHER_CTX_free, when(ctx, is_equal_to(&mock_cipher_ctx)));
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_UPLD_REQ_PATH "/" HLS_CRYPTO_KEY_FILENAME, unlink);
    HLS__ENCRYPT_SEGMENT_START__TEARDOWN
#undef RD_BUF_MAX_SZ
#undef UTEST__CRYPTOKEY_MIN_CONTENT
#undef UTEST__SEGM_ORIGIN_CONTENT
#undef UTEST__CRYPTOKEY_CHOSEN_ID
#undef UTEST__CRYPTO_CHOSEN_IV_HEX
#undef UTEST__CRYPTO_CHOSEN_KEY_HEX
} // end of  atfp_hls_test__enc_seg__start_ok

Ensure(atfp_hls_test__enc_seg__start_key_error) {
#define UTEST__CRYPTOKEY_CHOSEN_ID   "8134EADF"
#define UTEST__CRYPTO_CHOSEN_IV_HEX  "5D4A3683317A3844A3131751A384"
#define UTEST__CRYPTO_CHOSEN_KEY_HEX "D4A383311A3845D4A38331751A384E"
#define UTEST__SEGM_ORIGIN_CONTENT   "expected content of given segment file"
#define UTEST__CRYPTOKEY_MIN_CONTENT \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \"" UTEST__CRYPTOKEY_CHOSEN_ID "\":{" \
    "\"iv\": {\"nbytes\":16,\"data\":\"" UTEST__CRYPTO_CHOSEN_IV_HEX "\"}," \
    "\"key\":{\"nbytes\":16,\"data\":\"" UTEST__CRYPTO_CHOSEN_KEY_HEX "\"}," \
    " \"alg\":\"aes\"}}"
#define RD_BUF_MAX_SZ (sizeof(UTEST__SEGM_ORIGIN_CONTENT) + 1)
    HLS__ENCRYPT_SEGMENT_START__SETUP
    long           expect_key_octet_sz = 15, expect_iv_octet_sz = 14;
    unsigned char *mock_key_octet =
        (unsigned char *)"\x5D\x4A\x36\x83\x31\x75\x1A\x44\xA3\x83\x13\x17\x51\xA3\x84";
    unsigned char *mock_iv_octet =
        (unsigned char *)"\xD4\xA3\x83\x75\x1A\x45\xD4\xA3\x83\x31\x75\x1A\x38\x4E";
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID));
    {
        const char *_wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        size_t      _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, HLS_CRYPTO_KEY_FILENAME,
            UTEST_OPS_WRITE2FILE
        );
    }
    mock_fp.internal.op.encrypt_segment(&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if (err_cnt == 0) {
        expect(EVP_aes_128_cbc, will_return(&mock_cipher_aes));
        expect(EVP_CIPHER_CTX_new, will_return(&mock_cipher_ctx));
        expect(
            EVP_EncryptInit_ex, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(key, is_equal_to(NULL)), when(iv, is_equal_to(NULL))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(mock_key_octet),
            when(str, is_equal_to_string(UTEST__CRYPTO_CHOSEN_KEY_HEX)),
            will_set_contents_of_parameter(len, &expect_key_octet_sz, sizeof(long))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(mock_iv_octet),
            when(str, is_equal_to_string(UTEST__CRYPTO_CHOSEN_IV_HEX)),
            will_set_contents_of_parameter(len, &expect_iv_octet_sz, sizeof(long))
        );
        expect(CRYPTO_free, when(addr, is_equal_to(mock_key_octet)));
        expect(CRYPTO_free, when(addr, is_equal_to(mock_iv_octet)));
        expect(EVP_CIPHER_CTX_free, when(ctx, is_equal_to(&mock_cipher_ctx)));
        expect(
            _utest_hls_enc_segm__common_done_cb, when(asa_src, is_not_null),
            when(err_cnt, is_greater_than(0)), when(is_final, is_equal_to(0)),
            when(eof_reached, is_equal_to(0))
        );
        while (!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle;
        assert_that(asa_src->cb_args.entries, is_not_null);
        assert_that(asa_src->cb_args.entries[1], is_equal_to(NULL));
    }
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_UPLD_REQ_PATH "/" HLS_CRYPTO_KEY_FILENAME, unlink);
    HLS__ENCRYPT_SEGMENT_START__TEARDOWN
#undef RD_BUF_MAX_SZ
#undef UTEST__CRYPTOKEY_MIN_CONTENT
#undef UTEST__SEGM_ORIGIN_CONTENT
#undef UTEST__CRYPTOKEY_CHOSEN_ID
#undef UTEST__CRYPTO_CHOSEN_IV_HEX
#undef UTEST__CRYPTO_CHOSEN_KEY_HEX
} // end of  atfp_hls_test__enc_seg__start_key_error

Ensure(atfp_hls_test__enc_seg__start_crypto_error) {
#define UTEST__CRYPTOKEY_CHOSEN_ID   "8134EADF"
#define UTEST__CRYPTO_CHOSEN_IV_HEX  "5D4A3683317A3844A3131751A384"
#define UTEST__CRYPTO_CHOSEN_KEY_HEX "D4A383311A385D4A3831751A384E"
#define UTEST__SEGM_ORIGIN_CONTENT   "expected content of given segment file"
#define UTEST__CRYPTOKEY_MIN_CONTENT \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \"" UTEST__CRYPTOKEY_CHOSEN_ID "\":{" \
    "\"iv\": {\"nbytes\":14,\"data\":\"" UTEST__CRYPTO_CHOSEN_IV_HEX "\"}," \
    "\"key\":{\"nbytes\":14,\"data\":\"" UTEST__CRYPTO_CHOSEN_KEY_HEX "\"}," \
    " \"alg\":\"aes\"}}"
#define RD_BUF_MAX_SZ (sizeof(UTEST__SEGM_ORIGIN_CONTENT) + 1)
    HLS__ENCRYPT_SEGMENT_START__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID));
    {
        const char *_wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        size_t      _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, HLS_CRYPTO_KEY_FILENAME,
            UTEST_OPS_WRITE2FILE
        );
    }
    mock_fp.internal.op.encrypt_segment(&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if (err_cnt == 0) {
        int success = 0;
        expect(EVP_aes_128_cbc, will_return(&mock_cipher_aes));
        expect(EVP_CIPHER_CTX_new, will_return(&mock_cipher_ctx));
        expect(
            EVP_EncryptInit_ex, will_return(success), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(key, is_equal_to(NULL)), when(iv, is_equal_to(NULL))
        );
        expect(EVP_CIPHER_CTX_free, when(ctx, is_equal_to(&mock_cipher_ctx)));
        expect(
            _utest_hls_enc_segm__common_done_cb, when(asa_src, is_not_null),
            when(err_cnt, is_greater_than(0)), when(is_final, is_equal_to(0)),
            when(eof_reached, is_equal_to(0))
        );
        while (!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle;
        assert_that(asa_src->cb_args.entries, is_not_null);
        assert_that(asa_src->cb_args.entries[1], is_equal_to(NULL));
    }
    PATH_CONCAT_THEN_RUN(sys_basepath, MOCK_STORAGE_LOCAL_UPLD_REQ_PATH "/" HLS_CRYPTO_KEY_FILENAME, unlink);
    HLS__ENCRYPT_SEGMENT_START__TEARDOWN
#undef RD_BUF_MAX_SZ
#undef UTEST__CRYPTOKEY_MIN_CONTENT
#undef UTEST__SEGM_ORIGIN_CONTENT
#undef UTEST__CRYPTOKEY_CHOSEN_ID
#undef UTEST__CRYPTO_CHOSEN_IV_HEX
#undef UTEST__CRYPTO_CHOSEN_KEY_HEX
} // end of  atfp_hls_test__enc_seg__start_crypto_error

static ASA_RES_CODE utest_storage_fn_read(asa_op_base_cfg_t *asaobj) {
    ASA_RES_CODE evt_result = ASTORAGE_RESULT_UNKNOWN_ERROR, *evt_result_p = &evt_result;
    char        *rd_data_p = asaobj->op.read.dst;
    size_t       nread = 0, *nread_p = &nread;
    mock(rd_data_p, evt_result_p, nread_p);
    asaobj->op.read.cb(asaobj, evt_result, nread);
    return ASTORAGE_RESULT_ACCEPT;
}

#define HLS__ENCRYPT_SEGMENT_CONTINUE__SETUP \
    int       mock_cipher_ctx = 0; \
    char      mock_read_buf[RD_BUF_MAX_SZ] = {0}; \
    json_t   *mock_spec = json_object(), *mock_err_info = json_object(); \
    void     *mock_asasrc_cb_args[2] = {NULL, &mock_cipher_ctx}; \
    asa_cfg_t mock_storage_cfg = {.alias = MOCK_STORAGE_ALIAS, .ops = {.fn_read = utest_storage_fn_read}}; \
    asa_op_base_cfg_t mock_asa_src = { \
        .storage = &mock_storage_cfg, \
        .cb_args = {.entries = mock_asasrc_cb_args, .size = 2}, \
        .op = {.read = {.dst_max_nbytes = RD_BUF_MAX_SZ, .dst = &mock_read_buf[0]}} \
    }; \
    atfp_hls_t mock_fp = { \
        .super = \
            {.data = \
                 {.callback = _utest_hls_enc_segm__common_done_cb, \
                  .spec = mock_spec, \
                  .error = mock_err_info, \
                  .storage = {.handle = &mock_asa_src}}}, \
        .internal = {.op = {.encrypt_segment = atfp_hls_stream__encrypt_segment__continue}} \
    }; \
    mock_asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp;

#define HLS__ENCRYPT_SEGMENT_CONTINUE__TEARDOWN \
    free(mock_fp.super.transfer.streaming_dst.block.data); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

#define PLAINTXT_BLOCK1 "expected content"
#define PLAINTXT_BLOCK2 " of given segmen"
#define PLAINTXT_BLOCK3 "t file, or it co"
#define PLAINTXT_BLOCK4 "uld possibly be "
#define PLAINTXT_BLOCK5 "initial packet m"
#define PLAINTXT_BLOCK6 "ap for fMP4"

#define CIPHERTXT_BLOCK1 "030G23#9tYraMhe$"
#define CIPHERTXT_BLOCK2 "30G23#9tYraMhe$R"
#define CIPHERTXT_BLOCK3 "23#9tYraMhe$Rk9o"
#define CIPHERTXT_BLOCK4 "Mh238Ut4tihg984y"
#define CIPHERTXT_BLOCK5 "#9tYraMhe$Rk9tvN"
#define CIPHERTXT_BLOCK6 "9tYraMhe$Rk9vVip"

Ensure(atfp_hls_test__enc_seg__continue_ok) {
#define SEGM_CONTENT_CHUNK1 PLAINTXT_BLOCK1 PLAINTXT_BLOCK2 PLAINTXT_BLOCK3
#define SEGM_CONTENT_CHUNK2 PLAINTXT_BLOCK4 PLAINTXT_BLOCK5 PLAINTXT_BLOCK6
#define EXPECT_CIPHERTXT1   CIPHERTXT_BLOCK1 CIPHERTXT_BLOCK2 CIPHERTXT_BLOCK3
#define EXPECT_CIPHERTXT2   CIPHERTXT_BLOCK4 CIPHERTXT_BLOCK5
#define EXPECT_CIPHERTXT3   CIPHERTXT_BLOCK6
#define RD_BUF_MAX_SZ       sizeof(SEGM_CONTENT_CHUNK1) + 3
    HLS__ENCRYPT_SEGMENT_CONTINUE__SETUP
    ASA_RES_CODE expect_evt_result = ASTORAGE_RESULT_COMPLETE;
    { // subcase 1
        size_t chunk_sz = sizeof(char) * (sizeof(SEGM_CONTENT_CHUNK1) - 1);
        size_t encrypt_update_sz = chunk_sz - (chunk_sz % 16);
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            utest_storage_fn_read,
            will_set_contents_of_parameter(rd_data_p, SEGM_CONTENT_CHUNK1, encrypt_update_sz),
            will_set_contents_of_parameter(nread_p, &encrypt_update_sz, sizeof(size_t)),
            will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
        );
        expect(
            EVP_EncryptUpdate, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(in, is_equal_to_string(SEGM_CONTENT_CHUNK1)), when(inl, is_equal_to(encrypt_update_sz)),
            will_set_contents_of_parameter(out, EXPECT_CIPHERTXT1, encrypt_update_sz),
            will_set_contents_of_parameter(outl, &encrypt_update_sz, sizeof(int)),
        );
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            _utest_hls_enc_segm__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_equal_to(0)),
            when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(0)),
            when(out_chunkbytes_sz, is_equal_to(encrypt_update_sz)), when(out_chunkbytes, is_not_null),
            when(out_chunkbytes, is_equal_to_string(EXPECT_CIPHERTXT1))
        );
        mock_fp.internal.op.encrypt_segment(&mock_fp);
    }
    { // subcase 2
        size_t chunk_sz = sizeof(char) * (sizeof(SEGM_CONTENT_CHUNK2) - 1);
        size_t encrypt_update_sz = chunk_sz - (chunk_sz % 16);
        size_t encrypt_final_sz = sizeof(EXPECT_CIPHERTXT3) - 1;
        size_t encrypt_total_sz = sizeof(EXPECT_CIPHERTXT2 EXPECT_CIPHERTXT3) - 1;
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            utest_storage_fn_read, will_set_contents_of_parameter(rd_data_p, SEGM_CONTENT_CHUNK2, chunk_sz),
            will_set_contents_of_parameter(nread_p, &chunk_sz, sizeof(size_t)),
            will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
        );
        expect(
            EVP_EncryptUpdate, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(in, is_equal_to_string(SEGM_CONTENT_CHUNK2)), when(inl, is_equal_to(chunk_sz)),
            will_set_contents_of_parameter(out, EXPECT_CIPHERTXT2, encrypt_update_sz),
            will_set_contents_of_parameter(outl, &encrypt_update_sz, sizeof(int)),
        );
        expect(
            EVP_EncryptFinal_ex, will_return(1), when(ctx, is_equal_to(&mock_cipher_ctx)),
            will_set_contents_of_parameter(out, EXPECT_CIPHERTXT3, encrypt_final_sz),
            will_set_contents_of_parameter(outl, &encrypt_final_sz, sizeof(int)),
        );
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            _utest_hls_enc_segm__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_equal_to(0)),
            when(is_final, is_equal_to(1)), when(eof_reached, is_equal_to(1)),
            when(out_chunkbytes_sz, is_equal_to(encrypt_total_sz)),
            when(out_chunkbytes, is_equal_to_string(EXPECT_CIPHERTXT2 EXPECT_CIPHERTXT3))
        );
        mock_fp.internal.op.encrypt_segment(&mock_fp);
    }
    HLS__ENCRYPT_SEGMENT_CONTINUE__TEARDOWN
#undef RD_BUF_MAX_SZ
#undef SEGM_CONTENT_CHUNK1
#undef SEGM_CONTENT_CHUNK2
#undef EXPECT_CIPHERTXT1
#undef EXPECT_CIPHERTXT2
#undef EXPECT_CIPHERTXT3
} // end of  atfp_hls_test__enc_seg__continue_ok

Ensure(atfp_hls_test__enc_seg__continue_storage_error) {
#define RD_BUF_MAX_SZ 100
    HLS__ENCRYPT_SEGMENT_CONTINUE__SETUP
    ASA_RES_CODE expect_evt_result = ASTORAGE_RESULT_OS_ERROR;
    size_t       chunk_sz = 0;
    expect(EVP_CIPHER_CTX_block_size, will_return(16));
    expect(
        utest_storage_fn_read, will_set_contents_of_parameter(nread_p, &chunk_sz, sizeof(size_t)),
        will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
    );
    expect(
        _utest_hls_enc_segm__common_done_cb, when(err_cnt, is_greater_than(0)),
        when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(0)),
        when(out_chunkbytes_sz, is_equal_to(0)),
    );
    mock_fp.internal.op.encrypt_segment(&mock_fp);
    assert_that(json_object_get(mock_err_info, "storage"), is_not_null);
    HLS__ENCRYPT_SEGMENT_CONTINUE__TEARDOWN
#undef RD_BUF_MAX_SZ
} // end of  atfp_hls_test__enc_seg__continue_storage_error

Ensure(atfp_hls_test__enc_seg__continue_crypto_error) {
#define SEGM_CONTENT_CHUNK PLAINTXT_BLOCK1 PLAINTXT_BLOCK2 PLAINTXT_BLOCK3
#define RD_BUF_MAX_SZ      sizeof(SEGM_CONTENT_CHUNK) + 3
    HLS__ENCRYPT_SEGMENT_CONTINUE__SETUP
    ASA_RES_CODE expect_evt_result = ASTORAGE_RESULT_COMPLETE;
    {
        size_t chunk_sz = sizeof(char) * (sizeof(SEGM_CONTENT_CHUNK) - 1);
        size_t encrypt_update_sz = chunk_sz - (chunk_sz % 16);
        expect(EVP_CIPHER_CTX_block_size, will_return(16));
        expect(
            utest_storage_fn_read,
            will_set_contents_of_parameter(rd_data_p, SEGM_CONTENT_CHUNK, encrypt_update_sz),
            will_set_contents_of_parameter(nread_p, &encrypt_update_sz, sizeof(size_t)),
            will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
        );
        int success = 0;
        expect(
            EVP_EncryptUpdate, will_return(success), when(ctx, is_equal_to(&mock_cipher_ctx)),
            when(in, is_equal_to_string(SEGM_CONTENT_CHUNK)), when(inl, is_equal_to(encrypt_update_sz)),
        );
        expect(
            _utest_hls_enc_segm__common_done_cb, when(err_cnt, is_greater_than(0)),
            when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(0)),
            when(out_chunkbytes_sz, is_equal_to(0)), when(out_chunkbytes, is_not_null),
        );
        mock_fp.internal.op.encrypt_segment(&mock_fp);
    }
    assert_that(json_object_get(mock_err_info, "storage"), is_not_null);
    HLS__ENCRYPT_SEGMENT_CONTINUE__TEARDOWN
#undef RD_BUF_MAX_SZ
#undef SEGM_CONTENT_CHUNK
} // end of  atfp_hls_test__enc_seg__continue_crypto_error

#undef PLAINTXT_BLOCK1
#undef PLAINTXT_BLOCK2
#undef PLAINTXT_BLOCK3
#undef PLAINTXT_BLOCK4
#undef PLAINTXT_BLOCK5
#undef PLAINTXT_BLOCK6

#undef CIPHERTXT_BLOCK1
#undef CIPHERTXT_BLOCK2
#undef CIPHERTXT_BLOCK3
#undef CIPHERTXT_BLOCK4
#undef CIPHERTXT_BLOCK5
#undef CIPHERTXT_BLOCK6

TestSuite *app_transcoder_hls_stream_encrypt_segment_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__enc_seg__start_ok);
    add_test(suite, atfp_hls_test__enc_seg__start_key_error);
    add_test(suite, atfp_hls_test__enc_seg__start_crypto_error);
    add_test(suite, atfp_hls_test__enc_seg__continue_ok);
    add_test(suite, atfp_hls_test__enc_seg__continue_storage_error);
    add_test(suite, atfp_hls_test__enc_seg__continue_crypto_error);
    return suite;
} // end of app_transcoder_hls_stream_encrypt_segment_tests
