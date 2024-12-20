#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "transcoder/file_processor.h"
#define  UTEST_STRINGIFY(x)  #x

#define  LOCAL_TMPBUF_BASEPATH  "tmp/utest"
#define  DONE_FLAG_INDEX__IN_ASA_USRARG     (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ  (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)

#define  UTEST_USER_ID           438
#define  UTEST_UPLOAD_REQ_ID     0xe138d1a6
#define  UTEST_VERSION          "jR"
#define  UTEST_USER_ID__STR         UTEST_STRINGIFY(438)
#define  UTEST_UPLOAD_REQ_ID__STR   UTEST_STRINGIFY(e138d1a6)
#define  UTEST_FILEREQ_PATH        LOCAL_TMPBUF_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR
#define  UTEST_FILEREQ_PATH_SZ     sizeof(UTEST_FILEREQ_PATH)
#define  UTEST_STATUS_PATH_SZ      ATFP__MAXSZ_STATUS_FOLDER_NAME + 1
#define  UTEST_VER_FULLPATH_SZ     UTEST_STATUS_PATH_SZ + UTEST_FILEREQ_PATH_SZ
#define  EXPECT_COMMITTING_NFILES   4
#define  EXPECT_COMMITTING_FILENAMES   {"let", "go", "calm", "down"}
#define  EXPECT_DISCARDING_NFILES   5
#define  EXPECT_DISCARDING_FILENAMES   {"Palau", "Guam", "Fiji", "Marshall", "Tonga"}

static void  utest__commit_new_version__done_cb(atfp_t *processor) {
    json_t *err_info = processor->data.error;
    int num_err_items = json_object_size(err_info);
    mock(processor, num_err_items);
    if(num_err_items > 0)
        json_object_clear(err_info);
    asa_op_base_cfg_t  *asa_dst = processor->data.storage.handle;
    if(asa_dst && asa_dst->cb_args.entries) {
        uint8_t *done_flag = asa_dst->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flag)
            *done_flag = 1;
    }
} // end of utest__commit_new_version__done_cb

static void _atfp_utest__commit_version__construct_path(char *out, size_t o_sz, const char *status, const char *fname)
{
    memset(&out[0], 0, o_sz);
    strncat(&out[0], UTEST_FILEREQ_PATH"/", strlen(UTEST_FILEREQ_PATH"/"));
    strncat(&out[0], status, strlen(status));
    strncat(&out[0], "/", 1);
    strncat(&out[0], UTEST_VERSION"/", strlen(UTEST_VERSION"/"));
    strncat(&out[0], fname, strlen(fname));
    assert_that(out[o_sz - 1], is_equal_to(0x0));
} // end of _atfp_utest__commit_version__construct_path

