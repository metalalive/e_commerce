#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "utils.h"
#include "app_cfg.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"

#define UTEST_BASSEPATH           "tmp/utest/media"
#define UTEST_ASA_LOCAL_BASEPATH  UTEST_BASSEPATH "/asa_local"
#define UTEST_ASA_REMOTE_BASEPATH UTEST_BASSEPATH "/asa_remote"

#define NUM_READY_SEGMENTS         5
#define NUM_READY_METADATA_FILES   3
#define UTEST_DATA_SEGMENT_PREFIX  "utest_dataseg_"
#define UTEST_DATA_SEGMENT_PATTERN "%07d"
#define UTEST_SEGMENT_NUM_MAXDIGIT 7
#define MOCK_ASA_WR_BUF_SZ         16
#define NBYTES_SEGMENT_FULLPATH__ASA_LOCAL \
    sizeof(UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX) + UTEST_SEGMENT_NUM_MAXDIGIT
#define NBYTES_SEGMENT_FULLPATH__ASA_DST \
    sizeof(UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX) + UTEST_SEGMENT_NUM_MAXDIGIT

#define EXPECT_DONE_FLAG__IN_ASA_USRARG (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ              (EXPECT_DONE_FLAG__IN_ASA_USRARG + 1)

#define RUNNER_CREATE_FOLDER(fullpath)          mkdir(fullpath, S_IRWXU)
#define RUNNER_OPEN_WRONLY_CREATE_USR(fullpath) open(fullpath, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR)
#define RUNNER_OPEN_RDONLY_USR(fullpath)        open(fullpath, O_RDONLY, S_IRUSR)
#define RUNNER_ACCESS_F_OK(fullpath)            access(fullpath, F_OK)

static uint8_t utest__atfp_has_done_processing(atfp_t *processor) { return (uint8_t)mock(processor); }

#define UTEST_HLS__FLUSH_OUTPUT_SETUP \
    app_envvars_t env = {0}; \
    app_load_envvars(&env); \
    uint8_t    flush_done = 0; \
    void      *asalocal_usr_args[NUM_CB_ARGS_ASAOBJ] = {0, 0, &flush_done}; \
    void      *asaremote_usr_args[NUM_CB_ARGS_ASAOBJ] = {0, 0, &flush_done}; \
    char       mock_asa_wr_buf[MOCK_ASA_WR_BUF_SZ] = {0}; \
    char       seg_fullpath_asalocal[NBYTES_SEGMENT_FULLPATH__ASA_LOCAL] = {0}; \
    char       seg_fullpath_asadst[NBYTES_SEGMENT_FULLPATH__ASA_DST] = {0}; \
    uv_loop_t *loop = uv_default_loop(); \
    int        idx = 0; \
    asa_cfg_t  mock_storage_common = { \
         .base_path = env.sys_base_path, \
         .ops = \
            {.fn_open = app_storage_localfs_open, \
              .fn_close = app_storage_localfs_close, \
              .fn_write = app_storage_localfs_write, \
              .fn_read = app_storage_localfs_read, \
              .fn_unlink = app_storage_localfs_unlink} \
    }; \
    asa_op_localfs_cfg_t mock_asa_remote = { \
        .loop = loop, \
        .super = \
            {.op = \
                 {.open = {.cb = NULL}, \
                  .write = {.src = &mock_asa_wr_buf[0], .src_max_nbytes = MOCK_ASA_WR_BUF_SZ}, \
                  .mkdir = {.path = {.origin = UTEST_ASA_REMOTE_BASEPATH}}}, \
             .cb_args = {.entries = (void **)asaremote_usr_args, .size = NUM_CB_ARGS_ASAOBJ}, \
             .storage = &mock_storage_common} \
    }; \
    atfp_ops_t mock_atfp_ops = {.has_done_processing = utest__atfp_has_done_processing}; \
    atfp_hls_t \
        mock_hlsproc = \
            { \
                .asa_local = \
                    {.super = \
                         {.cb_args = {.entries = (void **)asalocal_usr_args, .size = NUM_CB_ARGS_ASAOBJ}, \
                          .op = {.mkdir = {.path = {.origin = UTEST_ASA_LOCAL_BASEPATH}}}, \
                          .storage = &mock_storage_common}, \
                     .loop = loop}, \
                .super = \
                    {.data = \
                         {.error = json_object(), \
                          .callback = utest_atfp_hls__flush_output_cb, \
                          .storage = \
                              { \
                                  .basepath = NULL, \
                                  .handle = &mock_asa_remote.super, \
                              }}, \
                     .ops = &mock_atfp_ops}, \
                .internal = \
                    {.segment = \
                         { \
                             .rdy_list = {.capacity = 0, .size = 0, .entries = NULL}, \
                             .filename = \
                                 {.prefix = \
                                      {.data = UTEST_DATA_SEGMENT_PREFIX, \
                                       .sz = strlen(UTEST_DATA_SEGMENT_PREFIX)}, \
                                  .pattern = \
                                      {.data = UTEST_DATA_SEGMENT_PATTERN, \
                                       .sz = strlen(UTEST_DATA_SEGMENT_PATTERN), \
                                       .max_num_digits = UTEST_SEGMENT_NUM_MAXDIGIT}}, \
                             .fullpath = {._asa_local = {.data = &seg_fullpath_asalocal[0], .sz = NBYTES_SEGMENT_FULLPATH__ASA_LOCAL}, ._asa_dst = {.data = &seg_fullpath_asadst[0], .sz = NBYTES_SEGMENT_FULLPATH__ASA_DST}}, \
                         }}, \
    }; \
    asaremote_usr_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_hlsproc; \
    asalocal_usr_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_hlsproc; \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, "./tmp/utest", RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_BASSEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_ASA_LOCAL_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_ASA_REMOTE_BASEPATH, RUNNER_CREATE_FOLDER);

