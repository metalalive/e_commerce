#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "utils.h"
#include "storage/localfs.h"
#include "transcoder/video/common.h"

#define UTEST_STRINGIFY(x) #x

#define RUNNER_CREATE_FOLDER(fullpath) mkdir(fullpath, S_IRWXU)
#define RUNNER_ACCESS_F_OK(fullpath)   access(fullpath, F_OK)

#define RUNNER_OPEN_WRONLY_CREATE_USR(fullpath) open(fullpath, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR)

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
    void         *asa_dst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    uint8_t       done_flag = 0; \
    uv_loop_t    *loop = uv_default_loop(); \
    app_envvars_t env = {0}; \
    app_load_envvars(&env); \
    const char *sys_basepath = env.sys_base_path; \
    asa_cfg_t   mock_storage_cfg = \
        {.base_path = PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, strdup), \
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
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN( \
        sys_basepath, UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR, \
        RUNNER_CREATE_FOLDER \
    ); \
    PATH_CONCAT_THEN_RUN( \
        sys_basepath, \
        UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS, \
        RUNNER_CREATE_FOLDER \
    ); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH, RUNNER_CREATE_FOLDER); \
    int fd = PATH_CONCAT_THEN_RUN( \
        sys_basepath, UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1, RUNNER_OPEN_WRONLY_CREATE_USR \
    ); \
    close(fd); \
    fd = PATH_CONCAT_THEN_RUN( \
        sys_basepath, UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2, RUNNER_OPEN_WRONLY_CREATE_USR \
    ); \
    close(fd);

#define ATFP_HLS_TEST__RMFILE__TEARDOWN \
    PATH_CONCAT_THEN_RUN( \
        sys_basepath, \
        UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS, \
        rmdir \
    ); \
    PATH_CONCAT_THEN_RUN( \
        sys_basepath, UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR, rmdir \
    ); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, rmdir); \
    free(mock_storage_cfg.base_path); \
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
    atfp_storage_video_remove_version(&mock_fp, UTEST_TRANSCODE_STATUS);
    expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(mock_asa_dst.super.op.scandir.fileinfo.size, is_equal_to(UTEST_NUM_FILES));
    while (!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH, RUNNER_ACCESS_F_OK), is_equal_to(-1));
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    ATFP_HLS_TEST__RMFILE__TEARDOWN;
} // end of atfp_video_test__remove_version_ok

Ensure(atfp_video_test__remove_version_missing_in_middle) {
    ATFP_HLS_TEST__RMFILE__SETUP;
    atfp_storage_video_remove_version(&mock_fp, UTEST_TRANSCODE_STATUS);
    expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(mock_asa_dst.super.op.scandir.fileinfo.size, is_equal_to(UTEST_NUM_FILES));
    assert_that(mock_asa_dst.super.op.scandir.fileinfo.rd_idx, is_less_than(UTEST_NUM_FILES));
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1, unlink);
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2, unlink);
    while (!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH, RUNNER_ACCESS_F_OK), is_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_TARGET_PATH, rmdir);
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