static void _atfp_utest__commit_version__files_setup(
        const char *status, const char **fnames, size_t fname_sz)
{
    int idx = 0, fd = -1;
    for(idx = 0; idx < fname_sz; idx++) {
        size_t fullpath_sz  = strlen(UTEST_FILEREQ_PATH"/") + strlen(status) + strlen("/") +
            strlen(UTEST_VERSION"/") +  strlen(fnames[idx]) + 1;
        char fullpath[fullpath_sz];
        _atfp_utest__commit_version__construct_path(&fullpath[0], fullpath_sz, status, fnames[idx]);
        fd = open(&fullpath[0], O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        close(fd);
    } // end of loop
} // end of _atfp_utest__commit_version__files_setup

static void _atfp_utest__commit_version__files_teardown(
        const char *status, const char **fnames, size_t fname_sz)
{
    int idx = 0;
    for(idx = 0; idx < fname_sz; idx++) {
        size_t fullpath_sz  = strlen(UTEST_FILEREQ_PATH"/") + strlen(status) + strlen("/") +
            strlen(UTEST_VERSION"/") +  strlen(fnames[idx]) + 1;
        char fullpath[fullpath_sz];
        _atfp_utest__commit_version__construct_path(&fullpath[0], fullpath_sz, status, fnames[idx]);
        unlink(&fullpath[0]);
    } // end of loop
} // end of _atfp_utest__commit_version__files_teardown

static void _atfp_utest__commit_version__verify_existence(
        const char *status, const char **fnames, size_t fname_sz, int expect_ret)
{
    int idx = 0, ret = 0;
    for(idx = 0; idx < fname_sz; idx++) {
        size_t fullpath_sz  = strlen(UTEST_FILEREQ_PATH"/") + strlen(status) + strlen("/") +
            strlen(UTEST_VERSION"/") +  strlen(fnames[idx]) + 1;
        char fullpath[fullpath_sz];
        _atfp_utest__commit_version__construct_path(&fullpath[0], fullpath_sz, status, fnames[idx]);
        ret = access(&fullpath[0], F_OK);
        assert_that(ret, is_equal_to(expect_ret));
    } // end of loop
} // end of  _atfp_utest__commit_version__verify_existence

#define  UTEST__COMMIT_VERSION_SETUP  \
    char mkdir_path_prefix[UTEST_FILEREQ_PATH_SZ] = {0}; \
    char mkdir_path_origin[UTEST_STATUS_PATH_SZ] = {0}; \
    char mkdir_path_currparent[UTEST_VER_FULLPATH_SZ] = {0}; \
    uint8_t  done_flag = 0; \
    uv_loop_t *loop  = uv_default_loop(); \
    void  *asa_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_cfg_t  mock_storage_cfg = {.base_path=LOCAL_TMPBUF_BASEPATH, .ops={ \
        .fn_mkdir=app_storage_localfs_mkdir, .fn_rename=app_storage_localfs_rename}}; \
    asa_op_localfs_cfg_t  mock_asa_dst = { .loop=loop, .file={.file=-1}, .super={ .storage=&mock_storage_cfg, \
        .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_cb_args},  .op={ \
            .mkdir={.path={.origin=&mkdir_path_origin[0], .prefix=&mkdir_path_prefix[0], \
                .curr_parent=&mkdir_path_currparent[0] }},  \
    }}}; \
    json_t *mock_errinfo = json_object(); \
    atfp_t  mock_fp = {.data={.error=mock_errinfo, .callback=utest__commit_new_version__done_cb, \
        .storage={.handle=&mock_asa_dst.super}, .usr_id=UTEST_USER_ID, .upld_req_id=UTEST_UPLOAD_REQ_ID, \
        .version=UTEST_VERSION}, .transfer={.transcoded_dst={.flags={.version_exists=EXPECT_VERSION_EXIST_FLAG}}}}; \
    asa_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    asa_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    mkdir(LOCAL_TMPBUF_BASEPATH, S_IRWXU); \
    mkdir(LOCAL_TMPBUF_BASEPATH "/" UTEST_USER_ID__STR, S_IRWXU); \
    mkdir(UTEST_FILEREQ_PATH, S_IRWXU); \
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME, S_IRWXU); \
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME "/" UTEST_VERSION, S_IRWXU);

#define  UTEST__COMMIT_VERSION_TEARDOWN  \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME "/" UTEST_VERSION); \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME); \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME "/" UTEST_VERSION); \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME); \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__DISCARDING_FOLDER_NAME "/" UTEST_VERSION); \
    rmdir(UTEST_FILEREQ_PATH "/" ATFP__DISCARDING_FOLDER_NAME); \
    rmdir(UTEST_FILEREQ_PATH); \
    rmdir(LOCAL_TMPBUF_BASEPATH "/" UTEST_USER_ID__STR); \
    rmdir(LOCAL_TMPBUF_BASEPATH); \
    json_decref(mock_errinfo);

