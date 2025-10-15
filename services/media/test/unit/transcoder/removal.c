#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include "app_cfg.h"
#include "utils.h"
#include <uv.h>

#include "storage/localfs.h"
#include "transcoder/file_processor.h"

#define UTEST_STRINGIFY(x)          #x
#define UTEST_USER_ID               518
#define UTEST_USER_ID_STR           UTEST_STRINGIFY(518)
#define UTEST_UPLD_REQ_ID           0xab19b055
#define UTEST_UPLD_REQ_ID_STR       UTEST_STRINGIFY(ab19b055)
#define UTEST_FILE_BASEPATH         "tmp/utest"
#define UTEST_USERFILE_BASEPATH     UTEST_FILE_BASEPATH "/" UTEST_USER_ID_STR
#define UTEST_RESOURCE_BASEPATH     UTEST_USERFILE_BASEPATH "/" UTEST_UPLD_REQ_ID_STR
#define UTEST_RESC_TRANSCODING_PATH UTEST_RESOURCE_BASEPATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME
#define UTEST_RESC_DISCARDING_PATH  UTEST_RESOURCE_BASEPATH "/" ATFP__DISCARDING_FOLDER_NAME
#define UTEST_RESC_COMMITTED_PATH   UTEST_RESOURCE_BASEPATH "/" ATFP__COMMITTED_FOLDER_NAME

#define UT_RESC_TRANSCODING_LOCALPATH \
    UTEST_USER_ID_STR "/" UTEST_UPLD_REQ_ID_STR "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME
#define UT_RESC_DISCARDING_LOCALPATH \
    UTEST_USER_ID_STR "/" UTEST_UPLD_REQ_ID_STR "/" ATFP__DISCARDING_FOLDER_NAME
#define UT_RESC_COMMITTED_LOCALPATH \
    UTEST_USER_ID_STR "/" UTEST_UPLD_REQ_ID_STR "/" ATFP__COMMITTED_FOLDER_NAME

#define DONE_FLAG_INDEX__IN_ASA_USRARG (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ             (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define WR_BUF_MAX_SZ                  0

#define RUNNER_CREATE_FOLDER(fullpath) mkdir(fullpath, S_IRWXU)
#define RUNNER_OPEN_CREATE(fullpath)   open(fullpath, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR)
#define RUNNER_ACCESS_F_OK(fullpath)   access(fullpath, F_OK)

static ASA_RES_CODE utest_storage_rmdir(asa_op_base_cfg_t *asaobj) {
    const char *path = asaobj->op.rmdir.path;
    int         err = mock(asaobj, path);
    return err ? ASTORAGE_RESULT_UNKNOWN_ERROR : app_storage_localfs_rmdir(asaobj);
}

static ASA_RES_CODE utest_storage_scandir(asa_op_base_cfg_t *asaobj) {
    const char *path = asaobj->op.scandir.path;
    int         err = mock(asaobj, path);
    return err ? ASTORAGE_RESULT_UNKNOWN_ERROR : app_storage_localfs_scandir(asaobj);
}

static ASA_RES_CODE utest_storage_scandir_next(asa_op_base_cfg_t *asaobj, asa_dirent_t *ent) {
    int err = mock(asaobj);
    return err ? ASTORAGE_RESULT_UNKNOWN_ERROR : app_storage_localfs_scandir_next(asaobj, ent);
}

static ASA_RES_CODE utest_storage_unlink(asa_op_base_cfg_t *asaobj) {
    const char *path = asaobj->op.unlink.path;
    int         err = mock(asaobj, path);
    return err ? ASTORAGE_RESULT_UNKNOWN_ERROR : app_storage_localfs_unlink(asaobj);
}

static void _utest_remove_version_unlinkfile_done(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    processor->data.callback(processor);
}

