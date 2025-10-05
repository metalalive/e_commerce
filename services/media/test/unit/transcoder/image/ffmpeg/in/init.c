#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

extern const atfp_ops_entry_t atfp_ops_image_ffmpg_in;

#define UTEST_FILE_BASEPATH           "tmp/utest"
#define UTEST_ASALOCAL_BASEPATH       UTEST_FILE_BASEPATH "/asalocal"
#define UTEST_LOCAL_BUF_FNAME_POSTFIX "a456-78bc90-19ad1e4f"
#define UTEST_LOCAL_TMPBUF \
    UTEST_ASALOCAL_BASEPATH "/local_buffer" \
                            "-" UTEST_LOCAL_BUF_FNAME_POSTFIX

#define DONE_FLAG_INDEX__IN_ASA_USRARG (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ             (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)

#define DEINIT_IF_EXISTS(var, fn_label) \
    if (var) { \
        fn_label((void *)var); \
        (var) = NULL; \
    }

static ASA_RES_CODE utest_storage_remote_close(asa_op_base_cfg_t *_asaobj) {
    return (ASA_RES_CODE)mock(_asaobj);
}

static void utest_img__asasrc_final_dealloc(asa_op_base_cfg_t *asaobj) {
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_set_source(_map, NULL);
    atfp_asa_map_deinit(_map);
    mock(asaobj);
}

static void utest_img__asalocal_final_dealloc(asa_op_base_cfg_t *asaobj) {
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_set_localtmp(_map, NULL);
    DEINIT_IF_EXISTS(asaobj->op.open.dst_path, free);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.origin, free);
    mock(asaobj);
}

static ASA_RES_CODE utest_img__preload_from_storage(atfp_img_t *imgproc, void (*cb)(atfp_img_t *)) {
    ASA_RES_CODE result = (ASA_RES_CODE)mock(imgproc);
    if (result == ASTORAGE_RESULT_ACCEPT)
        cb(imgproc);
    return result;
}

static void utest_img__av_init(atfp_av_ctx_t *_avctx, const char *filepath, json_t *err_info) {
    int err = (int)mock(_avctx, filepath, err_info);
    if (err)
        json_object_set_new(err_info, "utest", json_string("assume error happened"));
}

static void utest_img__av_deinit(atfp_av_ctx_t *_avctx) { mock(_avctx); }

static void utest_atfp_usr_cb(atfp_t *processor) {
    json_t *err_info = NULL;
    int     num_err_items = 0;
    if (processor)
        err_info = processor->data.error;
    if (err_info)
        num_err_items = json_object_size(err_info);
    mock(processor, num_err_items);
    if (!processor)
        return;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    if (asa_src && asa_src->cb_args.entries) {
        uint8_t *done_flag = asa_src->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if (done_flag && *done_flag == 0)
            *done_flag = 1;
    }
} // end of utest_atfp_usr_cb

#define UTEST_ATFP_IMG__INIT_SETUP \
    uv_loop_t  *loop = uv_default_loop(); \
    uint8_t     done_flag = 0; \
    const char *local_buf_fname_postfix = UTEST_LOCAL_BUF_FNAME_POSTFIX; \
    void       *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    void       *asalocal_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_cfg_t   mock_storage_local_cfg = { \
          .ops = \
            {.fn_open = app_storage_localfs_open, \
               .fn_close = app_storage_localfs_close, \
               .fn_unlink = app_storage_localfs_unlink} \
    }; \
    asa_cfg_t mock_storage_remote_cfg = \
        {.ops = { \
             .fn_close = utest_storage_remote_close, \
         }}; \
    asa_op_base_cfg_t mock_asa_src = { \
        .cb_args = {.size = NUM_CB_ARGS_ASAOBJ, .entries = asasrc_cb_args}, \
        .storage = &mock_storage_remote_cfg, \
        .deinit = utest_img__asasrc_final_dealloc \
    }; \
    asa_op_localfs_cfg_t mock_asa_local = \
        {.loop = loop, \
         .file = {.file = -1}, \
         .super = { \
             .storage = &mock_storage_local_cfg, \
             .deinit = utest_img__asalocal_final_dealloc, \
             .cb_args = {.size = NUM_CB_ARGS_ASAOBJ, .entries = asalocal_cb_args}, \
             .op = {.mkdir = {.path = {.origin = strdup(UTEST_ASALOCAL_BASEPATH)}}}, \
         }}; \
    json_t        *mock_errinfo = json_object(); \
    arpc_receipt_t mock_rpc_receipt = {0}; \
    atfp_img_t    *mock_fp = (atfp_img_t *)atfp_ops_image_ffmpg_in.ops.instantiate(); \
    mock_fp->ops.src.preload_from_storage = utest_img__preload_from_storage; \
    mock_fp->ops.src.avctx_init = utest_img__av_init; \
    mock_fp->ops.src.avctx_deinit = utest_img__av_deinit; \
    mock_fp->super.data = (atfp_data_t \
    ){.error = mock_errinfo, \
      .rpc_receipt = &mock_rpc_receipt, \
      .callback = utest_atfp_usr_cb, \
      .storage = {.handle = &mock_asa_src}}; \
    atfp_asa_map_t *mock_map = atfp_asa_map_init(1); \
    atfp_asa_map_set_source(mock_map, &mock_asa_src); \
    atfp_asa_map_set_localtmp(mock_map, &mock_asa_local); \
    asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG] = mock_fp; \
    asasrc_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \
    asasrc_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    asalocal_cb_args[ATFP_INDEX__IN_ASA_USRARG] = mock_fp; \
    asalocal_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \
    asalocal_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    mkdir(UTEST_FILE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU);