#define UTEST_HLS__FLUSH_FILES_SETUP \
    const char *expect_seg_local_path[NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES] = { \
        UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000004", \
        UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000195", \
        UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000026", \
        UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000994", \
        UTEST_ASA_LOCAL_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000008", \
        UTEST_ASA_LOCAL_BASEPATH "/" HLS_FMP4_FILENAME, \
        UTEST_ASA_LOCAL_BASEPATH "/" HLS_MASTER_PLAYLIST_FILENAME, \
        UTEST_ASA_LOCAL_BASEPATH "/" HLS_PLAYLIST_FILENAME, \
    }; \
    const char *expect_seg_remote_path[NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES] = { \
        UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000004", \
        UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000195", \
        UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000026", \
        UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000994", \
        UTEST_ASA_REMOTE_BASEPATH "/" UTEST_DATA_SEGMENT_PREFIX "0000008", \
        UTEST_ASA_REMOTE_BASEPATH "/" HLS_FMP4_FILENAME, \
        UTEST_ASA_REMOTE_BASEPATH "/" HLS_MASTER_PLAYLIST_FILENAME, \
        UTEST_ASA_REMOTE_BASEPATH "/" HLS_PLAYLIST_FILENAME, \
    }; \
    const char *expect_seg_content[NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES] = { \
        "Tienanmen massacre on June 4 1989, students calling for democracy killed by tank, " \
        "gunshoot from CCP's brainless military", \
        "CCP forcefully harvests organs from prisoners, many of them were tortured before death", \
        "CCP causes Uyghur genocide, forces them to give up Islam and become Han Chinese by " \
        "forcing them to eat pork meat", \
        "Millions of people still don't realize the negative impact of debt trap from " \
        "one-belt-one-road from CCP, ", \
        "which is new way of taking over any territory around the world, money game invented by " \
        "China Communist party", \
        "CCP has made huge amount of effort trying to cover their dark history, by setting up " \
        "international news media", \
        "brainwishing people around the world, spreading political propaganda 24/7", \
        "take over one country after another, it is not for history, it is for perfornal " \
        "benefit.", \
    }; \
    for (idx = 0; idx < (NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES); idx++) { \
        int fd = PATH_CONCAT_THEN_RUN( \
            env.sys_base_path, expect_seg_local_path[idx], RUNNER_OPEN_WRONLY_CREATE_USR \
        ); \
        write(fd, expect_seg_content[idx], strlen(expect_seg_content[idx])); \
        close(fd); \
    }

#define UTEST_HLS__FLUSH_OUTPUT_TEARDOWN \
    if (mock_hlsproc.internal.segment.rdy_list.entries) { \
        free(mock_hlsproc.internal.segment.rdy_list.entries); \
        mock_hlsproc.internal.segment.rdy_list.entries = NULL; \
    } \
    json_decref(mock_hlsproc.super.data.error); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_ASA_LOCAL_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_ASA_REMOTE_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, UTEST_BASSEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(env.sys_base_path, "./tmp/utest", rmdir);

#define UTEST_HLS__FLUSH_FILES_TEARDOWN \
    for (idx = 0; idx < (NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES); idx++) { \
        PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[idx], unlink); \
        PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[idx], unlink); \
    }

