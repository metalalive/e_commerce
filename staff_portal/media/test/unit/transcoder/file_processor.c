#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <uv.h>

#include "storage/localfs.h"
#include "transcoder/file_processor.h"

#define  LOCAL_TMPBUF_BASEPATH  "tmp/buffer/media/test"

#define  UNITTEST_FOLDER_NAME   "utest"
#define  FILECHUNK_FOLDER_NAME  "mock_fchunk"
#define  FULLPATH1   LOCAL_TMPBUF_BASEPATH "/" UNITTEST_FOLDER_NAME
#define  FULLPATH2   FULLPATH1 "/"  FILECHUNK_FOLDER_NAME
#define  FILEPATH_TEMPLATE  FULLPATH2  "/%d"  
#define  NUM_FILECHUNKS  5
#define  FILECHUNK_CONTENT(name)  const char *(name)[NUM_FILECHUNKS] = { \
    "9824th3gw", "cp23724rT@#@RWG", "(u#aa&K%j:A", "93rjsie", "43hGfm"};

#define  RENDER_FILECHUNK_PATH(path_template, idx) \
    size_t filepath_sz = strlen(path_template) + 1; \
    char filepath[filepath_sz]; \
    int nwrite = snprintf(&filepath[0], filepath_sz, path_template, idx+1); \
    filepath[nwrite] = 0x0;


static __attribute__((optimize("O0"))) void  utest_init_transcoder_srcfile_chunk(void)
{
    int idx = 0;
    mkdir(FULLPATH1, S_IRWXU);
    mkdir(FULLPATH2, S_IRWXU);
    FILECHUNK_CONTENT(f_content);
    for(idx=0; idx < NUM_FILECHUNKS; idx++) {
        RENDER_FILECHUNK_PATH(FILEPATH_TEMPLATE, idx);
        int fd = open(&filepath[0], O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        write(fd, f_content[idx], strlen(f_content[idx]));
        close(fd);
    }
} // end of utest_init_transcoder_srcfile_chunk

static __attribute__((optimize("O0"))) void  utest_deinit_transcoder_srcfile_chunk(void)
{
    int idx = 0;
    for(idx=0; idx < NUM_FILECHUNKS; idx++) {
        RENDER_FILECHUNK_PATH(FILEPATH_TEMPLATE, idx);
        unlink(&filepath[0]);
    }
    rmdir(FULLPATH2);
    rmdir(FULLPATH1);
}


static void  transcoder_utest__closefile_callback (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
     assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
} // end of transcoder_utest__closefile_callback


static void  transcoder_utest__openfile_callback (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
#define  READ_BUFFER_SZ 128
     int stderr_fd = 2;
     char rd_buf[READ_BUFFER_SZ] = {0};
     const char *expect_content = cfg->cb_args.entries[1];
     asa_op_localfs_cfg_t  *localfile_handle = (asa_op_localfs_cfg_t *) cfg;
     assert_that(result, is_equal_to(ASTORAGE_RESULT_COMPLETE));
     if(result != ASTORAGE_RESULT_COMPLETE)
         return;
     assert_that(localfile_handle->file.file, is_greater_than(stderr_fd));
     if(localfile_handle->file.file <= stderr_fd)
         return;
     size_t  nread = read(localfile_handle->file.file, &rd_buf[0], READ_BUFFER_SZ);
     assert_that(nread, is_greater_than(0));
     assert_that(&rd_buf[0], is_equal_to_string(expect_content));
#undef  READ_BUFFER_SZ
} // end of transcoder_utest__openfile_callback

static void  transcoder_utest__openfile_error_callback (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
     assert_that(result, is_equal_to(ASTORAGE_RESULT_OS_ERROR));
}


static json_t *transcoder_utest__gen_atfp_spec(const char **expect_f_content, size_t num_files)
{
    int idx = 0;
    json_t  *spec = json_object();
    json_t  *parts_size = json_array();
    json_object_set_new(spec, "parts_size", parts_size);
    for(idx = 0; idx < num_files; idx++) {
        size_t  content_size = strlen(expect_f_content[idx]);
        json_array_append_new(parts_size, json_integer(content_size));
    }
    return spec;
}  // end of transcoder_utest__gen_atfp_spec


Ensure(transcoder_test__get_atfp_object) {
    atfp_t *file_processor = app_transcoder_file_processor("audio/wmv");
    assert_that(file_processor, is_equal_to(NULL));
    file_processor = app_transcoder_file_processor("video/mp4");
    assert_that(file_processor, is_not_equal_to(NULL));
    assert_that(file_processor->ops, is_not_equal_to(NULL));
    assert_that(file_processor->ops->init,   is_not_equal_to(NULL));
    assert_that(file_processor->ops->deinit, is_not_equal_to(NULL));
    assert_that(file_processor->ops->processing,   is_not_equal_to(NULL));
    assert_that(file_processor->ops->get_obj_size, is_not_equal_to(NULL));
    file_processor->ops->deinit(file_processor);
} // end of transcoder_test__get_atfp_object

Ensure(transcoder_test__open_srcfile_chunk_ok) {
    void *asacfg_usr_args[2] = {0};
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  asa_cfg = {.loop=loop, .super={.cb_args={.size=2, .entries=asacfg_usr_args}}};
    asa_cfg_t   storage = {.ops={.fn_open=app_storage_localfs_open}};
    ASA_RES_CODE result ;
    FILECHUNK_CONTENT(expect_f_content);
    utest_init_transcoder_srcfile_chunk();
    asa_cfg.super.cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG] = NULL;
    for(int chunk_seq = 0; chunk_seq < NUM_FILECHUNKS; chunk_seq++) {
        asa_cfg.super.cb_args.entries[1] = (void *) expect_f_content[chunk_seq];
        // in thhis application, sequential numbers in a set of ffile chunks start from one.
        result = atfp_open_srcfile_chunk( &asa_cfg.super, &storage, FULLPATH2,
                chunk_seq + 1, transcoder_utest__openfile_callback );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_ACCEPT) {
            uv_run(loop, UV_RUN_ONCE);
            asa_cfg.super.op.close.cb = transcoder_utest__closefile_callback;
            app_storage_localfs_close(&asa_cfg.super);
            uv_run(loop, UV_RUN_ONCE);
        }
    }
    utest_deinit_transcoder_srcfile_chunk();
} // end of transcoder_test__open_srcfile_chunk_ok


