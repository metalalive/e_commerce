#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "transcoder/video/hls.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASALOCAL_BASEPATH   UTEST_FILE_BASEPATH "/asalocal"
#define  UTEST_ASADST_BASEPATH     UTEST_FILE_BASEPATH "/asadst"
#define  UTEST_STRINGIFY(x)  #x

#define  DONE_FLAG_INDEX__IN_ASA_USRARG     (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ  (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define  WR_BUF_MAX_SZ   10

extern const atfp_ops_entry_t  atfp_ops_video_hls;

static  ASA_RES_CODE utest_hls__storage_fn_close (asa_op_base_cfg_t *asaobj)
{
    ASA_RES_CODE  cb_result;
    ASA_RES_CODE *cb_result_ptr = &cb_result;
    mock(asaobj, cb_result_ptr);
    {
        atfp_t *processor = (atfp_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        processor->transfer.dst.flags.asaremote_open = 0;
    }
    asaobj->op.close.cb(asaobj, cb_result);
    return  ASTORAGE_RESULT_ACCEPT;
}

static int  utest_hls__avfilter_init (atfp_hls_t *hlsproc)
{
    int err = (int) mock(hlsproc);
    if(err) {
        json_t *err_info = hlsproc->super.data.error;
        json_object_set_new(err_info, "transcoder", json_string("got some error"));
    }
    return err;
}

static int  utest_hls__avctx_init (atfp_hls_t *hlsproc)
{ return (int) mock(hlsproc); }

static void  utest_hls__avctx_deinit(atfp_hls_t *hlsproc)
{ mock(hlsproc); }

#define  DEINIT_IF_EXISTS(var) \
    if(var) { \
        free((void *)var); \
        (var) = NULL; \
    }


static void utest_hls__asa_dst_final_dealloc (asa_op_base_cfg_t *asaobj) {
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    DEINIT_IF_EXISTS(processor->data.version);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.prefix);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.origin);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.curr_parent);
    DEINIT_IF_EXISTS(asaobj->op.open.dst_path);
    DEINIT_IF_EXISTS(asaobj);
}

static  uint8_t  utest_hls__src_has_done_processing (atfp_t *processor)
{ return  (uint8_t)mock(processor); }

static  int  utest_hls__filter_decoded_frame (atfp_av_ctx_t *src, atfp_av_ctx_t *dst)
{ return  (int) mock(src, dst); }

static  int  utest_hls__encode_filtered_frame (atfp_av_ctx_t *dst)
{ return  (int) mock(dst); }

static  int  utest_hls__write_encoded_packet (atfp_av_ctx_t *dst)
{ return  (int) mock(dst); }

static  int  utest_hls__flush_filtered_frames (atfp_av_ctx_t *src, atfp_av_ctx_t *dst)
{ return  (int) mock(src, dst); }

static  int  utest_hls__flush_encoded_frames (atfp_av_ctx_t *dst)
{ return  (int) mock(dst); }

static  int  utest_hls__final_write_encoded_packet (atfp_av_ctx_t *dst)
{ return  (int) mock(dst); }

static  ASA_RES_CODE  utest_hls__move_localfile_to_dst (atfp_hls_t *hlsproc)
{
    ASA_RES_CODE result = (ASA_RES_CODE) mock(hlsproc);
    if(result == ASTORAGE_RESULT_ACCEPT) {
        atfp_t *processor = &hlsproc->super;
        processor -> data.callback(processor);
    } // assume this function completes successfully
    return  result;
}

static  uint8_t  utest_hls__has_done_flush_filter (atfp_av_ctx_t *src, atfp_av_ctx_t *dst)
{ return  (uint8_t) mock(src, dst); }

static  uint8_t  utest_hls__has_done_flush_encoder (atfp_av_ctx_t *dst)
{ return  (uint8_t) mock(dst); }

