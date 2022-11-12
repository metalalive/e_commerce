#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"
#include "../test/unit/transcoder/test.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASALOCAL_BASEPATH    UTEST_FILE_BASEPATH "/asalocal"

#define  DONE_FLAG_INDEX__IN_ASA_USRARG    (ATFP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ                (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define  MOCK_STORAGE_ALIAS    "localfs"

#define  MOCK_USER_ID           233
#define  MOCK_UPLD_REQ_1_ID     0x150de9a6



static  void _utest_hls_cryptokey_req__common_cb (atfp_t *processor)
{
    json_t  *err_info = processor->data.error;
    size_t  err_cnt = json_object_size(err_info);
    char    * out_chunkbytes = processor->transfer.streaming_dst.block.data;
    size_t    out_chunkbytes_sz = processor->transfer.streaming_dst.block.len;
    uint8_t   is_final = processor->transfer.streaming_dst.flags.is_final;
    mock(err_cnt, out_chunkbytes, out_chunkbytes_sz, is_final);
    asa_op_base_cfg_t  *_asa_local = & ((atfp_hls_t *)processor) ->asa_local .super;
    if(_asa_local->cb_args.entries) {
        uint8_t  *done_flg_p = _asa_local->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flg_p)
            *done_flg_p = 1;
    }
} // end of _utest_hls_cryptokey_req__common_cb

#define  HLS__CRYPTO_KEY_ACQUIRE__SETUP \
    uint8_t mock_done_flag = 0 ; \
    void  *mock_asalocal_cb_args [NUM_CB_ARGS_ASAOBJ] = {NULL, &mock_done_flag}; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(); \
    json_t *mock_doc_metadata = json_object(); \
    json_t *mock_err_info = json_object(); \
    asa_cfg_t  mock_storage_cfg = {.alias=MOCK_STORAGE_ALIAS, .base_path=UTEST_ASALOCAL_BASEPATH, \
        .ops={.fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close}}; \
    app_cfg_t *mock_appcfg = app_get_global_cfg(); \
    mock_appcfg->storages.size = 1; \
    mock_appcfg->storages.capacity = 1; \
    mock_appcfg->storages.entries = &mock_storage_cfg; \
    mock_appcfg->tmp_buf.path = UTEST_ASALOCAL_BASEPATH; \
    atfp_hls_t  *mock_fp = (atfp_hls_t  *) atfp__video_hls__instantiate_stream(); \
    { \
        mock_fp->super.data = (atfp_data_t){.callback=_utest_hls_cryptokey_req__common_cb, .spec=mock_spec, \
            .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID}; \
        mock_fp->asa_local.super.cb_args.entries = mock_asalocal_cb_args; \
        mock_fp->asa_local.super.cb_args.size = NUM_CB_ARGS_ASAOBJ; \
    } \
    json_object_set_new(mock_spec, "loop", json_integer((uint64_t)loop)); \
    json_object_set_new(mock_spec, "metadata", mock_doc_metadata); \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            NULL, UTEST_OPS_MKDIR); \


#define  HLS__CRYPTO_KEY_ACQUIRE__TEARDOWN \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,  0, NULL, UTEST_OPS_RMDIR); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH); \
    mock_appcfg->storages.size = 0; \
    mock_appcfg->storages.capacity = 0; \
    mock_appcfg->storages.entries = NULL; \
    mock_appcfg->tmp_buf.path =  NULL; \
    json_decref(mock_spec); \
    json_decref(mock_err_info); \
    mock_fp->asa_local.super.deinit(&mock_fp->asa_local.super) ;  


Ensure(atfp_hls_test__key_req__ok) {
#define   UTEST__CRYPTOKEY_CHOSEN_ID    "8134EADF"
#define   UTEST__EXPECT_kEY_OCTET   "\x5D\x4A\xF8\x33\x17\x51\xA3\x09"
#define   UTEST__EXPECT_kEY_SZ      (sizeof(UTEST__EXPECT_kEY_OCTET) - 1)
#define   UTEST__CRYPTOKEY_MIN_CONTENT  \
    "{\"73724A57\":{\"key\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \""UTEST__CRYPTOKEY_CHOSEN_ID"\":{\"key\":{\"nbytes\":8,\"data\":\"5D4AF8331751A309\"},\"alg\":\"aes\"}}"
    HLS__CRYPTO_KEY_ACQUIRE__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID)); \
    { // create (local) crypto keyfile
        const char *_wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        size_t _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_WRITE2FILE);
    }
    assert_that(mock_fp->internal.op.acquire_key, is_equal_to(atfp_hls_stream__acquire_key));
    mock_fp->internal.op. acquire_key(mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_cryptokey_req__common_cb, when(err_cnt, is_equal_to(0)),
                when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_equal_to(0)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_fp->internal.op.acquire_key, is_equal_to(atfp_hls_stream__acquire_key__final));
    }
    expect(_utest_hls_cryptokey_req__common_cb, when(err_cnt, is_equal_to(0)),
             when(is_final, is_equal_to(1)), when(out_chunkbytes_sz, is_equal_to(UTEST__EXPECT_kEY_SZ)),
             when(out_chunkbytes,  is_equal_to_string(UTEST__EXPECT_kEY_OCTET)));
    mock_fp->internal.op.acquire_key(mock_fp);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    HLS__CRYPTO_KEY_ACQUIRE__TEARDOWN
#undef  UTEST__CRYPTOKEY_MIN_CONTENT
#undef  UTEST__CRYPTOKEY_CHOSEN_ID
#undef  UTEST__EXPECT_kEY_OCTET
#undef  UTEST__EXPECT_kEY_SZ
} // end of atfp_hls_test__key_req__ok


Ensure(atfp_hls_test__key_req__missing_file) {
    HLS__CRYPTO_KEY_ACQUIRE__SETUP
    mock_fp->internal.op. acquire_key(mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_cryptokey_req__common_cb, when(err_cnt, is_greater_than(0)),
                when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_equal_to(0)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_get(mock_err_info, "storage"), is_not_null);
    }
    HLS__CRYPTO_KEY_ACQUIRE__TEARDOWN
} // end of atfp_hls_test__key_req__missing_file


Ensure(atfp_hls_test__key_req__missing_item) {
#define   UTEST__CRYPTOKEY_CHOSEN_ID    "8134EADF"
#define   UTEST__CRYPTOKEY_MIN_CONTENT  \
    "{\"73724A57\":{\"key\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \"dee71ba8\":{\"key\":{\"nbytes\":8,\"data\":\"5D4AF8331751A309\"},\"alg\":\"aes\"}}"
    HLS__CRYPTO_KEY_ACQUIRE__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID)); \
    { // create (local) crypto keyfile
        const char *_wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        size_t _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_WRITE2FILE);
    }
    mock_fp->internal.op. acquire_key(mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_cryptokey_req__common_cb, when(err_cnt, is_greater_than(0)),
                when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_equal_to(0)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_get(mock_err_info, "transcoder"), is_not_null);
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    HLS__CRYPTO_KEY_ACQUIRE__TEARDOWN
#undef  UTEST__CRYPTOKEY_MIN_CONTENT
#undef  UTEST__CRYPTOKEY_CHOSEN_ID
} // end of atfp_hls_test__key_req__missing_item


TestSuite *app_transcoder_hls_stream_cryptokey_request_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__key_req__ok);
    add_test(suite, atfp_hls_test__key_req__missing_file);
    add_test(suite, atfp_hls_test__key_req__missing_item);
    return suite;
}
