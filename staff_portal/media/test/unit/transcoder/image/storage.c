#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "storage/localfs.h"
#include "transcoder/image/common.h"

#define  EXPECT_DONE_FLAG__IN_ASA_USRARG      (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  UTEST_NUM_USRARGS_ASA   (EXPECT_DONE_FLAG__IN_ASA_USRARG + 1)

#define  STRINGIFY(x)    #x

#define  UTEST_USER_ID      125
#define  UTEST_USER_ID_STR  STRINGIFY(125)
#define  UTEST_UPLOAD_REQ_ID      0x00a2b3b5
#define  UTEST_UPLOAD_REQ_ID_STR  STRINGIFY(00a2b3b5)
#define  UTEST_VERSION    "LW"
#define  UTEST_BASEPATH   "tmp/utest"
#define  UTEST_APP_BASEPATH   UTEST_BASEPATH"/media"

#define  UTEST_ASA_LOCAL_BASEPATH    UTEST_APP_BASEPATH "/asa_local"
#define  UTEST_ASA_REMOTE_BASEPATH   UTEST_APP_BASEPATH "/asa_remote"
#define  UTEST_ASAREMOTE_TRANSCODING_PATH   UTEST_ASA_REMOTE_BASEPATH"/"ATFP__TEMP_TRANSCODING_FOLDER_NAME
#define  UTEST_ASAREMOTE_DISCARDING_PATH    UTEST_ASA_REMOTE_BASEPATH"/"ATFP__DISCARDING_FOLDER_NAME
#define  UTEST_ASALOCAL_UNCLOSED_FILEPATH    UTEST_ASA_LOCAL_BASEPATH"/mock_transferring_file"
#define  UTEST_ASAREMOTE_UNCLOSED_FILEPATH   UTEST_ASAREMOTE_TRANSCODING_PATH"/"UTEST_VERSION
#define  UTEST_ASAREMOTE_DISCARDED_FILEPATH  UTEST_ASAREMOTE_DISCARDING_PATH"/"UTEST_VERSION

static void  utest_atfp_rm_remote_version(atfp_t *processor, const char *status)
{
   if(strcmp(status, ATFP__TEMP_TRANSCODING_FOLDER_NAME) == 0) {
       unlink(UTEST_ASAREMOTE_UNCLOSED_FILEPATH);
   } else if (strcmp(status, ATFP__DISCARDING_FOLDER_NAME)) {
       unlink(UTEST_ASAREMOTE_DISCARDED_FILEPATH);
   }
   int err = (int)mock(processor, status);
   if(err)
       json_object_set_new(processor->data.error, "utest", json_string("assume error happened"));
   processor->data.callback(processor);
}

static void  utest_atfp_img__common_deinit_done_cb(atfp_img_t *igproc)
{
    asa_op_base_cfg_t  *_asa_remote = igproc->super.data.storage.handle;
    uint8_t  *_done_flag = _asa_remote->cb_args.entries[EXPECT_DONE_FLAG__IN_ASA_USRARG];
    mock(igproc);
    *_done_flag = 1;
}