static void  utest_hls_done_usr_cb(atfp_t *processor)
{
    mock(processor);
    if(!processor)
        return;
    asa_op_base_cfg_t  *asa_dst = processor->data.storage.handle;
    if(asa_dst && asa_dst->cb_args.entries) {
        uint8_t *done_flag = asa_dst->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flag)
            *done_flag = 1;
    }
} // end of utest_hls_done_usr_cb

#define ATFP_HLS_TEST__INIT__SETUP \
    char mock_wr_buf[WR_BUF_MAX_SZ] = {0}; \
    uv_loop_t *loop  = uv_default_loop(); \
    atfp_asa_map_t  mock_map = {0}; \
    uint8_t done_flag = 0; \
    void  *asa_dst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_cfg_t  mock_storage_cfg = {.ops={.fn_close=utest_hls__storage_fn_close}}; \
    asa_op_localfs_cfg_t  mock_asa_local_srcside = { .loop=loop, \
        .super={.op={.mkdir={.path={.origin=UTEST_ASALOCAL_BASEPATH}}}} \
    }; \
    asa_op_base_cfg_t  *mock_asa_dst = calloc(1, sizeof(asa_op_base_cfg_t)); \
    *mock_asa_dst = (asa_op_base_cfg_t) { \
        .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_dst_cb_args}, \
        .op={ \
            .mkdir={.path={.origin=strdup(UTEST_ASADST_BASEPATH)}}, \
            .write={.src_max_nbytes=WR_BUF_MAX_SZ, .src=&mock_wr_buf[0]} \
        }, .storage=&mock_storage_cfg, .deinit=utest_hls__asa_dst_final_dealloc, \
    }; \
    json_t *mock_spec = json_object(); \
    json_t *mock_err_info = json_object(); \
    atfp_hls_t *mock_fp = (atfp_hls_t *) atfp_ops_video_hls.ops.instantiate(); \
    mock_fp->super.data = (atfp_data_t) {.callback=utest_hls_done_usr_cb, .spec=mock_spec, \
            .error=mock_err_info,  .storage={.handle=mock_asa_dst}, .version=strdup("Nh")  }; \
    mock_fp->internal.op.avctx_init    = utest_hls__avctx_init; \
    mock_fp->internal.op.avfilter_init = utest_hls__avfilter_init; \
    mock_fp->internal.op.avctx_deinit  = utest_hls__avctx_deinit; \
    atfp_asa_map_set_localtmp(&mock_map, &mock_asa_local_srcside); \
    asa_dst_cb_args[ATFP_INDEX__IN_ASA_USRARG] = mock_fp; \
    asa_dst_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = &mock_map; \
    asa_dst_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    const char *created_path = UTEST_ASALOCAL_BASEPATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME "/" "Nh";


#define ATFP_HLS_TEST__INIT__TEARDOWN \
    json_decref(mock_spec); \
    rmdir(created_path); \
    rmdir(UTEST_ASALOCAL_BASEPATH "/" ATFP__TEMP_TRANSCODING_FOLDER_NAME); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH);


