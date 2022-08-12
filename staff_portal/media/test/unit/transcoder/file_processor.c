#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
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

static __attribute__((optimize("O0"))) void  utest_deinit_transcoder_srcfile_chunk(asa_op_base_cfg_t *asa_cfg)
{
    int idx = 0;
    if(asa_cfg->op.open.dst_path) {
        free(asa_cfg->op.open.dst_path);
        asa_cfg->op.open.dst_path = NULL;
    }
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
     size_t  expect_content_idx = cfg->cb_args.size - 1;
     const char *expect_content = cfg->cb_args.entries[expect_content_idx];
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


#include <libavcodec/avcodec.h>
#include <libavutil/frame.h>
// TODO, refactor ?
Ensure(transcoder_test__get_atfp_object) {
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)
    atfp_t *mock_fp = app_transcoder_file_processor("audio/wmv");
    assert_that(mock_fp, is_equal_to(NULL));
    mock_fp = app_transcoder_file_processor("video/mp4");
    assert_that(mock_fp, is_not_equal_to(NULL));
    assert_that(mock_fp->ops, is_not_equal_to(NULL));
    if(mock_fp && mock_fp->ops) {
        assert_that(mock_fp->ops->init,   is_not_equal_to(NULL));
        assert_that(mock_fp->ops->deinit, is_not_equal_to(NULL));
        assert_that(mock_fp->ops->processing,   is_not_equal_to(NULL));
        assert_that(mock_fp->ops->has_done_processing, is_not_equal_to(NULL));
        assert_that(mock_fp->ops->label_match, is_not_equal_to(NULL));
        assert_that(mock_fp->ops->instantiate, is_not_equal_to(NULL));
        atfp_asa_map_t    mock_map = {0};
        void  *cb_args_entries[NUM_CB_ARGS_ASAOBJ] = {mock_fp, &mock_map};
        asa_op_base_cfg_t mock_asaobj = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ,
            .entries=cb_args_entries}};
        mock_fp->data.storage.handle = &mock_asaobj;
        expect(av_packet_unref);
        expect(av_frame_unref);
        mock_fp->ops->deinit(mock_fp);
    }
#undef  NUM_CB_ARGS_ASAOBJ
} // end of transcoder_test__get_atfp_object

Ensure(transcoder_test__open_srcfile_chunk_ok) {
#define  UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG   (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ  (UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG + 1)
    void *asaobj_usr_args[NUM_CB_ARGS_ASAOBJ] = {0};
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  asaobj = {.loop=loop, .super={.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asaobj_usr_args}}};
    asa_cfg_t   storage = {.ops={.fn_open=app_storage_localfs_open}};
    ASA_RES_CODE result ;
    FILECHUNK_CONTENT(expect_f_content);
    utest_init_transcoder_srcfile_chunk();
    asaobj.super.cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = NULL;
    for(int chunk_seq = 0; chunk_seq < NUM_FILECHUNKS; chunk_seq++) {
        asaobj.super.cb_args.entries[UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG] = (void *) expect_f_content[chunk_seq];
        // in thhis application, sequential numbers in a set of ffile chunks start from one.
        result = atfp_open_srcfile_chunk( &asaobj.super, &storage, FULLPATH2,
                chunk_seq + 1, transcoder_utest__openfile_callback );
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        if(result == ASTORAGE_RESULT_ACCEPT) {
            uv_run(loop, UV_RUN_ONCE);
            asaobj.super.op.close.cb = transcoder_utest__closefile_callback;
            app_storage_localfs_close(&asaobj.super);
            uv_run(loop, UV_RUN_ONCE);
        }
    }
    utest_deinit_transcoder_srcfile_chunk(&asaobj.super);
#undef  NUM_CB_ARGS_ASAOBJ
#undef  UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG
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
        free(asa_cfg.super.op.open.dst_path);
    }
} // end of transcoder_test__open_srcfile_chunk_error


