#include <unistd.h>
#include <uv.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "storage/localfs.h"

#define EXPECT_CB_ARGS_SZ 3
#define EXPECT_FILE_PATH "./tmp/utest_asa_localfs.txt"
static __attribute__((optimize("O0")))  void utest_1_asa_close_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    { // cb_args check
        int *actual_arg = (int *)cfg->cb_args.entries[1];
        assert_that(actual_arg, is_not_null);
        if(actual_arg) {
            assert_that(*actual_arg, is_equal_to(234));
        }
    }
    free(cfg);
    unlink(EXPECT_FILE_PATH);
} // end of utest_1_asa_close_cb

static __attribute__((optimize("O0")))  void utest_1_asa_open_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    assert_that(cfg->cb_args.size, is_equal_to(EXPECT_CB_ARGS_SZ));
    { // cb_args check
        int *actual_arg = (int *)cfg->cb_args.entries[0];
        assert_that(actual_arg, is_not_null);
        if(actual_arg) {
            assert_that(*actual_arg, is_equal_to(123));
        }
    }
    cfg->op.open.dst_path = NULL;
    cfg->op.close.cb = utest_1_asa_close_cb;
    assert_that(app_storage_localfs_close(cfg), is_equal_to(ASTORAGE_RESULT_ACCEPT));
} // end of utest_1_asa_open_cb

static __attribute__((optimize("O0")))  void utest_4_asa_simple_callback (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{ mock(cfg, result); }

Ensure(storage_localfs_openfile_test) {
    int expect_cb_args[EXPECT_CB_ARGS_SZ] = {123, 234, 345};
    size_t  cfg_sz = sizeof(asa_op_localfs_cfg_t) + sizeof(void *) * EXPECT_CB_ARGS_SZ;
    asa_op_localfs_cfg_t *cfg = malloc(cfg_sz);
    cfg->file = (uv_fs_t){0};
    cfg->loop = uv_default_loop();
    cfg->super.op.open.cb = utest_1_asa_open_cb;
    cfg->super.op.open.dst_path = EXPECT_FILE_PATH; 
    cfg->super.op.open.mode = S_IRUSR | S_IWUSR;
    cfg->super.op.open.flags = O_CREAT | O_WRONLY;
    {
        char *ptr = (char *) cfg + sizeof(asa_op_localfs_cfg_t);
        cfg->super.cb_args.entries = (void **) ptr;
        cfg->super.cb_args.size = EXPECT_CB_ARGS_SZ;
        for(size_t idx = 0; idx < EXPECT_CB_ARGS_SZ; idx++) {
            cfg->super.cb_args.entries[idx] = (void *)&expect_cb_args[idx];
        } // end of loop
    }
    ASA_RES_CODE result = app_storage_localfs_open((asa_op_base_cfg_t *)cfg);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    uv_run(cfg->loop, UV_RUN_ONCE);
    uv_run(cfg->loop, UV_RUN_ONCE);
} // end of storage_localfs_openfile_test
#undef EXPECT_CB_ARGS_SZ
#undef EXPECT_FILE_PATH


#define EXPECT_MKDIR_PATH "tmp/utest/media/async_storage/a146"
#define EXPECT_RMDIR_PATH "tmp/utest/media/async_storage/a146"
static __attribute__((optimize("O0")))  void utest_2_asa_rmdir_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    cfg->op.rmdir.path = NULL;
    free(cfg);
    rmdir("tmp/utest/media/async_storage");
    rmdir("tmp/utest/media");
    rmdir("tmp/utest"); // would fail if there's any file/ subfolder in it
}

static __attribute__((optimize("O0")))  void utest_2_asa_mkdir_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    // TODO, check whether folder really exists, even though return value from libuv should be sufficient
    cfg->op.rmdir.path = EXPECT_RMDIR_PATH;
    cfg->op.rmdir.cb = utest_2_asa_rmdir_cb;
    assert_that(app_storage_localfs_rmdir(cfg), is_equal_to(ASTORAGE_RESULT_ACCEPT));
}