Ensure(atfp_hls_test__init_deinit__ok) {
    ATFP_HLS_TEST__INIT__SETUP;
    { // init
        atfp_ops_video_hls.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        expect(utest_hls__avctx_init,    will_return(0), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls__avfilter_init, will_return(0), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(mock_fp)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        assert_that(access(created_path, F_OK), is_equal_to(0));
        assert_that(mock_asa_dst->op.mkdir.path.origin, is_not_equal_to(NULL));
        assert_that(mock_fp->asa_local.super.op.mkdir.path.origin, is_not_equal_to(NULL));
        assert_that(mock_fp->internal.segment.fullpath._asa_local.data , is_not_equal_to(NULL));
        assert_that(mock_fp->internal.segment.fullpath._asa_dst.data   , is_not_equal_to(NULL));
        assert_that(mock_fp->internal.segment.fullpath._asa_local.sz , is_greater_than(0));
        assert_that(mock_fp->internal.segment.fullpath._asa_dst.sz   , is_greater_than(0));
        { // memory corruption test
            char *buf = mock_fp->internal.segment.fullpath._asa_dst.data;
            size_t bufsz = mock_fp->internal.segment.fullpath._asa_dst.sz;
            char *basepath = mock_asa_dst->op.mkdir.path.origin;
            const char *filename = HLS_FMP4_FILENAME;
            memset(buf, 0x0, sizeof(char) * bufsz);
            strncat(buf, basepath, strlen(basepath));
            strncat(buf, "/", 1);
            strncat(buf, filename, strlen(filename));
        }
    } { // de-init
        expect(utest_hls__avctx_deinit,  when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(NULL)));
        uint8_t still_ongoing = atfp_ops_video_hls.ops.deinit(&mock_fp->super);
        assert_that(still_ongoing, is_equal_to(0));
    }
    ATFP_HLS_TEST__INIT__TEARDOWN;
} // end of atfp_hls_test__init_deinit__ok


Ensure(atfp_hls_test__init_avctx_error) {
    ATFP_HLS_TEST__INIT__SETUP;
    { // init
        atfp_ops_video_hls.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        expect(utest_hls__avctx_init,    will_return(0), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls__avfilter_init, will_return(-1), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(mock_fp)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(1));
    } { // de-init
        json_object_clear(mock_err_info);
        expect(utest_hls__avctx_deinit,  when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(NULL)));
        uint8_t still_ongoing = atfp_ops_video_hls.ops.deinit(&mock_fp->super);
        assert_that(still_ongoing, is_equal_to(0));
    }
    ATFP_HLS_TEST__INIT__TEARDOWN;
} // end of atfp_hls_test__init_avctx_error