#define UTEST_ATFP_IMG__INIT_TEARDOWN \
    DEINIT_IF_EXISTS(mock_errinfo, json_decref); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH);

Ensure(atfp_img_ffi_test__init_ok
){UTEST_ATFP_IMG__INIT_SETUP{// init
                             expect(uuid_generate_random, when(uuo, is_not_null));
expect(
    uuid_unparse, when(uuo, is_not_null),
    will_set_contents_of_parameter(
        out, local_buf_fname_postfix, sizeof(char) * strlen(local_buf_fname_postfix)
    )
);
atfp__image_ffm_in__init_transcode(&mock_fp->super);
expect(utest_img__preload_from_storage, will_return(ASTORAGE_RESULT_ACCEPT));
expect(utest_img__av_init, will_return(0));
expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)), when(num_err_items, is_equal_to(0)));
while (!done_flag)
    uv_run(loop, UV_RUN_ONCE);
assert_that(mock_asa_local.file.file, is_greater_than(-1));
assert_that(access(UTEST_LOCAL_TMPBUF, F_OK), is_equal_to(0));
}
{ // de-init
    expect(utest_img__av_deinit);
    expect(
        utest_storage_remote_close, will_return(ASTORAGE_RESULT_UNKNOWN_ERROR),
        when(_asaobj, is_equal_to(&mock_asa_src))
    );
    atfp__image_ffm_in__deinit_transcode(&mock_fp->super);
    expect(utest_img__asalocal_final_dealloc, when(asaobj, is_equal_to(&mock_asa_local)));
    expect(utest_img__asasrc_final_dealloc, when(asaobj, is_equal_to(&mock_asa_src)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(NULL)));
    while (access(UTEST_LOCAL_TMPBUF, F_OK) == 0)
        uv_run(loop, UV_RUN_ONCE);
}
UTEST_ATFP_IMG__INIT_TEARDOWN
} // end of  atfp_img_ffi_test__init_ok

Ensure(atfp_img_ffi_test__preload_error
){UTEST_ATFP_IMG__INIT_SETUP{// init
                             expect(uuid_generate_random, when(uuo, is_not_null));
expect(
    uuid_unparse, when(uuo, is_not_null),
    will_set_contents_of_parameter(
        out, local_buf_fname_postfix, sizeof(char) * strlen(local_buf_fname_postfix)
    )
);
atfp__image_ffm_in__init_transcode(&mock_fp->super);
expect(utest_img__preload_from_storage, will_return(ASTORAGE_RESULT_OS_ERROR));
expect(
    utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)), when(num_err_items, is_greater_than(0))
);
while (!done_flag)
    uv_run(loop, UV_RUN_ONCE);
assert_that(mock_asa_local.file.file, is_greater_than(-1));
assert_that(access(UTEST_LOCAL_TMPBUF, F_OK), is_equal_to(0));
}
{ // de-init
    expect(utest_img__av_deinit);
    expect(
        utest_storage_remote_close, will_return(ASTORAGE_RESULT_UNKNOWN_ERROR),
        when(_asaobj, is_equal_to(&mock_asa_src))
    );
    atfp__image_ffm_in__deinit_transcode(&mock_fp->super);
    expect(utest_img__asalocal_final_dealloc, when(asaobj, is_equal_to(&mock_asa_local)));
    expect(utest_img__asasrc_final_dealloc, when(asaobj, is_equal_to(&mock_asa_src)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(NULL)));
    while (access(UTEST_LOCAL_TMPBUF, F_OK) == 0)
        uv_run(loop, UV_RUN_ONCE);
}
UTEST_ATFP_IMG__INIT_TEARDOWN
} // end of  atfp_img_ffi_test__preload_error

