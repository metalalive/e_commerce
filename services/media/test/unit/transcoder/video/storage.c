#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "storage/localfs.h"
#include "transcoder/video/common.h"

#define UTEST_STRINGIFY(x) #x

#define UTEST_FILE_BASEPATH     "tmp/utest"
#define UTEST_ASALOCAL_BASEPATH UTEST_FILE_BASEPATH "/asalocal"
#define UTEST_ASADST_BASEPATH   UTEST_FILE_BASEPATH "/asadst"

#define DONE_FLAG_INDEX__IN_ASA_USRARG (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ             (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define WR_BUF_MAX_SZ                  10

static void utest_atfp_done_usr_cb(atfp_t *processor) {
    mock(processor);
    if (!processor)
        return;
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    if (asa_dst && asa_dst->cb_args.entries) {
        uint8_t *done_flag = asa_dst->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if (done_flag)
            *done_flag = 1;
    }
} // end of utest_atfp_done_usr_cb

#define ATFP_HLS_TEST__RMFILE__SETUP \
    void      *asa_dst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    uint8_t    done_flag = 0; \
    uv_loop_t *loop = uv_default_loop(); \
    asa_cfg_t  mock_storage_cfg = \
        {.base_path = UTEST_FILE_BASEPATH, \
         .ops = { \
             .fn_scandir = app_storage_localfs_scandir, \
             .fn_scandir_next = app_storage_localfs_scandir_next, \
             .fn_unlink = app_storage_localfs_unlink, \
             .fn_rmdir = app_storage_localfs_rmdir, \
         }}; \
    asa_op_localfs_cfg_t mock_asa_dst = { \
        .loop = loop, \
        .super = \
            {.storage = &mock_storage_cfg, \
             .cb_args = {.size = NUM_CB_ARGS_ASAOBJ, .entries = asa_dst_cb_args}} \
    }; \
    json_t *mock_err_info = json_object(); \
    atfp_t  mock_fp = \
        {.data = { \
             .callback = utest_atfp_done_usr_cb, \
             .error = mock_err_info, \
             .storage = {.handle = &mock_asa_dst.super}, \
             .version = UTEST_VERSION, \
             .usr_id = UTEST_USER_ID, \
             .upld_req_id = UTEST_UPLOAD_REQ_ID, \
         }}; \
    asa_dst_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    asa_dst_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    mkdir(UTEST_FILE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR, S_IRWXU); \
    mkdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR, S_IRWXU); \
    mkdir( \
        UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS, \
        S_IRWXU \
    ); \
    mkdir(UTEST_TARGET_PATH, S_IRWXU); \
    int fd = open(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    close(fd); \
    fd = open(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    close(fd);

#define ATFP_HLS_TEST__RMFILE__TEARDOWN \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS \
    ); \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR); \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR); \
    rmdir(UTEST_FILE_BASEPATH); \
    json_decref(mock_err_info);

#define UTEST_USER_ID            426
#define UTEST_UPLOAD_REQ_ID      0x12345678
#define UTEST_VERSION            "Nh"
#define UTEST_TRANSCODE_STATUS   ATFP__DISCARDING_FOLDER_NAME
#define UTEST_USER_ID__STR       UTEST_STRINGIFY(426)
#define UTEST_UPLOAD_REQ_ID__STR UTEST_STRINGIFY(12345678)
#define UTEST_TARGET_PATH \
    UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS \
                        "/" UTEST_VERSION
#define UTEST_NUM_FILES   2
#define UTEST_FILE_NAME_1 "segment_abc"
#define UTEST_FILE_NAME_2 "segment_xyz"
Ensure(atfp_video_test__remove_version_ok) {
    ATFP_HLS_TEST__RMFILE__SETUP;
    {
        atfp_storage_video_remove_version(&mock_fp, UTEST_TRANSCODE_STATUS);
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.size, is_equal_to(UTEST_NUM_FILES));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(access(UTEST_TARGET_PATH, F_OK), is_equal_to(-1));
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    ATFP_HLS_TEST__RMFILE__TEARDOWN;
} // end of atfp_video_test__remove_version_ok

Ensure(atfp_video_test__remove_version_missing_in_middle) {
    ATFP_HLS_TEST__RMFILE__SETUP;
    {
        atfp_storage_video_remove_version(&mock_fp, UTEST_TRANSCODE_STATUS);
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.size, is_equal_to(UTEST_NUM_FILES));
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.rd_idx, is_less_than(UTEST_NUM_FILES));
        unlink(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1);
        unlink(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2);
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(access(UTEST_TARGET_PATH, F_OK), is_equal_to(0));
        assert_that(json_object_size(mock_err_info), is_greater_than(0));
        rmdir(UTEST_TARGET_PATH);
    }
    ATFP_HLS_TEST__RMFILE__TEARDOWN;
} // end of atfp_video_test__remove_version_missing_in_middle
#undef UTEST_NUM_FILES
#undef UTEST_FILE_NAME_1
#undef UTEST_FILE_NAME_2
#undef UTEST_TARGET_PATH
#undef UTEST_UPLOAD_REQ_ID__STR
#undef UTEST_USER_ID__STR
#undef UTEST_USER_ID
#undef UTEST_UPLOAD_REQ_ID
#undef UTEST_VERSION
#undef UTEST_TRANSCODE_STATUS

TestSuite *app_transcoder_video_storage_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_video_test__remove_version_ok);
    add_test(suite, atfp_video_test__remove_version_missing_in_middle);
    return suite;
}