Ensure(atfp_hls_test__deinit_asa_close_files) {
    ATFP_HLS_TEST__INIT__SETUP;
    { // init
        done_flag = 0;
        atfp_ops_video_hls.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        expect(utest_hls__avctx_init,    will_return(0), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls__avfilter_init, will_return(0), when(hlsproc, is_equal_to(mock_fp)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(mock_fp)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        assert_that(access(created_path, F_OK), is_equal_to(0));
    }
#define  UTEST_FILENAME   "some_file"
#define  UTEST_ASALOCAL_FILEPATH    UTEST_ASALOCAL_BASEPATH "/" UTEST_FILENAME
    { // de-init, assume some files were open but have not been closed yet
        int fd_local = open(UTEST_ASALOCAL_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        mock_fp->asa_local.file.file = fd_local;
        mock_fp->super.transfer.dst.flags.asalocal_open  = 1;
        mock_fp->super.transfer.dst.flags.asaremote_open = 1;
        mock_fp->super.transfer.dst.flags.version_exists = 0;
        expect(utest_hls__avctx_deinit,  when(hlsproc, is_equal_to(mock_fp)));
        ASA_RES_CODE  expect_cb_result = ASTORAGE_RESULT_COMPLETE;
        expect(utest_hls__storage_fn_close, will_set_contents_of_parameter(
                    cb_result_ptr, &expect_cb_result, sizeof(ASA_RES_CODE)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(NULL)));
        uint8_t still_ongoing = atfp_ops_video_hls.ops.deinit(&mock_fp->super);
        assert_that(still_ongoing, is_equal_to(1));
        if(still_ongoing) 
            uv_run(loop, UV_RUN_ONCE);
        unlink(UTEST_ASALOCAL_FILEPATH);
    }
#undef  UTEST_ASALOCAL_FILEPATH
#undef  UTEST_FILENAME
    ATFP_HLS_TEST__INIT__TEARDOWN;
} // end of atfp_hls_test__deinit_asa_close_files




#define  ATFP_HLS_TEST__RMFILE__SETUP \
    void  *asa_dst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    uint8_t done_flag = 0; \
    uv_loop_t *loop  = uv_default_loop(); \
    asa_cfg_t  mock_storage_cfg = {.base_path=UTEST_FILE_BASEPATH, .ops={ \
        .fn_scandir=app_storage_localfs_scandir,  .fn_scandir_next=app_storage_localfs_scandir_next, \
        .fn_unlink=app_storage_localfs_unlink,    .fn_rmdir=app_storage_localfs_rmdir, \
    }}; \
    asa_op_localfs_cfg_t  mock_asa_dst = {.loop=loop, .super={ .storage=&mock_storage_cfg, \
        .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_dst_cb_args }}};  \
    json_t *mock_err_info = json_object();  \
    atfp_hls_t  mock_fp = {.super = {.data = {.callback=utest_hls_done_usr_cb, .error=mock_err_info, \
        .storage={.handle=&mock_asa_dst.super}, .version=UTEST_VERSION, .usr_id=UTEST_USER_ID, \
        .upld_req_id=UTEST_UPLOAD_REQ_ID,}}};  \
    asa_dst_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; \
    asa_dst_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    mkdir(UTEST_FILE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR, S_IRWXU); \
    mkdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR, S_IRWXU); \
    mkdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS, S_IRWXU); \
    mkdir(UTEST_TARGET_PATH, S_IRWXU); \
    int fd = open(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    close(fd); \
    fd = open(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    close(fd);

#define  ATFP_HLS_TEST__RMFILE__TEARDOWN \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/" UTEST_TRANSCODE_STATUS); \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR); \
    rmdir(UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR); \
    rmdir(UTEST_FILE_BASEPATH); \
    json_decref(mock_err_info); 

#define  UTEST_USER_ID           426
#define  UTEST_UPLOAD_REQ_ID     0x12345678
#define  UTEST_VERSION          "Nh"
#define  UTEST_TRANSCODE_STATUS     ATFP__DISCARDING_FOLDER_NAME
#define  UTEST_USER_ID__STR         UTEST_STRINGIFY(426)
#define  UTEST_UPLOAD_REQ_ID__STR   UTEST_STRINGIFY(12345678)
#define  UTEST_TARGET_PATH       UTEST_FILE_BASEPATH "/" UTEST_USER_ID__STR "/" UTEST_UPLOAD_REQ_ID__STR "/"  UTEST_TRANSCODE_STATUS  "/" UTEST_VERSION
#define  UTEST_NUM_FILES         2
#define  UTEST_FILE_NAME_1       "segment_abc"
#define  UTEST_FILE_NAME_2       "segment_xyz"
Ensure(atfp_hls_test__removefile__ok) {
    ATFP_HLS_TEST__RMFILE__SETUP;
    {
        atfp_hls__remove_file(&mock_fp.super, UTEST_TRANSCODE_STATUS);
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.size , is_equal_to(UTEST_NUM_FILES));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(access(UTEST_TARGET_PATH, F_OK), is_equal_to(-1));
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    ATFP_HLS_TEST__RMFILE__TEARDOWN;
} // end of atfp_hls_test__removefile__ok


Ensure(atfp_hls_test__removefile__missing_in_middle) {
    ATFP_HLS_TEST__RMFILE__SETUP;
    {
        atfp_hls__remove_file(&mock_fp.super, UTEST_TRANSCODE_STATUS);
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp)));
        uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.size, is_equal_to(UTEST_NUM_FILES));
        assert_that(mock_asa_dst.super.op.scandir.fileinfo.rd_idx, is_less_than(UTEST_NUM_FILES));
        unlink(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_1);
        unlink(UTEST_TARGET_PATH "/" UTEST_FILE_NAME_2);
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(access(UTEST_TARGET_PATH, F_OK), is_equal_to(0));
        assert_that(json_object_size(mock_err_info), is_greater_than(0));
        rmdir(UTEST_TARGET_PATH);
    }
    ATFP_HLS_TEST__RMFILE__TEARDOWN;
} // end of atfp_hls_test__removefile__missing_in_middle
#undef  UTEST_NUM_FILES  
#undef  UTEST_FILE_NAME_1
#undef  UTEST_FILE_NAME_2
#undef  UTEST_TARGET_PATH   
#undef  UTEST_UPLOAD_REQ_ID__STR
#undef  UTEST_USER_ID__STR
#undef  UTEST_USER_ID
#undef  UTEST_UPLOAD_REQ_ID
#undef  UTEST_VERSION
#undef  UTEST_TRANSCODE_STATUS