Ensure(atfp_img_ffi_test__avctx_error) {
    UTEST_ATFP_IMG__INIT_SETUP
    int expect_err = 1;
    { // init
        expect(uuid_generate_random, when(uuo, is_not_null));
        expect(
            uuid_unparse, when(uuo, is_not_null),
            will_set_contents_of_parameter(
                out, local_buf_fname_postfix, sizeof(char) * strlen(local_buf_fname_postfix)
            )
        );
        atfp__image_ffm_in__init_transcode(&mock_fp->super);
        expect(utest_img__preload_from_storage, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_img__av_init, will_return(expect_err));
        expect(
            utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)),
            when(num_err_items, is_greater_than(0))
        );
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_asa_local.file.file, is_greater_than(-1));
        assert_that(access(UTEST_LOCAL_TMPBUF, F_OK), is_equal_to(0));
    }
    { // de-init
        expect(utest_img__av_deinit);
        expect(
            utest_storage_remote_close, will_return(ASTORAGE_RESULT_UNKNOWN_ERROR),
            when(_asaobj, is_equal_to(&mock_asa_src))
        );
        atfp__image_ffm_in__deinit_transcode(&mock_fp->super);
        expect(utest_img__asalocal_final_dealloc, when(asaobj, is_equal_to(&mock_asa_local)));
        expect(utest_img__asasrc_final_dealloc, when(asaobj, is_equal_to(&mock_asa_src)));
        expect(utest_atfp_usr_cb, when(processor, is_equal_to(NULL)));
        while (access(UTEST_LOCAL_TMPBUF, F_OK) == 0)
            uv_run(loop, UV_RUN_ONCE);
    }
    UTEST_ATFP_IMG__INIT_TEARDOWN
} // end of  atfp_img_ffi_test__avctx_error

static int utest_img__av_decode_pkt(atfp_av_ctx_t *_avctx) { return (int)mock(_avctx); }

static int utest_img__av_fetch_nxt_pkt(atfp_av_ctx_t *_avctx) { return (int)mock(_avctx); }

#define UTEST_ATFP_IMG__PROCESS_SETUP \
    json_t       *mock_err_info = json_object(); \
    atfp_av_ctx_t mock_avctx = {0}; \
    atfp_img_t    mock_fp = { \
           .super = {.data = {.error = mock_err_info, .callback = utest_atfp_usr_cb}}, \
           .ops = {.src = {.decode_pkt = utest_img__av_decode_pkt, .next_pkt = utest_img__av_fetch_nxt_pkt}}, \
           .av = &mock_avctx, \
    };

#define UTEST_ATFP_IMG__PROCESS_TEARDOWN json_decref(mock_err_info);

Ensure(atfp_img_ffi_test__process_decode_pkt_ok) {
    UTEST_ATFP_IMG__PROCESS_SETUP
    expect(utest_img__av_decode_pkt, will_return(0), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp.super)));
    atfp__image_ffm_in__proceeding_transcode(&mock_fp.super);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    UTEST_ATFP_IMG__PROCESS_TEARDOWN
} // end of atfp_img_ffi_test__process_decode_pkt_ok

Ensure(atfp_img_ffi_test__process_grab_nxt_pkt_ok) {
    UTEST_ATFP_IMG__PROCESS_SETUP
    expect(utest_img__av_decode_pkt, will_return(1), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_img__av_fetch_nxt_pkt, will_return(0), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_img__av_decode_pkt, will_return(0), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp.super)));
    atfp__image_ffm_in__proceeding_transcode(&mock_fp.super);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    UTEST_ATFP_IMG__PROCESS_TEARDOWN
} // end of atfp_img_ffi_test__process_grab_nxt_pkt_ok

Ensure(atfp_img_ffi_test__process_eof_reached) {
    UTEST_ATFP_IMG__PROCESS_SETUP
    uint8_t end_of_file_flg = 1;
    expect(utest_img__av_decode_pkt, will_return(1), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_img__av_fetch_nxt_pkt, will_return(end_of_file_flg), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp.super)));
    atfp__image_ffm_in__proceeding_transcode(&mock_fp.super);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    UTEST_ATFP_IMG__PROCESS_TEARDOWN
} // end of atfp_img_ffi_test__process_eof_reached

Ensure(atfp_img_ffi_test__process_decode_error) {
    UTEST_ATFP_IMG__PROCESS_SETUP
    expect(utest_img__av_decode_pkt, will_return(1), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_img__av_fetch_nxt_pkt, will_return(0), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_img__av_decode_pkt, will_return(AVERROR(EBADF)), when(_avctx, is_equal_to(&mock_avctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp.super)));
    atfp__image_ffm_in__proceeding_transcode(&mock_fp.super);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    UTEST_ATFP_IMG__PROCESS_TEARDOWN
} // end of atfp_img_ffi_test__process_decode_error

TestSuite *app_transcoder_img_ffm_in_init_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffi_test__init_ok);
    add_test(suite, atfp_img_ffi_test__preload_error);
    add_test(suite, atfp_img_ffi_test__avctx_error);
    add_test(suite, atfp_img_ffi_test__process_decode_pkt_ok);
    add_test(suite, atfp_img_ffi_test__process_grab_nxt_pkt_ok);
    add_test(suite, atfp_img_ffi_test__process_eof_reached);
    add_test(suite, atfp_img_ffi_test__process_decode_error);
    return suite;
}