#define  UTEST_STORAGE__DEINIT_SETUP \
    uint8_t  done_flag = 0; \
    uv_loop_t *loop = uv_default_loop(); \
    void *mock_asa_cb_args[UTEST_NUM_USRARGS_ASA] = {0}; \
    asa_cfg_t  mock_common_storage_cfg = {.ops={.fn_close=app_storage_localfs_close, \
        .fn_unlink=app_storage_localfs_unlink}}; \
    asa_op_localfs_cfg_t  mock_asa_remote = {.loop=loop, .super={.storage=&mock_common_storage_cfg, \
        .cb_args={.entries=&mock_asa_cb_args[0], .size=UTEST_NUM_USRARGS_ASA}, \
        .op={.open={.dst_path=UTEST_ASAREMOTE_UNCLOSED_FILEPATH}} \
    }}; \
    atfp_img_t mock_fp = {.super={.transfer={.transcoded_dst={.flags={.asalocal_open=1, \
        .asaremote_open=1, .version_exists=1, .version_created=1 }, .remove_file=utest_atfp_rm_remote_version \
        }}, .data={.storage={.handle=&mock_asa_remote.super}}}, \
        .internal={.dst={.asa_local={.super={.storage=&mock_common_storage_cfg, .cb_args={ \
            .entries=&mock_asa_cb_args[0], .size=UTEST_NUM_USRARGS_ASA}, \
            .op={.open={.dst_path=UTEST_ASALOCAL_UNCLOSED_FILEPATH}}},  .loop=loop}}} \
    }; \
    mock_asa_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    mock_asa_cb_args[EXPECT_DONE_FLAG__IN_ASA_USRARG] = &done_flag; \
    mkdir(UTEST_BASEPATH, S_IRWXU); \
    mkdir(UTEST_APP_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASA_LOCAL_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASA_REMOTE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_TRANSCODING_PATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_DISCARDING_PATH,  S_IRWXU);

#define  UTEST_STORAGE__DEINIT_TEARDOWN \
    rmdir(UTEST_ASAREMOTE_DISCARDING_PATH); \
    rmdir(UTEST_ASAREMOTE_TRANSCODING_PATH); \
    rmdir(UTEST_ASA_REMOTE_BASEPATH); \
    rmdir(UTEST_ASA_LOCAL_BASEPATH); \
    rmdir(UTEST_APP_BASEPATH); \
    rmdir(UTEST_BASEPATH);

Ensure(atfp_img_storage_test__common_deinit_ok)
{
    UTEST_STORAGE__DEINIT_SETUP
    mock_fp.internal.dst.asa_local.file.file = open(UTEST_ASALOCAL_UNCLOSED_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    mock_asa_remote.file.file = open(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    atfp_img_dst_common_deinit (&mock_fp, utest_atfp_img__common_deinit_done_cb);
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(0));
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(0));
    expect(utest_atfp_rm_remote_version, will_return(0), when(status, is_equal_to_string(ATFP__DISCARDING_FOLDER_NAME)));
    expect(utest_atfp_rm_remote_version, will_return(0), when(status, is_equal_to_string(ATFP__TEMP_TRANSCODING_FOLDER_NAME)));
    expect(utest_atfp_img__common_deinit_done_cb, when(igproc, is_equal_to(&mock_fp)));
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    UTEST_STORAGE__DEINIT_TEARDOWN
} // end of atfp_img_storage_test__common_deinit_ok

Ensure(atfp_img_storage_test__common_deinit_localf_err)
{ // assume files on both sides were already deleted without closing it
    UTEST_STORAGE__DEINIT_SETUP
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    atfp_img_dst_common_deinit (&mock_fp, utest_atfp_img__common_deinit_done_cb);
    expect(utest_atfp_rm_remote_version, will_return(0), when(status, is_equal_to_string(ATFP__DISCARDING_FOLDER_NAME)));
    expect(utest_atfp_rm_remote_version, will_return(0), when(status, is_equal_to_string(ATFP__TEMP_TRANSCODING_FOLDER_NAME)));
    expect(utest_atfp_img__common_deinit_done_cb, when(igproc, is_equal_to(&mock_fp)));
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    UTEST_STORAGE__DEINIT_TEARDOWN
} // end of atfp_img_storage_test__common_deinit_localf_err