#define ATFP_HLS_TEST__PROCESS_FRAME__SETUP \
    atfp_asa_map_t  mock_map = {0}; \
    void  *asa_dst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}, *asa_src_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    char  mock_avctx_src[1] = {0}, mock_avctx_dst[1] = {0}; \
    json_t *mock_err_info = json_object(); \
    asa_op_base_cfg_t  mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_src_cb_args}}; \
    asa_op_base_cfg_t  mock_asa_dst = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_dst_cb_args}}; \
    atfp_ops_t mock_fp_src_ops = {.has_done_processing=utest_hls__src_has_done_processing}; \
    atfp_hls_t  mock_fp_src = { .av=(atfp_av_ctx_t *)&mock_avctx_src[0], \
        .super={ .ops=&mock_fp_src_ops, .data={.error=mock_err_info, .storage={.handle=&mock_asa_src}}} \
    }; \
    atfp_hls_t  mock_fp_dst = { \
        .super={.data={.callback=utest_hls_done_usr_cb, .error=mock_err_info, .storage={.handle=&mock_asa_dst}}}, \
        .internal={.op={ \
            .filter=utest_hls__filter_decoded_frame, .encode=utest_hls__encode_filtered_frame, \
            .write=utest_hls__write_encoded_packet,  .move_to_storage=utest_hls__move_localfile_to_dst, \
            .has_done_flush_filter=utest_hls__has_done_flush_filter, \
            .has_done_flush_encoder=utest_hls__has_done_flush_encoder, \
            .finalize={ \
                .filter=utest_hls__flush_filtered_frames, .encode=utest_hls__flush_encoded_frames, \
                .write=utest_hls__final_write_encoded_packet, \
            } \
        }},  .av=(atfp_av_ctx_t *)&mock_avctx_dst[0] \
    }; \
    atfp_asa_map_set_source(&mock_map, &mock_asa_src); \
    asa_dst_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = &mock_fp_dst; \
    asa_dst_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = &mock_map; \
    asa_src_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = &mock_fp_src; \
    asa_src_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = &mock_map;

#define ATFP_HLS_TEST__PROCESS_FRAME__TEARDOWN \
    json_decref(mock_err_info);


#define  ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME(fn_filt, fn_encode, fn_write) \
    for(idx = 0; idx < expect_num_filtered_frms; idx++) { \
        expect(fn_filt,  will_return(return_ok), \
                when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) ); \
        for(jdx = 0; jdx < expect_num_encoded_pkts; jdx++) { \
            expect(fn_encode,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0]))); \
            expect(fn_write,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0]))); \
        }  \
        expect(fn_encode,  will_return(return_need_more_data), when(dst, is_equal_to(&mock_avctx_dst[0]))); \
    } \
    expect(fn_filt,  will_return(return_need_more_data), \
            when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) );


Ensure(atfp_hls_test__process__filter_encode_frames) {
    ATFP_HLS_TEST__PROCESS_FRAME__SETUP
    uint8_t  idx = 0, jdx = 0, expect_num_filtered_frms = 3, expect_num_encoded_pkts = 4; // per filtered frame
    int return_ok = ATFP_AVCTX_RET__OK, return_need_more_data = ATFP_AVCTX_RET__NEED_MORE_DATA;
    ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME( utest_hls__filter_decoded_frame,
            utest_hls__encode_filtered_frame, utest_hls__write_encoded_packet );
    expect(utest_hls__src_has_done_processing, will_return(0));
    expect(utest_hls__has_done_flush_filter,   will_return(0));
    expect(utest_hls__has_done_flush_encoder,  will_return(0));
    expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
            when(hlsproc, is_equal_to(&mock_fp_dst)));
    expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
    atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    ATFP_HLS_TEST__PROCESS_FRAME__TEARDOWN
} // end of atfp_hls_test__process__filter_encode_frames