Ensure(transcoder_test__switch_srcfile_chunk_ok) {
#define  UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG   (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ  (UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG + 1)
    FILECHUNK_CONTENT(expect_f_content);
    void *asaobj_usr_args[NUM_CB_ARGS_ASAOBJ] = {0};
    uv_loop_t  *loop = uv_default_loop();
    asa_op_localfs_cfg_t  asaobj = {.loop=loop, .super={.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asaobj_usr_args}}};
    asa_cfg_t   storage = {.ops={.fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close}};
    const char *atfp_src_basepath = FULLPATH2;
    json_t  *spec  = transcoder_utest__gen_atfp_spec(expect_f_content, NUM_FILECHUNKS);
    atfp_t mock_fp = {.data={.spec=spec,  .storage={.basepath=atfp_src_basepath,
        .handle=&asaobj.super, .config=&storage}}};
    asaobj.super.cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp;
    utest_init_transcoder_srcfile_chunk();
    int idx = 0;
    { // open the first chunk
        int chunk_seq = 0;
        asaobj.super.cb_args.entries[UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG] = (void *) expect_f_content[chunk_seq];
        ASA_RES_CODE result = atfp_open_srcfile_chunk( &asaobj.super, &storage, atfp_src_basepath,
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
            asaobj.super.cb_args.entries[UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG] = (void *) switch_chunk_seq[idx].expect_content;
            ASA_RES_CODE result = atfp_switch_to_srcfile_chunk(&mock_fp, switch_chunk_seq[idx].num,
                transcoder_utest__openfile_callback);
            assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
            uv_run(loop, UV_RUN_ONCE); // invoke file-close callback
            uv_run(loop, UV_RUN_ONCE); // invoke file-open  callback
        }
#undef  NUM_SWITCHES
    }
    json_decref(spec);
    utest_deinit_transcoder_srcfile_chunk(&asaobj.super);
#undef  NUM_CB_ARGS_ASAOBJ
#undef  UTEST_EXPECT_CONTENT_INDEX__IN_ASA_USRARG
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


Ensure(transcoder_test__asamap_basic_ok) {
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)
    void *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0};
    void *asalocal_cb_args[NUM_CB_ARGS_ASAOBJ] = {0};
    asa_op_base_cfg_t     mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asasrc_cb_args}};
    asa_op_localfs_cfg_t  mock_asa_local = {.super={.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asalocal_cb_args}}};
    uint8_t num_dst = 0;
    atfp_asa_map_t  *map = atfp_asa_map_init(num_dst);
    atfp_asa_map_t  *readback_map = NULL;
    assert_that(map, is_not_equal_to(NULL));
    if(map) {
        readback_map = mock_asa_src.cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        assert_that(readback_map , is_equal_to(NULL));
        atfp_asa_map_set_source(map, &mock_asa_src);
        atfp_asa_map_set_localtmp(map, &mock_asa_local);
        readback_map = mock_asa_src.cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        assert_that(readback_map , is_equal_to(map));
        readback_map = mock_asa_local.super.cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        assert_that(readback_map , is_equal_to(map));

        asa_op_localfs_cfg_t  *readback_asa_local = atfp_asa_map_get_localtmp(map);
        asa_op_base_cfg_t     *readback_asa_src = atfp_asa_map_get_source(map);
        assert_that(readback_asa_local, is_equal_to(&mock_asa_local));
        assert_that(readback_asa_src  , is_equal_to(&mock_asa_src));
        atfp_asa_map_deinit(map);
    }
#undef  NUM_CB_ARGS_ASAOBJ
} // end of transcoder_test__asamap_basic_ok