static void utest_atfp_hls__flush_output_cb(atfp_t *processor) {
    atfp_hls_t           *hlsproc = (atfp_hls_t *)processor;
    asa_op_localfs_cfg_t *asa_local_dst = &hlsproc->asa_local;
    size_t                num_err_msg = json_object_size(processor->data.error);
    uint8_t              *done_flag = asa_local_dst->super.cb_args.entries[EXPECT_DONE_FLAG__IN_ASA_USRARG];
    *done_flag = 1;
    mock(processor, num_err_msg);
} // end of utest_atfp_hls__flush_output_cb

static void
utest_hls__output_verify_content(app_envvars_t *env, const char *filepath, const char *expect_content) {
    size_t expect_content_sz = strlen(expect_content);
    char   actual_content[expect_content_sz + 1];
    int    fd = PATH_CONCAT_THEN_RUN(env->sys_base_path, filepath, RUNNER_OPEN_RDONLY_USR);
    int    nread = read(fd, &actual_content[0], expect_content_sz);
    actual_content[nread] = 0;
    assert_that(nread, is_equal_to(expect_content_sz));
    assert_that(&actual_content[0], is_equal_to_string(expect_content));
    close(fd);
}

Ensure(atfp_hls_test__flush_segments__when_processing) {
    UTEST_HLS__FLUSH_OUTPUT_SETUP;
    UTEST_HLS__FLUSH_FILES_SETUP;
    size_t       expect_numfiles_transferred = NUM_READY_SEGMENTS - 1;
    ASA_RES_CODE result = atfp_hls__try_flush_to_storage(&mock_hlsproc);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest__atfp_has_done_processing, will_return(0));
    expect(utest__atfp_has_done_processing, will_return(0));
    expect(utest_atfp_hls__flush_output_cb, when(num_err_msg, is_equal_to(0)));
    for (idx = 0; idx < expect_numfiles_transferred; idx++) { // final segment not ready yet
        int wr_buf_sz = mock_asa_remote.super.op.write.src_max_nbytes;
        expect(SHA1_Init, will_return(1));
        for (int file_sz = strlen(expect_seg_content[idx]); file_sz > 0; file_sz -= wr_buf_sz)
            expect(SHA1_Update, will_return(1));
        expect(SHA1_Final, will_return(1));
        expect(OPENSSL_cleanse);
    } // end of loop
    while (!flush_done)
        uv_run(loop, UV_RUN_ONCE);
    { // examine after completing transfer
        int access_result;
        assert_that(mock_hlsproc.internal.segment.rdy_list.size, is_equal_to(expect_numfiles_transferred));
        assert_that(mock_hlsproc.internal.segment.rdy_list.entries, is_not_equal_to(NULL));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[0], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[1], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[2], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[3], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[4], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[5], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[6], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        // the segment with the latest number not transferred
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[0], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[1], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[2], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[3], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[4], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[5], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result = PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[6], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        utest_hls__output_verify_content(&env, expect_seg_remote_path[0], expect_seg_content[0]);
        utest_hls__output_verify_content(&env, expect_seg_remote_path[2], expect_seg_content[2]);
        utest_hls__output_verify_content(&env, expect_seg_remote_path[4], expect_seg_content[4]);
    }
    if (!mock_hlsproc.internal.segment.rdy_list.entries)
        free(mock_hlsproc.internal.segment.rdy_list.entries);
    UTEST_HLS__FLUSH_FILES_TEARDOWN;
    UTEST_HLS__FLUSH_OUTPUT_TEARDOWN;
} // end of atfp_hls_test__flush_segments__when_processing