Ensure(atfp_hls_test__process__filter_encode_error) {
    ATFP_HLS_TEST__PROCESS_FRAME__SETUP
    uint8_t  idx = 0, expect_num_encoded_pkts = 3; // per filtered frame
    int return_ok = ATFP_AVCTX_RET__OK, return_need_more_data = ATFP_AVCTX_RET__NEED_MORE_DATA,
        return_error = -1;
    { // subcase 1, error when filtering
        expect(utest_hls__filter_decoded_frame,  will_return(return_ok),
                when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) );
        for(idx = 0; idx < expect_num_encoded_pkts; idx++) {
            expect(utest_hls__encode_filtered_frame,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0])));
            expect(utest_hls__write_encoded_packet,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0])));
        } // end of loop
        expect(utest_hls__encode_filtered_frame,  will_return(return_need_more_data), when(dst, is_equal_to(&mock_avctx_dst[0])));
        expect(utest_hls__filter_decoded_frame,  will_return(return_error),
                when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) );
        expect(utest_hls__src_has_done_processing, will_return(0));
        expect(utest_hls__has_done_flush_filter,   will_return(0));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        // the user callback utest_hls_done_usr_cb will NOT be invoked due to the error
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(1));
    } { // subcase 2, error when encoding
        json_object_clear(mock_err_info);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
        expect(utest_hls__filter_decoded_frame,  will_return(return_ok),
                when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) );
        for(idx = 0; idx < expect_num_encoded_pkts; idx++) {
            expect(utest_hls__encode_filtered_frame,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0])));
            expect(utest_hls__write_encoded_packet,  will_return(return_ok), when(dst, is_equal_to(&mock_avctx_dst[0])));
        } // end of loop
        expect(utest_hls__encode_filtered_frame,  will_return(return_error), when(dst, is_equal_to(&mock_avctx_dst[0])));
        expect(utest_hls__src_has_done_processing, will_return(0));
        expect(utest_hls__has_done_flush_filter,   will_return(0));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        // the user callback utest_hls_done_usr_cb will NOT be invoked due to the error
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(1));
    }
    ATFP_HLS_TEST__PROCESS_FRAME__TEARDOWN
} // end of atfp_hls_test__process__filter_encode_error