Ensure(storage_localfs_test__mkdir_simple) {
    size_t  cfg_sz = sizeof(asa_op_localfs_cfg_t) + sizeof(EXPECT_MKDIR_PATH) * 2;
    asa_op_localfs_cfg_t *cfg = malloc(cfg_sz);
    memset(cfg, 0x0, cfg_sz);
    cfg->loop = uv_default_loop();
    cfg->super.op.mkdir.cb = utest_2_asa_mkdir_cb;
    cfg->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
    {
        char *ptr = (char *) cfg + sizeof(asa_op_localfs_cfg_t);
        cfg->super.op.mkdir.path.origin = ptr;
        memcpy(cfg->super.op.mkdir.path.origin, EXPECT_MKDIR_PATH, sizeof(EXPECT_MKDIR_PATH));
        ptr += sizeof(EXPECT_MKDIR_PATH);
        cfg->super.op.mkdir.path.curr_parent = ptr;
    }
    uint8_t allow_exist = 1;
    ASA_RES_CODE result = app_storage_localfs_mkdir((asa_op_base_cfg_t *)cfg, allow_exist);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    // check event loop 5 times for recursively creating parent folders
    uv_run(cfg->loop, UV_RUN_ONCE);
    uv_run(cfg->loop, UV_RUN_ONCE);
    uv_run(cfg->loop, UV_RUN_ONCE);
    uv_run(cfg->loop, UV_RUN_ONCE);
    uv_run(cfg->loop, UV_RUN_ONCE);
    // check event loop one more time for removing a single folder (non-recursive)
    uv_run(cfg->loop, UV_RUN_ONCE);
} // end of storage_localfs_test__mkdir_simple
#undef EXPECT_MKDIR_PATH 
#undef EXPECT_RMDIR_PATH 


#define WR_BUF_SZ  13
#define RD_BUF_SZ  19
#define EXPECT_CB_ARGS_SZ  1
#define EXPECT_FILE_CONTENT  "Liskov substitution principle, interface segregation ," \
    " Dependency inversion principle (figure out the difference from Dependency Injection)"
#define EXPECT_FILE_PATH "tmp/utest_asa_localfs_rwtest.txt"
typedef struct {
    char   *rd_back;
    size_t  wr_ptr;
    size_t  rd_ptr;
    uint8_t wr_end:1;
    uint8_t rd_end:1;
} utest3_usrdata_t;

static void _utest_3_asa_prepare_nxt_write(asa_op_base_cfg_t *cfg, utest3_usrdata_t *usrdata) {
    char *ptr = &(EXPECT_FILE_CONTENT)[ usrdata->wr_ptr ];
    size_t sz = sizeof(EXPECT_FILE_CONTENT) - usrdata->wr_ptr;
    if(sz > cfg->op.write.src_max_nbytes) {
        sz = cfg->op.write.src_max_nbytes;
    }
    memcpy(cfg->op.write.src, ptr, sz);
    cfg->op.write.src_sz = sz;
    assert_that(app_storage_localfs_write(cfg), is_equal_to(ASTORAGE_RESULT_ACCEPT));
} // end of _utest_3_asa_prepare_nxt_write

static void utest_3_asa_read_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nread);

static __attribute__((optimize("O0"))) void utest_3_asa_close_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    free(cfg);
    unlink(EXPECT_FILE_PATH);
} // end of utest_3_asa_close_cb

static __attribute__((optimize("O0"))) void utest_3_asa_write_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nwrite)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    utest3_usrdata_t *usrdata = cfg->cb_args.entries[0];
    usrdata->wr_ptr += nwrite;
    //// if(nwrite < cfg->op.write.src_sz) {
    if(nwrite < WR_BUF_SZ) {
        usrdata->wr_end = 1;
    }
    if(usrdata->wr_end) {
        asa_op_localfs_cfg_t *fs_cfg = (asa_op_localfs_cfg_t *)cfg;
        lseek(fs_cfg->file.file, 0, SEEK_SET);
        assert_that(app_storage_localfs_read(cfg), is_equal_to(ASTORAGE_RESULT_ACCEPT));
    } else {
        _utest_3_asa_prepare_nxt_write(cfg, usrdata);
    }
} // end of utest_3_asa_write_cb

static __attribute__((optimize("O0"))) void utest_3_asa_read_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result, size_t nread)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    utest3_usrdata_t *usrdata = cfg->cb_args.entries[0];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        char *ptr = &usrdata->rd_back[ usrdata->rd_ptr ];
        memcpy(ptr, cfg->op.read.dst, nread);
        usrdata->rd_ptr += nread;
        if(nread < cfg->op.read.dst_sz) {
            usrdata->rd_end = 1;
        }
        if(usrdata->rd_end) {
            result = app_storage_localfs_close(cfg);
        } else {
            result = app_storage_localfs_read(cfg);
        }
    } else {
        result = app_storage_localfs_close(cfg);
    }
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
} // end of utest_3_asa_read_cb