Ensure(transcoder_test__open_srcfile_chunk_error) {
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  asa_cfg = {.loop=loop, .super={.cb_args={.size=0, .entries=NULL}}};
    asa_cfg_t   storage = {.ops={.fn_open=app_storage_localfs_open}};
    {
        int chunk_seq = NUM_FILECHUNKS + 1;
        ASA_RES_CODE result = atfp_open_srcfile_chunk( &asa_cfg.super, &storage, FULLPATH2,
                chunk_seq + 1, transcoder_utest__openfile_error_callback );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_ACCEPT) 
            uv_run(loop, UV_RUN_ONCE);
    }
} // end of transcoder_test__open_srcfile_chunk_error


Ensure(transcoder_test__switch_srcfile_chunk_ok) {
    FILECHUNK_CONTENT(expect_f_content);
    void *asacfg_usr_args[2] = {0};
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  asa_cfg = {.loop=loop, .super={.cb_args={.size=2, .entries=asacfg_usr_args}}};
    asa_cfg_t   storage = {.ops={.fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close}};
    const char *atfp_src_basepath = FULLPATH2;
    json_t  *spec  = transcoder_utest__gen_atfp_spec(expect_f_content, NUM_FILECHUNKS);
    atfp_t   processor = {.data={.spec=spec, .src={.basepath=atfp_src_basepath,
        .storage={.handle=&asa_cfg.super, .config=&storage}}}};
    asa_cfg.super.cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG] = &processor;
    utest_init_transcoder_srcfile_chunk();
    int idx = 0;
    { // open the first chunk
        int chunk_seq = 0;
        asa_cfg.super.cb_args.entries[1] = (void *) expect_f_content[chunk_seq];
        ASA_RES_CODE result = atfp_open_srcfile_chunk( &asa_cfg.super, &storage, atfp_src_basepath,
                chunk_seq + 1, transcoder_utest__openfile_callback );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        uv_run(loop, UV_RUN_ONCE);
    } { // jump to specific chunk
#define  NUM_SWITCHES  8
        struct {
            int num;
            const char *expect_content;
        } switch_chunk_seq[NUM_SWITCHES] = {
            { 3, expect_f_content[3]},
            { 0, expect_f_content[0]},
            {-1, expect_f_content[1]},
            { 4, expect_f_content[4]},
            { 2, expect_f_content[2]},
            { 1, expect_f_content[1]},
            {-1, expect_f_content[2]},
            {-1, expect_f_content[3]},
        };
        for(idx = 0; idx < NUM_SWITCHES; idx++) {
            asa_cfg.super.cb_args.entries[1] = (void *) switch_chunk_seq[idx].expect_content;
            ASA_RES_CODE result = atfp_switch_to_srcfile_chunk(&processor, switch_chunk_seq[idx].num,
                transcoder_utest__openfile_callback);
            assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
            uv_run(loop, UV_RUN_ONCE); // invoke file-close callback
            uv_run(loop, UV_RUN_ONCE); // invoke file-open  callback
        }
#undef  NUM_SWITCHES
    }
    json_decref(spec);
    utest_deinit_transcoder_srcfile_chunk();
} // end of transcoder_test__switch_srcfile_chunk_ok