#define  EXPECT_VERSION_EXIST_FLAG  1
Ensure(atfp_test__commit_version__discard_old) {
    UTEST__COMMIT_VERSION_SETUP;
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME,        S_IRWXU);
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME "/" UTEST_VERSION,        S_IRWXU);
    const char *committing_fnames[EXPECT_COMMITTING_NFILES] = EXPECT_COMMITTING_FILENAMES;
    const char *discarding_fnames[EXPECT_DISCARDING_NFILES] = EXPECT_DISCARDING_FILENAMES;
    _atfp_utest__commit_version__files_setup(ATFP__TEMP_TRANSCODING_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    _atfp_utest__commit_version__files_setup(ATFP__COMMITTED_FOLDER_NAME,
         (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES);
    {
    /*
        expect(utest__commit_new_version__done_cb, when(processor, is_equal_to(&mock_fp)),
                when(num_err_items, is_equal_to(0)) );
        atfp_storage__commit_new_version(&mock_fp);
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        _atfp_utest__commit_version__verify_existence( ATFP__COMMITTED_FOLDER_NAME,
             (const char **)committing_fnames, EXPECT_COMMITTING_NFILES, 0);
        _atfp_utest__commit_version__verify_existence( ATFP__DISCARDING_FOLDER_NAME,
             (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES, 0);
        int ret = access(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME "/" UTEST_VERSION , F_OK);
        assert_that(ret, is_equal_to(-1));
     * */
    }
    _atfp_utest__commit_version__files_teardown( ATFP__COMMITTED_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    _atfp_utest__commit_version__files_teardown( ATFP__DISCARDING_FOLDER_NAME,
         (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES);
    UTEST__COMMIT_VERSION_TEARDOWN;
} // end of atfp_test__commit_version__discard_old
#undef   EXPECT_VERSION_EXIST_FLAG


#define  EXPECT_VERSION_EXIST_FLAG  0
Ensure(atfp_test__commit_new_version) {
    UTEST__COMMIT_VERSION_SETUP;
    const char *committing_fnames[EXPECT_COMMITTING_NFILES] = EXPECT_COMMITTING_FILENAMES;
    _atfp_utest__commit_version__files_setup(ATFP__TEMP_TRANSCODING_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    {
        expect(utest__commit_new_version__done_cb, when(processor, is_equal_to(&mock_fp)),
                when(num_err_items, is_equal_to(0)) );
        atfp_storage__commit_new_version(&mock_fp);
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        _atfp_utest__commit_version__verify_existence( ATFP__COMMITTED_FOLDER_NAME,
             (const char **)committing_fnames, EXPECT_COMMITTING_NFILES, 0);
        int ret = access(UTEST_FILEREQ_PATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME "/" UTEST_VERSION , F_OK);
        assert_that(ret, is_equal_to(-1));
        ret = access(UTEST_FILEREQ_PATH "/" ATFP__DISCARDING_FOLDER_NAME "/" UTEST_VERSION , F_OK);
        assert_that(ret, is_equal_to(-1));
    }
    _atfp_utest__commit_version__files_teardown( ATFP__COMMITTED_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    UTEST__COMMIT_VERSION_TEARDOWN;
} // end of atfp_test__commit_new_version
#undef   EXPECT_VERSION_EXIST_FLAG


#define  EXPECT_VERSION_EXIST_FLAG  0
Ensure(atfp_test__commit_version__dup_error) {
    UTEST__COMMIT_VERSION_SETUP;
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME,        S_IRWXU);
    mkdir(UTEST_FILEREQ_PATH "/" ATFP__COMMITTED_FOLDER_NAME "/" UTEST_VERSION,        S_IRWXU);
    const char *committing_fnames[EXPECT_COMMITTING_NFILES] = EXPECT_COMMITTING_FILENAMES;
    const char *discarding_fnames[EXPECT_DISCARDING_NFILES] = EXPECT_DISCARDING_FILENAMES;
    _atfp_utest__commit_version__files_setup(ATFP__TEMP_TRANSCODING_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    _atfp_utest__commit_version__files_setup(ATFP__COMMITTED_FOLDER_NAME,
         (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES);
    { // will fail to commit due to duplicate version folder in committed status
        expect(utest__commit_new_version__done_cb, when(processor, is_equal_to(&mock_fp)),
                when(num_err_items, is_equal_to(1)) );
        atfp_storage__commit_new_version(&mock_fp);
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        _atfp_utest__commit_version__verify_existence( ATFP__TEMP_TRANSCODING_FOLDER_NAME,
             (const char **)committing_fnames, EXPECT_COMMITTING_NFILES, 0);
        _atfp_utest__commit_version__verify_existence( ATFP__COMMITTED_FOLDER_NAME,
             (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES, 0);
    }
    _atfp_utest__commit_version__files_teardown( ATFP__TEMP_TRANSCODING_FOLDER_NAME,
         (const char **)committing_fnames, EXPECT_COMMITTING_NFILES);
    _atfp_utest__commit_version__files_teardown( ATFP__COMMITTED_FOLDER_NAME,
         (const char **)discarding_fnames, EXPECT_DISCARDING_NFILES);
    UTEST__COMMIT_VERSION_TEARDOWN;
} // end of  atfp_test__commit_version__dup_error
#undef   EXPECT_VERSION_EXIST_FLAG

#undef  EXPECT_DISCARDING_NFILES
#undef  EXPECT_DISCARDING_FILENAMES
#undef  EXPECT_COMMITTING_NFILES
#undef  EXPECT_COMMITTING_FILENAMES
#undef  UTEST_VER_FULLPATH_SZ
#undef  UTEST_STATUS_PATH_SZ
#undef  UTEST_FILEREQ_PATH_SZ
#undef  UTEST_FILEREQ_PATH
#undef  UTEST_UPLOAD_REQ_ID__STR
#undef  UTEST_USER_ID__STR
#undef  UTEST_VERSION
#undef  UTEST_UPLOAD_REQ_ID
#undef  UTEST_USER_ID

TestSuite *app_transcoder_storage_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_test__commit_version__discard_old);
    //add_test(suite, atfp_test__commit_new_version);
    //add_test(suite, atfp_test__commit_version__dup_error);
    return suite;
}