static __attribute__((optimize("O0"))) void utest_3_asa_open_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
    utest3_usrdata_t *usrdata = (utest3_usrdata_t *)cfg->cb_args.entries[0];
    _utest_3_asa_prepare_nxt_write(cfg, usrdata);
} // end of utest_3_asa_open_cb

Ensure(storage_localfs_rwfile_test) {
    char file_content_readback[sizeof(EXPECT_FILE_CONTENT)] = {0};
    utest3_usrdata_t usrdata = {.rd_back = &file_content_readback[0], .wr_ptr = 0,
        .rd_ptr = 0, .wr_end = 0, .rd_end = 0};
    size_t  cb_args_sz = sizeof(void *) * EXPECT_CB_ARGS_SZ;
    size_t  wr_buf_sz = sizeof(char) * WR_BUF_SZ;
    size_t  rd_buf_sz = sizeof(char) * RD_BUF_SZ;
    size_t  total_cfg_sz = sizeof(asa_op_localfs_cfg_t) + wr_buf_sz + rd_buf_sz + cb_args_sz;
    asa_op_localfs_cfg_t *cfg = malloc(total_cfg_sz);
    memset(cfg, 0x0, total_cfg_sz);
    cfg->file = (uv_fs_t){0};
    cfg->loop = uv_default_loop();
    {
        cfg->super.op.close.cb = utest_3_asa_close_cb;
        cfg->super.op.open.cb = utest_3_asa_open_cb;
        cfg->super.op.open.dst_path = EXPECT_FILE_PATH; 
        cfg->super.op.open.mode = S_IRUSR | S_IWUSR;
        cfg->super.op.open.flags = O_CREAT | O_RDWR;
    }
    {
        char *ptr = (char *) cfg + sizeof(asa_op_localfs_cfg_t);
        cfg->super.cb_args.entries = (void **) ptr;
        cfg->super.cb_args.size = EXPECT_CB_ARGS_SZ;
        cfg->super.cb_args.entries[0] = (void *)&usrdata;
        ptr += cb_args_sz;
        cfg->super.op.write.src = ptr;
        cfg->super.op.write.src_sz = 0;
        cfg->super.op.write.src_max_nbytes = wr_buf_sz;
        cfg->super.op.write.offset  = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
        cfg->super.op.write.cb = utest_3_asa_write_cb;
        ptr += wr_buf_sz;
        cfg->super.op.read.dst  = ptr;
        cfg->super.op.read.dst_sz  = rd_buf_sz;
        cfg->super.op.read.dst_max_nbytes  = rd_buf_sz;
        cfg->super.op.read.offset  = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
        cfg->super.op.read.cb = utest_3_asa_read_cb;
    }
    ASA_RES_CODE result = app_storage_localfs_open((asa_op_base_cfg_t *)cfg);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    do {
        uv_run(cfg->loop, UV_RUN_ONCE);
    } while(usrdata.wr_end == 0 || usrdata.rd_end == 0);
    // check the content read back from original one
    assert_that(strcmp(&file_content_readback[0], EXPECT_FILE_CONTENT), is_equal_to(0));
    uv_run(cfg->loop, UV_RUN_ONCE); // close
} // end of storage_localfs_rwfile_test
#undef  WR_BUF_SZ
#undef  RD_BUF_SZ
#undef  EXPECT_CB_ARGS_SZ
#undef  EXPECT_FILE_CONTENT
#undef  EXPECT_FILE_PATH