Ensure(transcoder_test__estimate_src_filechunk_idx) {
    FILECHUNK_CONTENT(expect_f_content);
    json_t  *spec  = transcoder_utest__gen_atfp_spec(expect_f_content, NUM_FILECHUNKS);
    { // subcase #1
        int chunk_idx_start = 0;
        size_t  offset = 2;
        size_t  pos = strlen(expect_f_content[0]) + offset;
        int actual_fchunk_idx = atfp_estimate_src_filechunk_idx(spec, chunk_idx_start, &pos);
        assert_that(actual_fchunk_idx, is_equal_to(1));
        assert_that(pos, is_equal_to(offset));
    }
    { // subcase #2
        int chunk_idx_start = 0;
        size_t  offset = 1;
        size_t  pos = strlen(expect_f_content[0]) + strlen(expect_f_content[1]) +
                  strlen(expect_f_content[2]) + offset;
        int actual_fchunk_idx = atfp_estimate_src_filechunk_idx(spec, chunk_idx_start, &pos);
        assert_that(actual_fchunk_idx, is_equal_to(3));
        assert_that(pos, is_equal_to(offset));
    }
    { // subcase #3
        int chunk_idx_start = 3;
        size_t  offset = 1;
        size_t  pos = strlen(expect_f_content[3]) + offset;
        int actual_fchunk_idx = atfp_estimate_src_filechunk_idx(spec, chunk_idx_start, &pos);
        assert_that(actual_fchunk_idx, is_equal_to(4));
        assert_that(pos, is_equal_to(offset));
    }
    { // subcase #4
        int chunk_idx_start = NUM_FILECHUNKS - 1;
        size_t  offset = 3;
        size_t  pos = strlen(expect_f_content[chunk_idx_start]) + offset;
        int actual_fchunk_idx = atfp_estimate_src_filechunk_idx(spec, chunk_idx_start, &pos);
        assert_that(actual_fchunk_idx, is_equal_to(-1));
    }
    json_decref(spec);
} // end of transcoder_test__estimate_src_filechunk_idx


TestSuite *app_transcoder_file_processor_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, transcoder_test__get_atfp_object);
    add_test(suite, transcoder_test__open_srcfile_chunk_ok);
    add_test(suite, transcoder_test__open_srcfile_chunk_error);
    add_test(suite, transcoder_test__switch_srcfile_chunk_ok);
    add_test(suite, transcoder_test__estimate_src_filechunk_idx);
    return suite;
}
#undef  NUM_FILECHUNKS
#undef  FILEPATH_TEMPLATE
#undef  FULLPATH1
#undef  FULLPATH2
#undef  UNITTEST_FOLDER_NAME 
#undef  FILECHUNK_FOLDER_NAME