static void utest_storage_remove_version(atfp_t *processor, const char *status) {
    json_t     *err_info = processor->data.error;
    uint32_t    _usr_id = processor->data.usr_id;
    uint32_t    _upld_req_id = processor->data.upld_req_id;
    const char *version = processor->data.version;
    int         err = mock(processor, status, _usr_id, _upld_req_id, version);
    if (err) {
        json_object_set_new(err_info, "transcode", json_string("[utest][storage] assertion failure"));
        return;
    }
    size_t localpath_sz = sizeof(UTEST_USER_ID_STR) + 1 + sizeof(UTEST_UPLD_REQ_ID_STR) + 1 + strlen(status) +
                          1 + strlen(version) + 1;
    char   localpath[localpath_sz];
    size_t nwrite = snprintf(
        &localpath[0], localpath_sz, UTEST_USER_ID_STR "/" UTEST_UPLD_REQ_ID_STR "/%s/%s", status, version
    );
    assert(localpath[nwrite] == 0x0); // NULL-terminated
    assert(nwrite <= localpath_sz);
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    asa_remote->op.unlink.path = &localpath[0];
    asa_remote->op.unlink.cb = _utest_remove_version_unlinkfile_done;
    ASA_RES_CODE result = asa_remote->storage->ops.fn_unlink(asa_remote);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            err_info, "transcode",
            json_string("[utest][storage] failed to issue unlink operation for removing files")
        );
    }
} // end of  utest_storage_remove_version

static void utest_atfp_done_usr_cb(atfp_t *processor) {
    mock(processor);
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    uint8_t           *done_flag = asa_remote->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    if (done_flag)
        *done_flag = 1;
} // end of utest_atfp_done_usr_cb

#define ATFP_REMOVAL_TEST_SETUP \
    uint8_t       done_flag = 0; \
    void         *mock_asa_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    uv_loop_t    *loop = uv_default_loop(); \
    json_t       *mock_spec = json_object(), *mock_err_info = json_object(); \
    app_envvars_t env = {0}; \
    app_load_envvars(&env); \
    size_t workfiles_basepath_sz = strlen(env.sys_base_path) + strlen(UTEST_FILE_BASEPATH) + 2; \
    char  *workfiles_basepath = (char *)malloc(workfiles_basepath_sz); \
    snprintf(workfiles_basepath, workfiles_basepath_sz, "%s/%s", env.sys_base_path, UTEST_FILE_BASEPATH); \
    asa_cfg_t mock_storage_cfg = \
        {.base_path = workfiles_basepath, \
         .ops = { \
             .fn_scandir = utest_storage_scandir, \
             .fn_scandir_next = utest_storage_scandir_next, \
             .fn_unlink = utest_storage_unlink, \
             .fn_rmdir = utest_storage_rmdir, \
         }}; \
    asa_op_localfs_cfg_t mock_asa_remote = { \
        .loop = loop, \
        .super = \
            {.storage = &mock_storage_cfg, \
             .cb_args = {.size = NUM_CB_ARGS_ASAOBJ, .entries = mock_asa_cb_args}} \
    }; \
    atfp_t mock_fp = { \
        .data = \
            {.spec = mock_spec, \
             .error = mock_err_info, \
             .storage = {.handle = &mock_asa_remote.super}, \
             .usr_id = UTEST_USER_ID, \
             .upld_req_id = UTEST_UPLD_REQ_ID} \
    }; \
    mock_asa_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    mock_asa_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_FILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_USERFILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESOURCE_BASEPATH, RUNNER_CREATE_FOLDER);

#define ATFP_REMOVAL_TEST_TEARDOWN \
    assert_that(json_object_size(mock_spec), is_equal_to(0)); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESOURCE_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_USERFILE_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_FILE_BASEPATH, rmdir); \
    json_decref(mock_spec); \
    json_decref(mock_err_info); \
    free(mock_storage_cfg.base_path);

Ensure(atfp_removal_test__ok_all_empty) {
    ATFP_REMOVAL_TEST_SETUP
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if (err_cnt == 0) {
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__ok_all_empty

Ensure(atfp_removal_test__ok_all_status_folders) {
    ATFP_REMOVAL_TEST_SETUP
    // assume several transcoded versions were saved in storage
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_TRANSCODING_PATH, RUNNER_CREATE_FOLDER);
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_DISCARDING_PATH, RUNNER_CREATE_FOLDER);
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_CREATE_FOLDER);
    int fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_DISCARDING_PATH "/R12", RUNNER_OPEN_CREATE);
    close(fd);
    fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_DISCARDING_PATH "/rjk", RUNNER_OPEN_CREATE);
    close(fd);
    fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/didi", RUNNER_OPEN_CREATE);
    close(fd);
    fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/zen", RUNNER_OPEN_CREATE);
    close(fd);
    fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/Ga1", RUNNER_OPEN_CREATE);
    close(fd);
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int ret = json_object_size(mock_err_info);
    assert_that(ret, is_equal_to(0));
    if (ret == 0) {
        expect(
            utest_storage_rmdir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
        );
        // -----------------
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__DISCARDING_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__DISCARDING_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_rmdir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        // -----------------
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_rmdir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_TRANSCODING_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_DISCARDING_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
    }
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__ok_all_status_folders