Ensure(atfp_hls_test__process__flush_filter) {
    ATFP_HLS_TEST__PROCESS_FRAME__SETUP
    uint8_t  idx = 0, jdx = 0, expect_num_filtered_frms = 2, expect_num_encoded_pkts = 3; // per filtered frame
    int return_ok = ATFP_AVCTX_RET__OK, return_need_more_data = ATFP_AVCTX_RET__NEED_MORE_DATA;
    { // switch filtering function
        ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME( utest_hls__filter_decoded_frame,
            utest_hls__encode_filtered_frame, utest_hls__write_encoded_packet );
        expect(utest_hls__src_has_done_processing, will_return(1));
        expect(utest_hls__has_done_flush_filter,   will_return(0));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
                when(hlsproc, is_equal_to(&mock_fp_dst)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    } { // start flushing filter
        ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME( utest_hls__flush_filtered_frames,
            utest_hls__encode_filtered_frame, utest_hls__write_encoded_packet );
        expect(utest_hls__src_has_done_processing, will_return(1));
        expect(utest_hls__has_done_flush_filter,   will_return(0));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
                when(hlsproc, is_equal_to(&mock_fp_dst)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    ATFP_HLS_TEST__PROCESS_FRAME__TEARDOWN
} // end of atfp_hls_test__process__flush_filter


Ensure(atfp_hls_test__process__flush_encoder) {
    ATFP_HLS_TEST__PROCESS_FRAME__SETUP
    uint8_t  idx = 0, jdx = 0, expect_num_filtered_frms = 2, expect_num_encoded_pkts = 3; // per filtered frame
    int return_ok = ATFP_AVCTX_RET__OK, return_need_more_data = ATFP_AVCTX_RET__NEED_MORE_DATA;
    // assume the application has done flushing filter
    mock_fp_dst.internal.op.filter = utest_hls__flush_filtered_frames;
    { // switch encoding function
        ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME( utest_hls__flush_filtered_frames,
            utest_hls__encode_filtered_frame, utest_hls__write_encoded_packet );
        expect(utest_hls__src_has_done_processing, will_return(1));
        expect(utest_hls__has_done_flush_filter,   will_return(1));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
                when(hlsproc, is_equal_to(&mock_fp_dst)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    } { // start flushing encoder
        ATFP_HLS_TEST__WALKTHOUGH_ALL_FILT_FRAME( utest_hls__flush_filtered_frames,
            utest_hls__flush_encoded_frames, utest_hls__write_encoded_packet );
        expect(utest_hls__src_has_done_processing, will_return(1));
        expect(utest_hls__has_done_flush_filter,   will_return(1));
        expect(utest_hls__has_done_flush_encoder,  will_return(0));
        expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
                when(hlsproc, is_equal_to(&mock_fp_dst)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    } { // has done flushing encoder
        expect_num_encoded_pkts = 4;
        expect(utest_hls__flush_filtered_frames,  will_return(ATFP_AVCTX_RET__OK), 
                when(dst, is_equal_to(&mock_avctx_dst[0])),  when(src, is_equal_to(&mock_avctx_src[0])) );
        for(jdx = 0; jdx < expect_num_encoded_pkts; jdx++) {
            expect(utest_hls__flush_encoded_frames,  will_return(ATFP_AVCTX_RET__OK));
            expect(utest_hls__write_encoded_packet,  will_return(ATFP_AVCTX_RET__OK));
        }
        expect(utest_hls__flush_encoded_frames,  will_return(ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER),
                when(dst, is_equal_to(&mock_avctx_dst[0])));
        expect(utest_hls__src_has_done_processing, will_return(1));
        expect(utest_hls__has_done_flush_filter,   will_return(1));
        expect(utest_hls__has_done_flush_encoder,  will_return(1));
        expect(utest_hls__final_write_encoded_packet,  will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
                when(dst, is_equal_to(&mock_avctx_dst[0])));
        expect(utest_hls__move_localfile_to_dst,   will_return(ASTORAGE_RESULT_ACCEPT),
                when(hlsproc, is_equal_to(&mock_fp_dst)));
        expect(utest_hls_done_usr_cb, when(processor, is_equal_to(&mock_fp_dst)));
        atfp_ops_video_hls.ops.processing(&mock_fp_dst.super);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    ATFP_HLS_TEST__PROCESS_FRAME__TEARDOWN
} // end of atfp_hls_test__process__flush_encoder


TestSuite *app_transcoder_hls_init_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__init_deinit__ok);
    add_test(suite, atfp_hls_test__init_avctx_error);
    add_test(suite, atfp_hls_test__deinit_asa_close_files);
    add_test(suite, atfp_hls_test__removefile__ok);
    add_test(suite, atfp_hls_test__removefile__missing_in_middle);
    add_test(suite, atfp_hls_test__process__filter_encode_frames);
    add_test(suite, atfp_hls_test__process__filter_encode_error);
    add_test(suite, atfp_hls_test__process__flush_filter);
    add_test(suite, atfp_hls_test__process__flush_encoder);
    return suite;
}