Ensure(atfp_img_storage_test__common_deinit_remotef_err)
{
    UTEST_STORAGE__DEINIT_SETUP
    mock_fp.internal.dst.asa_local.file.file = open(UTEST_ASALOCAL_UNCLOSED_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    mock_asa_remote.file.file = open(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    atfp_img_dst_common_deinit (&mock_fp, utest_atfp_img__common_deinit_done_cb);
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(0));
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(0));
    expect(utest_atfp_rm_remote_version, will_return(1), when(status, is_equal_to_string(ATFP__DISCARDING_FOLDER_NAME)));
    expect(utest_atfp_rm_remote_version, will_return(1), when(status, is_equal_to_string(ATFP__TEMP_TRANSCODING_FOLDER_NAME)));
    expect(utest_atfp_img__common_deinit_done_cb, when(igproc, is_equal_to(&mock_fp)));
    while(!done_flag)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(access(UTEST_ASALOCAL_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    assert_that(access(UTEST_ASAREMOTE_UNCLOSED_FILEPATH, F_OK), is_equal_to(-1));
    UTEST_STORAGE__DEINIT_TEARDOWN
} // end of atfp_img_storage_test__common_deinit_remotef_err

#undef  UTEST_ASA_LOCAL_BASEPATH
#undef  UTEST_ASA_REMOTE_BASEPATH
#undef  UTEST_ASAREMOTE_TRANSCODING_PATH
#undef  UTEST_ASAREMOTE_DISCARDING_PATH
#undef  UTEST_ASALOCAL_UNCLOSED_FILEPATH
#undef  UTEST_ASAREMOTE_UNCLOSED_FILEPATH
#undef  UTEST_ASAREMOTE_DISCARDED_FILEPATH



#define  UTEST_ASA_REMOTE_BASEPATH   UTEST_APP_BASEPATH"/asa_remote"
#define  UTEST_ASAREMOTE_UESR_PATH   UTEST_ASA_REMOTE_BASEPATH"/"UTEST_USER_ID_STR
#define  UTEST_ASAREMOTE_RESOURCE_PATH      UTEST_ASAREMOTE_UESR_PATH"/"UTEST_UPLOAD_REQ_ID_STR
#define  UTEST_ASAREMOTE_TRANSCODING_PATH   UTEST_ASAREMOTE_RESOURCE_PATH"/"ATFP__TEMP_TRANSCODING_FOLDER_NAME
#define  UTEST_ASAREMOTE_DISCARDING_PATH    UTEST_ASAREMOTE_RESOURCE_PATH"/"ATFP__DISCARDING_FOLDER_NAME
#define  UTEST_ASAREMOTE_TRANSCODING_FILEPATH  UTEST_ASAREMOTE_TRANSCODING_PATH"/"UTEST_VERSION
#define  UTEST_ASAREMOTE_DISCARDED_FILEPATH    UTEST_ASAREMOTE_DISCARDING_PATH"/"UTEST_VERSION

static void  utest_atfp_img__remove_version_done_cb(atfp_t *processor)
{
    json_t *_err_info = processor->data.error;
    int  num_errs = json_object_size(_err_info);
    mock(processor, num_errs);
}

#define  UTEST_STORAGE__RM_VERSION_SETUP \
    uv_loop_t *loop = uv_default_loop(); \
    json_t *mock_err_info = json_object(); \
    void *mock_asa_cb_args[UTEST_NUM_USRARGS_ASA] = {0}; \
    asa_cfg_t  mock_common_storage_cfg = {.ops={.fn_unlink=app_storage_localfs_unlink}, \
         .base_path=UTEST_ASA_REMOTE_BASEPATH}; \
    asa_op_localfs_cfg_t  mock_asa_remote = {.loop=loop, .super={.storage=&mock_common_storage_cfg, \
        .cb_args={.entries=&mock_asa_cb_args[0], .size=UTEST_NUM_USRARGS_ASA}}}; \
    atfp_t  mock_fp = {.data={.storage={.handle=&mock_asa_remote.super}, .error=mock_err_info, \
        .usr_id=UTEST_USER_ID, .upld_req_id=UTEST_UPLOAD_REQ_ID, .version=UTEST_VERSION, \
        .callback=utest_atfp_img__remove_version_done_cb, \
    }}; \
    mock_asa_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    mkdir(UTEST_BASEPATH, S_IRWXU); \
    mkdir(UTEST_APP_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASA_REMOTE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_UESR_PATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_RESOURCE_PATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_TRANSCODING_PATH, S_IRWXU); \
    mkdir(UTEST_ASAREMOTE_DISCARDING_PATH,  S_IRWXU);

#define  UTEST_STORAGE__RM_VERSION_TEARDOWN \
    json_decref(mock_err_info); \
    unlink(UTEST_ASAREMOTE_TRANSCODING_FILEPATH); \
    unlink(UTEST_ASAREMOTE_DISCARDED_FILEPATH); \
    rmdir(UTEST_ASAREMOTE_DISCARDING_PATH); \
    rmdir(UTEST_ASAREMOTE_TRANSCODING_PATH); \
    rmdir(UTEST_ASAREMOTE_RESOURCE_PATH); \
    rmdir(UTEST_ASAREMOTE_UESR_PATH); \
    rmdir(UTEST_ASA_REMOTE_BASEPATH); \
    rmdir(UTEST_APP_BASEPATH); \
    rmdir(UTEST_BASEPATH);

Ensure(atfp_img_storage_test__remove_version_ok)
{
    UTEST_STORAGE__RM_VERSION_SETUP
    int fd = 0;
    fd = open(UTEST_ASAREMOTE_TRANSCODING_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    close(fd);
    fd = open(UTEST_ASAREMOTE_DISCARDED_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    close(fd);
    atfp_storage_image_remove_version(&mock_fp, ATFP__TEMP_TRANSCODING_FOLDER_NAME);
    expect(utest_atfp_img__remove_version_done_cb, when(num_errs, is_equal_to(0)),
            when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(access(UTEST_ASAREMOTE_TRANSCODING_FILEPATH, F_OK), is_equal_to(-1));
    atfp_storage_image_remove_version(&mock_fp, ATFP__DISCARDING_FOLDER_NAME);
    expect(utest_atfp_img__remove_version_done_cb, when(num_errs, is_equal_to(0)),
            when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(access(UTEST_ASAREMOTE_DISCARDED_FILEPATH, F_OK), is_equal_to(-1));
    UTEST_STORAGE__RM_VERSION_TEARDOWN
} // end of atfp_img_storage_test__remove_version_ok

Ensure(atfp_img_storage_test__remove_version_err)
{ // assume the transcoding file was already deleted
    UTEST_STORAGE__RM_VERSION_SETUP
    assert_that(access(UTEST_ASAREMOTE_TRANSCODING_FILEPATH, F_OK), is_equal_to(-1));
    assert_that(access(UTEST_ASAREMOTE_DISCARDED_FILEPATH, F_OK), is_equal_to(-1));
    atfp_storage_image_remove_version(&mock_fp, ATFP__TEMP_TRANSCODING_FOLDER_NAME);
    expect(utest_atfp_img__remove_version_done_cb, when(num_errs, is_equal_to(0)),
            when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    atfp_storage_image_remove_version(&mock_fp, ATFP__DISCARDING_FOLDER_NAME);
    expect(utest_atfp_img__remove_version_done_cb, when(num_errs, is_equal_to(0)),
            when(processor, is_equal_to(&mock_fp)));
    uv_run(loop, UV_RUN_ONCE);
    UTEST_STORAGE__RM_VERSION_TEARDOWN
} // end of atfp_img_storage_test__remove_version_err

TestSuite *app_transcoder_image_storage_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_storage_test__common_deinit_ok);
    add_test(suite, atfp_img_storage_test__common_deinit_localf_err);
    add_test(suite, atfp_img_storage_test__common_deinit_remotef_err);
    add_test(suite, atfp_img_storage_test__remove_version_ok);
    add_test(suite, atfp_img_storage_test__remove_version_err);
    return suite;
}