Ensure(atfp_removal_test__ok_one_status_folder) {
    ATFP_REMOVAL_TEST_SETUP { // assume several transcoded versions were saved in storage
        PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_CREATE_FOLDER);
        int fd =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/acid", RUNNER_OPEN_CREATE);
        close(fd);
        fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/asic", RUNNER_OPEN_CREATE);
        close(fd);
    }
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int ret = json_object_size(mock_err_info);
    assert_that(ret, is_equal_to(0));
    if (ret == 0) {
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_rmdir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_TRANSCODING_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_DISCARDING_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(-1));
        assert_that(errno, is_equal_to(ENOENT));
    }
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__ok_one_status_folder

Ensure(atfp_removal_test__err_scan_status_versions) {
    ATFP_REMOVAL_TEST_SETUP
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int ret = json_object_size(mock_err_info), expect_err = 1;
    assert_that(ret, is_equal_to(0));
    if (ret == 0) {
        expect(
            utest_storage_scandir, will_return(expect_err), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_greater_than(0));
    }
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__err_scan_status_versions

Ensure(atfp_removal_test__err_rm_single_version) {
    ATFP_REMOVAL_TEST_SETUP { // assume several transcoded versions were saved in storage
        PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_CREATE_FOLDER);
        int fd =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/acid", RUNNER_OPEN_CREATE);
        close(fd);
        fd = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/asic", RUNNER_OPEN_CREATE);
        close(fd);
    }
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int ret = json_object_size(mock_err_info), expect_err = 1;
    assert_that(ret, is_equal_to(0));
    if (ret == 0) {
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(expect_err), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_greater_than(0));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(0));
    }
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/acid", unlink);
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/asic", unlink);
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, rmdir);
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__err_rm_single_version

Ensure(atfp_removal_test__err_rm_status_folder) {
    ATFP_REMOVAL_TEST_SETUP { // assume several transcoded versions were saved in storage
        PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_CREATE_FOLDER);
        int fd =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH "/acid", RUNNER_OPEN_CREATE);
        close(fd);
    }
    expect(
        utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
        when(path, is_equal_to_string(UT_RESC_TRANSCODING_LOCALPATH))
    );
    atfp_discard_transcoded(&mock_fp, utest_storage_remove_version, utest_atfp_done_usr_cb);
    int ret = json_object_size(mock_err_info), expect_err = 1;
    assert_that(ret, is_equal_to(0));
    if (ret == 0) {
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_DISCARDING_LOCALPATH))
        );
        expect(
            utest_storage_scandir, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(utest_storage_scandir_next, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_remove_version, will_return(0), when(processor, is_equal_to(&mock_fp)),
            when(_usr_id, is_equal_to(UTEST_USER_ID)), when(_upld_req_id, is_equal_to(UTEST_UPLD_REQ_ID)),
            when(version, is_not_null), when(status, is_equal_to_string(ATFP__COMMITTED_FOLDER_NAME))
        );
        expect(utest_storage_unlink, will_return(0), when(asaobj, is_equal_to(&mock_asa_remote)));
        expect(
            utest_storage_rmdir, will_return(expect_err), when(asaobj, is_equal_to(&mock_asa_remote)),
            when(path, is_equal_to_string(UT_RESC_COMMITTED_LOCALPATH))
        );
        expect(utest_atfp_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        while (!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_greater_than(0));
        ret = PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, RUNNER_ACCESS_F_OK);
        assert_that(ret, is_equal_to(0));
    }
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_RESC_COMMITTED_PATH, rmdir);
    ATFP_REMOVAL_TEST_TEARDOWN
} // end of  atfp_removal_test__err_rm_status_folder

TestSuite *app_transcoder_removal_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_removal_test__ok_all_empty);
    add_test(suite, atfp_removal_test__ok_all_status_folders);
    add_test(suite, atfp_removal_test__ok_one_status_folder);
    add_test(suite, atfp_removal_test__err_scan_status_versions);
    add_test(suite, atfp_removal_test__err_rm_single_version);
    add_test(suite, atfp_removal_test__err_rm_status_folder);
    return suite;
}