Ensure(transcoder_test__asamap_destination_ok) {
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_DESTINATIONS  7
    void *asadsts_cb_args[NUM_DESTINATIONS][NUM_CB_ARGS_ASAOBJ] = {0};
    asa_op_base_cfg_t   mock_asa_dsts[NUM_DESTINATIONS] = {0}, *readback_asa_dst = NULL;
    atfp_asa_map_t  *map = atfp_asa_map_init(NUM_DESTINATIONS);
    atfp_asa_map_t  *readback_map = NULL;
    int idx = 0;
    assert_that(map, is_not_equal_to(NULL));
    if(!map) { return; }
    for(idx=0; idx < NUM_DESTINATIONS; idx++) {
        mock_asa_dsts[idx].cb_args.size = NUM_CB_ARGS_ASAOBJ;
        mock_asa_dsts[idx].cb_args.entries = asadsts_cb_args[idx];
    }
    { // subcase 1
        for(idx = 0; idx < NUM_DESTINATIONS; idx++) {
            uint8_t err = atfp_asa_map_add_destination(map, &mock_asa_dsts[idx]);
            assert_that(err, is_equal_to(0));
        } // no avilable entry for new destination
        uint8_t err = atfp_asa_map_add_destination(map, &mock_asa_dsts[0]);
        assert_that(err, is_not_equal_to(0));
    } { // subcase 2
        idx = 0;
        atfp_asa_map_reset_dst_iteration(map);
        while((readback_asa_dst = atfp_asa_map_iterate_destination(map))) {
            readback_map = readback_asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
            assert_that(readback_map, is_equal_to(map));
            assert_that(readback_asa_dst, is_equal_to(&mock_asa_dsts[idx++]));
        }
        readback_asa_dst = atfp_asa_map_iterate_destination(map);
        assert_that(readback_asa_dst, is_equal_to(NULL));
    } { // subcase 3
        asa_op_base_cfg_t  *working_asa_dst = &mock_asa_dsts[2];
        assert_that(atfp_asa_map_all_dst_stopped(map), is_equal_to(1));
        atfp_asa_map_dst_start_working(map, working_asa_dst);
        assert_that(atfp_asa_map_all_dst_stopped(map), is_equal_to(0));
        atfp_asa_map_dst_stop_working(map, working_asa_dst);
        assert_that(atfp_asa_map_all_dst_stopped(map), is_equal_to(1));
    } { // subcase 4
        asa_op_base_cfg_t  *deleting_asa_dst = &mock_asa_dsts[3];
        asa_op_base_cfg_t  *list_after_deleted[NUM_DESTINATIONS - 1] = {
            &mock_asa_dsts[0], &mock_asa_dsts[1], &mock_asa_dsts[2],
            &mock_asa_dsts[4], &mock_asa_dsts[5], &mock_asa_dsts[6], 
        };
        uint8_t err = atfp_asa_map_remove_destination(map, deleting_asa_dst);
        assert_that(err, is_equal_to(0));
        atfp_asa_map_reset_dst_iteration(map);
        for(idx = 0; idx < NUM_DESTINATIONS - 1; idx++)
             assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(list_after_deleted[idx]));
        assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(NULL));
    } { // subcase 5
        asa_op_base_cfg_t  *deleting_asa_dst = &mock_asa_dsts[6];
        asa_op_base_cfg_t  *list_after_deleted[NUM_DESTINATIONS - 2] = {
            &mock_asa_dsts[0], &mock_asa_dsts[1], &mock_asa_dsts[2],
            &mock_asa_dsts[4], &mock_asa_dsts[5],
        };
        uint8_t err = atfp_asa_map_remove_destination(map, deleting_asa_dst);
        assert_that(err, is_equal_to(0));
        atfp_asa_map_reset_dst_iteration(map);
        for(idx = 0; idx < NUM_DESTINATIONS - 2; idx++)
             assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(list_after_deleted[idx]));
        assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(NULL));
    } { // subcase 6
        asa_op_base_cfg_t  *deleting_asa_dst = &mock_asa_dsts[0];
        asa_op_base_cfg_t  *list_after_deleted[NUM_DESTINATIONS - 3] = {
            &mock_asa_dsts[1], &mock_asa_dsts[2], &mock_asa_dsts[4], &mock_asa_dsts[5],
        };
        uint8_t err = atfp_asa_map_remove_destination(map, deleting_asa_dst);
        assert_that(err, is_equal_to(0));
        atfp_asa_map_reset_dst_iteration(map);
        for(idx = 0; idx < NUM_DESTINATIONS - 3; idx++)
             assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(list_after_deleted[idx]));
        assert_that(atfp_asa_map_iterate_destination(map), is_equal_to(NULL));
    }
    atfp_asa_map_deinit(map);
#undef  NUM_DESTINATIONS
#undef  NUM_CB_ARGS_ASAOBJ
} // end of transcoder_test__asamap_destination_ok


TestSuite *app_transcoder_file_processor_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, transcoder_test__get_atfp_object);
    add_test(suite, transcoder_test__open_srcfile_chunk_ok);
    add_test(suite, transcoder_test__open_srcfile_chunk_error);
    add_test(suite, transcoder_test__switch_srcfile_chunk_ok);
    add_test(suite, transcoder_test__estimate_src_filechunk_idx);
    add_test(suite, transcoder_test__asamap_basic_ok);
    add_test(suite, transcoder_test__asamap_destination_ok);
    return suite;
}
#undef  NUM_FILECHUNKS
#undef  FILEPATH_TEMPLATE
#undef  FULLPATH1
#undef  FULLPATH2
#undef  UNITTEST_FOLDER_NAME 
#undef  FILECHUNK_FOLDER_NAME