#define  EXPECT_MKDIR_PREFIX  "tmp/utest/media"
#define  EXPECT_MKDIR_ORIGIN  "beard/chang/tiger"
Ensure(storage_localfs_test__mkdir_prefix) {
    mkdir("tmp/utest", S_IRWXU);
    mkdir(EXPECT_MKDIR_PREFIX, S_IRWXU);
    char  *path_ori = strdup(EXPECT_MKDIR_ORIGIN);
    char  *path_pfx = strdup(EXPECT_MKDIR_PREFIX);
    char  *path_cur_parent = calloc(sizeof(EXPECT_MKDIR_PREFIX) + sizeof(EXPECT_MKDIR_ORIGIN) + 1, sizeof(char));
    asa_op_localfs_cfg_t  mock_asaobj = { .loop=uv_default_loop(), .super={ .op={
        .mkdir={ .cb=utest_4_asa_simple_callback,  .mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR,
            .path={ .origin=path_ori, .prefix=path_pfx, .curr_parent=path_cur_parent}
    }}}};
    uint8_t allow_exist = 1;
    ASA_RES_CODE result = app_storage_localfs_mkdir(&mock_asaobj.super, allow_exist);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest_4_asa_simple_callback, when(result, is_equal_to(ASTORAGE_RESULT_COMPLETE)));
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    assert_that(access(path_cur_parent, F_OK), is_equal_to(0));
    rmdir(EXPECT_MKDIR_PREFIX "/" EXPECT_MKDIR_ORIGIN);
    rmdir(EXPECT_MKDIR_PREFIX "/beard/chang");
    rmdir(EXPECT_MKDIR_PREFIX "/beard");
    rmdir(EXPECT_MKDIR_PREFIX);
    rmdir("tmp/utest");
    free(path_ori);
    free(path_pfx);
    free(path_cur_parent);
} // end of storage_localfs_test__mkdir_prefix
#undef   EXPECT_MKDIR_PREFIX
#undef   EXPECT_MKDIR_ORIGIN


#define  EXPECT_MKDIR_PREFIX  "tmp/utest/media"
#define  EXPECT_MKDIR_ORIGIN  "duplicate"
Ensure(storage_localfs_test__mkdir_dup_error) {
    mkdir("tmp/utest", S_IRWXU);
    mkdir(EXPECT_MKDIR_PREFIX, S_IRWXU);
    mkdir(EXPECT_MKDIR_PREFIX "/" EXPECT_MKDIR_ORIGIN, S_IRWXU);
    char  *path_ori = strdup(EXPECT_MKDIR_ORIGIN);
    char  *path_pfx = strdup(EXPECT_MKDIR_PREFIX);
    char  *path_cur_parent = calloc(sizeof(EXPECT_MKDIR_PREFIX) + sizeof(EXPECT_MKDIR_ORIGIN) + 1, sizeof(char));
    asa_op_localfs_cfg_t  mock_asaobj = { .loop=uv_default_loop(), .super={ .op={
        .mkdir={ .cb=utest_4_asa_simple_callback,  .mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR,
            .path={ .origin=path_ori, .prefix=path_pfx, .curr_parent=path_cur_parent}
    }}}};
    uint8_t allow_exist = 0;
    ASA_RES_CODE result = app_storage_localfs_mkdir(&mock_asaobj.super, allow_exist);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest_4_asa_simple_callback, when(result, is_equal_to(ASTORAGE_RESULT_OS_ERROR)));
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    rmdir(EXPECT_MKDIR_PREFIX "/" EXPECT_MKDIR_ORIGIN);
    rmdir(EXPECT_MKDIR_PREFIX);
    rmdir("tmp/utest");
    free(path_ori);
    free(path_pfx);
    free(path_cur_parent);
} // end of storage_localfs_test__mkdir_dup_error
#undef   EXPECT_MKDIR_PREFIX
#undef   EXPECT_MKDIR_ORIGIN