Ensure(atfp_hls_test__flush_nothing__when_processing) {
    UTEST_HLS__FLUSH_OUTPUT_SETUP
    const char *expect_filepath = UTEST_ASA_LOCAL_BASEPATH "/"
                                                           "not_segment_file";
    {
        int fd = PATH_CONCAT_THEN_RUN(
            env.sys_base_path, expect_filepath, RUNNER_OPEN_WRONLY_CREATE_USR
        ); // create an empty file
        close(fd);
    }
    ASA_RES_CODE result = atfp_hls__try_flush_to_storage(&mock_hlsproc);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest__atfp_has_done_processing, will_return(0));
    expect(utest_atfp_hls__flush_output_cb, when(num_err_msg, is_equal_to(0)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(mock_hlsproc.internal.segment.rdy_list.size, is_equal_to(0));
    assert_that(
        mock_hlsproc.internal.segment.rdy_list.entries, is_equal_to(NULL)
    ); // expected because nothing transferred
    PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_filepath, unlink);
    UTEST_HLS__FLUSH_OUTPUT_TEARDOWN
} // end of atfp_hls_test__flush_nothing__when_processing

Ensure(atfp_hls_test__flush_segments__final) {
    UTEST_HLS__FLUSH_OUTPUT_SETUP;
    UTEST_HLS__FLUSH_FILES_SETUP;
    size_t       expect_numfiles_transferred = (NUM_READY_SEGMENTS + NUM_READY_METADATA_FILES);
    ASA_RES_CODE result = atfp_hls__try_flush_to_storage(&mock_hlsproc);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest__atfp_has_done_processing, will_return(1));
    expect(utest__atfp_has_done_processing, will_return(1));
    expect(utest_atfp_hls__flush_output_cb, when(num_err_msg, is_equal_to(0)));
    for (idx = 0; idx < expect_numfiles_transferred; idx++) { // final segment already ready
        int wr_buf_sz = mock_asa_remote.super.op.write.src_max_nbytes;
        expect(SHA1_Init, will_return(1));
        for (int file_sz = strlen(expect_seg_content[idx]); file_sz > 0; file_sz -= wr_buf_sz)
            expect(SHA1_Update, will_return(1));
        expect(SHA1_Final, will_return(1));
        expect(OPENSSL_cleanse);
    } // end of loop
    while (!flush_done)
        uv_run(loop, UV_RUN_ONCE);
    assert_that(mock_hlsproc.internal.segment.rdy_list.size, is_equal_to(NUM_READY_SEGMENTS));
    assert_that(
        mock_hlsproc.internal.segment.rdy_list.entries, is_not_equal_to(NULL)
    ); // list has entries of files that were transferred
    for (idx = 0; idx < expect_numfiles_transferred; idx++) {
        int access_result;
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[idx], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(0));
        access_result =
            PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[idx], RUNNER_ACCESS_F_OK);
        assert_that(access_result, is_equal_to(-1));
    }
    if (!mock_hlsproc.internal.segment.rdy_list.entries)
        free(mock_hlsproc.internal.segment.rdy_list.entries);
    UTEST_HLS__FLUSH_FILES_TEARDOWN;
    UTEST_HLS__FLUSH_OUTPUT_TEARDOWN;
} // end of atfp_hls_test__flush_segments__final

// assume a segment file is deleting accidentally when it is transferring
Ensure(atfp_hls_test__flush_error__transfer_segment) {
    UTEST_HLS__FLUSH_OUTPUT_SETUP;
    UTEST_HLS__FLUSH_FILES_SETUP;
    ASA_RES_CODE result = atfp_hls__try_flush_to_storage(&mock_hlsproc);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest__atfp_has_done_processing, will_return(0));
    expect(utest_atfp_hls__flush_output_cb, when(num_err_msg, is_equal_to(1)));
    { // assume first segment is transferred successfully
        int wr_buf_sz = mock_asa_remote.super.op.write.src_max_nbytes;
        expect(SHA1_Init, will_return(1));
        for (int file_sz = strlen(expect_seg_content[0]); file_sz > 0; file_sz -= wr_buf_sz)
            expect(SHA1_Update, will_return(1));
        expect(SHA1_Final, will_return(1));
        expect(OPENSSL_cleanse);
    }
    int access_result;
    while ((access_result =
                PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_remote_path[0], RUNNER_ACCESS_F_OK)) == -1)
        uv_run(loop, UV_RUN_ONCE);
    uv_run(loop, UV_RUN_ONCE);
    uv_run(loop, UV_RUN_ONCE);
    uv_run(loop, UV_RUN_ONCE);
    uv_run(loop, UV_RUN_ONCE); // there should be some bytes written in destination storage
    PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[0], unlink);
    while (!flush_done)
        uv_run(loop, UV_RUN_ONCE);
    json_t *err_info = json_object_get(mock_hlsproc.super.data.error, "storage");
    assert_that(err_info, is_not_equal_to(NULL));
    if (!mock_hlsproc.internal.segment.rdy_list.entries)
        free(mock_hlsproc.internal.segment.rdy_list.entries);
    UTEST_HLS__FLUSH_FILES_TEARDOWN;
    UTEST_HLS__FLUSH_OUTPUT_TEARDOWN;
} // end of atfp_hls_test__flush_error__transfer_segment

// assume next segment file in local storage is deleting before transferring
Ensure(atfp_hls_test__flush_error__open_next_segment) {
    UTEST_HLS__FLUSH_OUTPUT_SETUP;
    UTEST_HLS__FLUSH_FILES_SETUP;
    ASA_RES_CODE result = atfp_hls__try_flush_to_storage(&mock_hlsproc);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest__atfp_has_done_processing, will_return(0));
    expect(utest_atfp_hls__flush_output_cb, when(num_err_msg, is_equal_to(1)));
    { // assume first segment is transferred successfully
        int wr_buf_sz = mock_asa_remote.super.op.write.src_max_nbytes;
        expect(SHA1_Init, will_return(1));
        for (int file_sz = strlen(expect_seg_content[0]); file_sz > 0; file_sz -= wr_buf_sz)
            expect(SHA1_Update, will_return(1));
        expect(SHA1_Final, will_return(1));
        expect(OPENSSL_cleanse);
    }
    int access_result;
    while ((access_result =
                PATH_CONCAT_THEN_RUN(env.sys_base_path, expect_seg_local_path[0], RUNNER_ACCESS_F_OK)) != -1)
        uv_run(loop, UV_RUN_ONCE);
    UTEST_HLS__FLUSH_FILES_TEARDOWN;
    while (!flush_done)
        uv_run(loop, UV_RUN_ONCE);
    json_t *err_info = json_object_get(mock_hlsproc.super.data.error, "storage");
    assert_that(err_info, is_not_equal_to(NULL));
    if (!mock_hlsproc.internal.segment.rdy_list.entries)
        free(mock_hlsproc.internal.segment.rdy_list.entries);
    UTEST_HLS__FLUSH_OUTPUT_TEARDOWN;
} // end of atfp_hls_test__flush_error__open_next_segment

#undef MOCK_ASA_WR_BUF_SZ
#undef NBYTES_SEGMENT_FULLPATH__ASA_DST
#undef NBYTES_SEGMENT_FULLPATH__ASA_LOCAL
#undef UTEST_SEGMENT_NUM_MAXDIGIT
#undef UTEST_DATA_SEGMENT_PREFIX
#undef UTEST_DATA_SEGMENT_PATTERN
#undef NUM_READY_SEGMENTS
#undef UTEST_ASA_REMOTE_BASEPATH
#undef UTEST_ASA_LOCAL_BASEPATH
#undef UTEST_BASSEPATH

TestSuite *app_transcoder_hls_output_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__flush_nothing__when_processing);
    add_test(suite, atfp_hls_test__flush_segments__when_processing);
    add_test(suite, atfp_hls_test__flush_segments__final);
    add_test(suite, atfp_hls_test__flush_error__transfer_segment);
    add_test(suite, atfp_hls_test__flush_error__open_next_segment);
    return suite;
}