#define  EXPECT_FILE1_PATH  "./tmp/utest/media/file123"
#define  EXPECT_FILE2_PATH  "./tmp/utest/media/file456"
Ensure(storage_localfs_unlink_test) {
    {
        mkdir("./tmp/utest", S_IRWXU);
        mkdir("./tmp/utest/media", S_IRWXU);
        int fds[2] = {0};
        fds[0] = open(EXPECT_FILE1_PATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        fds[1] = open(EXPECT_FILE2_PATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        close(fds[0]);
        close(fds[1]);
        assert_that(access(EXPECT_FILE1_PATH, F_OK), is_equal_to(0));
        assert_that(access(EXPECT_FILE2_PATH, F_OK), is_equal_to(0));
    }
    ASA_RES_CODE result;
    asa_op_localfs_cfg_t  mock_asaobj = {.loop = uv_default_loop()};
    { // subcase #1
        mock_asaobj.super.op.unlink.path = EXPECT_FILE1_PATH;
        result = app_storage_localfs_unlink(&mock_asaobj.super);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        uv_run(mock_asaobj.loop, UV_RUN_ONCE);
        assert_that(access(EXPECT_FILE1_PATH, F_OK), is_equal_to(-1));
    } { // subcase #2
        mock_asaobj.super.op.unlink.cb = utest_4_asa_simple_callback;
        mock_asaobj.super.op.unlink.path = EXPECT_FILE2_PATH;
        expect(utest_4_asa_simple_callback, when(result, is_equal_to(ASTORAGE_RESULT_COMPLETE)));
        result = app_storage_localfs_unlink(&mock_asaobj.super);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        uv_run(mock_asaobj.loop, UV_RUN_ONCE);
        assert_that(access(EXPECT_FILE2_PATH, F_OK), is_equal_to(-1));
    } { // subcase #3
        mock_asaobj.super.op.unlink.cb = utest_4_asa_simple_callback;
        mock_asaobj.super.op.unlink.path = "./tmp/utest/media/file_not_exist";
        expect(utest_4_asa_simple_callback, when(result, is_equal_to(ASTORAGE_RESULT_OS_ERROR)));
        result = app_storage_localfs_unlink(&mock_asaobj.super);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    } {
        unlink(EXPECT_FILE1_PATH);
        unlink(EXPECT_FILE2_PATH);
        rmdir("./tmp/utest/media");
        rmdir("./tmp/utest");
    }
} // end of storage_localfs_unlink_test
#undef  EXPECT_FILE1_PATH
#undef  EXPECT_FILE2_PATH

static __attribute__((optimize("O0"))) void utest_5_asa_mock_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{ mock(cfg, result); }

static void _storage_localfs__rename_test(const char *old_path, const char *new_path,
       ASA_RES_CODE expect_result) {
    ASA_RES_CODE result;
    asa_op_localfs_cfg_t  mock_asaobj = {.loop = uv_default_loop(), .super={ .op={
        .rename={.cb=utest_5_asa_mock_cb, .path={._old=(char *)old_path,
            ._new=(char *)new_path }}
    }}};
    result =  app_storage_localfs_rename(&mock_asaobj.super);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest_5_asa_mock_cb, when(result, is_equal_to(expect_result)));
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
} // end of _storage_localfs__rename_test

#define  EXPECT_PATH1   "./tmp/utest/media/origin_name"
#define  EXPECT_PATH2   "./tmp/utest/media/modified_name"
#define  INVALID_PATH3  "./tmp/utest/media/nonexist_name"
#define  EXPECT_FILENAME   "dummyfile"
Ensure(storage_localfs_rename_test) {
    mkdir("./tmp/utest", S_IRWXU);
    mkdir("./tmp/utest/media", S_IRWXU);
    mkdir(EXPECT_PATH1, S_IRWXU);
    int fd = open(EXPECT_PATH1 "/" EXPECT_FILENAME, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    close(fd);
    _storage_localfs__rename_test(EXPECT_PATH1, EXPECT_PATH2, ASTORAGE_RESULT_COMPLETE); 
    access(EXPECT_PATH2 "/" EXPECT_FILENAME, F_OK);
    _storage_localfs__rename_test(INVALID_PATH3, EXPECT_PATH2, ASTORAGE_RESULT_OS_ERROR); 
    access(EXPECT_PATH2 "/" EXPECT_FILENAME, F_OK);
    unlink(EXPECT_PATH2 "/" EXPECT_FILENAME);
    rmdir(EXPECT_PATH2);
    rmdir("./tmp/utest/media");
    rmdir("./tmp/utest");
} // end of storage_localfs_rename_test
#undef   EXPECT_FILENAME
#undef   INVALID_PATH3
#undef   EXPECT_PATH1
#undef   EXPECT_PATH2


#define  EXPECT_BASEPATH   "tmp/utest/media/"
#define  EXPECT_DIR_NAMES    {"123folder", "456folder"}
#define  EXPECT_FILE_NAMES   {"a.out", "b.out", "d.out"}
#define  EXPECT_DIR_PATHS    {EXPECT_BASEPATH"123folder", EXPECT_BASEPATH"456folder"}
#define  EXPECT_FILE_PATHS   {EXPECT_BASEPATH"a.out", EXPECT_BASEPATH"b.out", EXPECT_BASEPATH"d.out"}
#define  EXPECT_NUM_DIRS    2
#define  EXPECT_NUM_FILES   3
Ensure(storage_localfs_scandir_test) {
    const char *expect_path_dirs  [EXPECT_NUM_DIRS]  = EXPECT_DIR_PATHS;
    const char *expect_path_files [EXPECT_NUM_FILES] = EXPECT_FILE_PATHS;
    const char *expect_name_dirs  [EXPECT_NUM_DIRS]  = EXPECT_DIR_NAMES;
    const char *expect_name_files [EXPECT_NUM_FILES] = EXPECT_FILE_NAMES;
    int idx = 0;
    { // setup
        mkdir("./tmp/utest", S_IRWXU);
        mkdir(EXPECT_BASEPATH, S_IRWXU);
        for(idx=0; idx < EXPECT_NUM_DIRS; idx++)
            mkdir(expect_path_dirs[idx], S_IRWXU);
        for(idx=0; idx < EXPECT_NUM_FILES; idx++) {
            int fd = open(expect_path_files[idx], O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
            close(fd);
        }
    }
    asa_op_localfs_cfg_t  mock_asaobj = {.loop = uv_default_loop(), .super={ .op={
        .scandir={.cb=utest_5_asa_mock_cb, .path=EXPECT_BASEPATH }}}};
    ASA_RES_CODE result = app_storage_localfs_scandir (&mock_asaobj.super);
    assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
    expect(utest_5_asa_mock_cb, when(result, is_equal_to(ASTORAGE_RESULT_COMPLETE)));
    uv_run(mock_asaobj.loop, UV_RUN_ONCE);
    int num_entries_cond = mock_asaobj.file.result >= (EXPECT_NUM_DIRS + EXPECT_NUM_FILES);
    assert_that(num_entries_cond, is_equal_to(1));
    while(1) {
        asa_dirent_t  e = {0};
        result = app_storage_localfs_scandir_next (&mock_asaobj.super,  &e);
        assert_that(result, is_not_equal_to(ASTORAGE_RESULT_OS_ERROR));
        assert_that(result, is_not_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_EOF_SCAN) {
            break;
        } else if(result == ASTORAGE_RESULT_COMPLETE) {
            assert_that(e.name, is_not_null);
            switch(e.type) { // tedious... since the order might change
                case ASA_DIRENT_DIR:
                    switch(e.name[0]) {
                        case '1':
                            assert_that(e.name, is_equal_to_string(expect_name_dirs[0]));
                            break;
                        case '4':
                            assert_that(e.name, is_equal_to_string(expect_name_dirs[1]));
                            break;
                    }
                    break;
                case ASA_DIRENT_FILE:
                    switch(e.name[0]) {
                        case 'a':
                            assert_that(e.name, is_equal_to_string(expect_name_files[0]));
                            break;
                        case 'b':
                            assert_that(e.name, is_equal_to_string(expect_name_files[1]));
                            break;
                        case 'd':
                            assert_that(e.name, is_equal_to_string(expect_name_files[2]));
                            break;
                    }
                    break;
                default:
                    break;
            } // end of swtich statement
        } // end of entry read
    } // end of loop
    { // tear down
        for(idx=0; idx < EXPECT_NUM_DIRS; idx++)
            rmdir(expect_path_dirs[idx]);
        for(idx=0; idx < EXPECT_NUM_FILES; idx++)
            unlink(expect_path_files[idx]);
        rmdir(EXPECT_BASEPATH);
        rmdir("./tmp/utest");
    }
} // end of storage_localfs_scandir_test
#undef   EXPECT_DIR_PATHS  
#undef   EXPECT_FILE_PATHS 
#undef   EXPECT_DIR_NAMES
#undef   EXPECT_FILE_NAMES 
#undef   EXPECT_NUM_DIRS   
#undef   EXPECT_NUM_FILES
#undef   EXPECT_BASEPATH


TestSuite *app_storage_localfs_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, storage_localfs_openfile_test);
    add_test(suite, storage_localfs_test__mkdir_simple);
    add_test(suite, storage_localfs_test__mkdir_prefix);
    add_test(suite, storage_localfs_test__mkdir_dup_error);
    add_test(suite, storage_localfs_rwfile_test);
    add_test(suite, storage_localfs_unlink_test);
    add_test(suite, storage_localfs_rename_test);
    add_test(suite, storage_localfs_scandir_test);
    return suite;
} // end of app_storage_localfs_tests
