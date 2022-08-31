#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASALOCAL_BASEPATH   UTEST_FILE_BASEPATH "/asalocal"
#define  UTEST_LOCAL_TMPBUF        UTEST_ASALOCAL_BASEPATH "/local_buffer"
#define  DONE_FLAG_INDEX__IN_ASA_USRARG     (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ  (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)

extern const atfp_ops_entry_t  atfp_ops_video_mp4;

static void  utest_atfp__async_usr_callback(uv_async_t* handle)
{
    atfp_t *processor = handle -> data;
    processor -> data.callback(processor);
}

static void  utest_atfp_usr_cb(atfp_t *processor) {
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    if(asa_src && asa_src->cb_args.entries) {
        uint8_t *done_flag = asa_src->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flag && *done_flag == 0) {
            *done_flag = 1;
            mock(processor);
        }
    } else {
       mock(processor);
    }
} // end of utest_atfp_usr_cb

static  ASA_RES_CODE  utest_mp4__av_init (atfp_mp4_t *mp4proc, void (*cb)(atfp_mp4_t *))
{
    ASA_RES_CODE result = (ASA_RES_CODE) mock(mp4proc);
    if(result == ASTORAGE_RESULT_ACCEPT)
        cb(mp4proc);
    return  result;
}

static void utest_mp4__av_deinit (atfp_mp4_t *mp4proc)
{ mock(mp4proc); }

static  ASA_RES_CODE  utest_mp4__preload_info (atfp_mp4_t *mp4proc, void (*cb)(atfp_mp4_t *))
{
    ASA_RES_CODE result = (ASA_RES_CODE) mock(mp4proc);
    if(result == ASTORAGE_RESULT_ACCEPT)
        cb(mp4proc);
    return  result;
}

static  int  utest_mp4__av_validate (atfp_av_ctx_t *avctx, json_t *err_info)
{
    int  err = (int) mock(avctx);
    if(err)
        json_object_set_new(err_info, "transcoder", json_string("[mp4] validation error"));
    return  err;
}

static ASA_RES_CODE  utest_asa__close_fn(asa_op_base_cfg_t *asaobj)
{
    ASA_RES_CODE result = (ASA_RES_CODE) mock(asaobj);
    if(result == ASTORAGE_RESULT_ACCEPT)
        asaobj->op.close.cb(asaobj, ASTORAGE_RESULT_COMPLETE);
    return  result;
}

static  int  utest_atfp_mockops_decode_pkt(atfp_av_ctx_t *avctx)
{ return (int) mock(avctx); }

static  int  utest_atfp_mockops_next_pkt(atfp_av_ctx_t *avctx)
{ return (int) mock(avctx); }

static  ASA_RES_CODE  utest_atfp_mockops_preload(atfp_mp4_t *mp4proc, size_t nbytes, void (*cb)(atfp_mp4_t *))
{ return (ASA_RES_CODE) mock(mp4proc,cb); }


#define  UTEST_ATFP_MP4__INIT_SETUP \
    uv_loop_t *loop  = uv_default_loop(); \
    atfp_asa_map_t  mock_map = {0}; \
    uint8_t done_flag = 0; \
    void  *asa_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_op_base_cfg_t  *mock_asa_src = calloc(1, sizeof(asa_op_base_cfg_t)); \
    *mock_asa_src = (asa_op_base_cfg_t) { \
        .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_cb_args}, \
    }; \
    asa_op_localfs_cfg_t  mock_asa_local = { .loop=loop, .file={.file=-1}, \
        .super={ \
            .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asa_cb_args}, \
            .op={.mkdir={.path={.origin=UTEST_ASALOCAL_BASEPATH}}}, \
        } \
    }; \
    asa_cfg_t  mock_storage_cfg = {.ops={.fn_close=utest_asa__close_fn}}; \
    json_t *mock_errinfo = json_object(); \
    atfp_mp4_t *mock_fp = (atfp_mp4_t *) atfp_ops_video_mp4.ops.instantiate(); \
    mock_fp->internal.op.av_init   = utest_mp4__av_init; \
    mock_fp->internal.op.av_deinit = utest_mp4__av_deinit; \
    mock_fp->internal.op.av_validate  = utest_mp4__av_validate; \
    mock_fp->internal.op.preload_info = utest_mp4__preload_info ; \
    mock_fp->super.data.callback = utest_atfp_usr_cb ; \
    mock_fp->super.data.error = mock_errinfo ; \
    mock_fp->super.data.storage.handle = mock_asa_src ; \
    mock_fp->super.data.storage.config = &mock_storage_cfg; \
    atfp_asa_map_set_source(&mock_map, mock_asa_src); \
    atfp_asa_map_set_localtmp(&mock_map, &mock_asa_local); \
    asa_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = mock_fp; \
    asa_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = &mock_map; \
    asa_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &done_flag; \
    mkdir(UTEST_FILE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU); \


#define  UTEST_ATFP_MP4__INIT_TEARDOWN \
    json_decref(mock_errinfo); \
    if(mock_asa_local.file.file >= 0) { \
        close(mock_asa_local.file.file); \
        mock_asa_local.file.file = -1; \
    } \
    if(mock_asa_local.super.op.open.dst_path) { \
        unlink(mock_asa_local.super.op.open.dst_path); \
        free(mock_asa_local.super.op.open.dst_path); \
    } \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH);


Ensure(atfp_mp4_test__init_deinit__ok) {
    UTEST_ATFP_MP4__INIT_SETUP
    { // init
        atfp_ops_video_mp4.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
        expect(utest_mp4__preload_info, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_mp4__av_init, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_mp4__av_validate, will_return(0));
        expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
        assert_that(mock_asa_local.file.file, is_greater_than(-1));
    } { // de-init
        expect(utest_mp4__av_deinit, when(mp4proc, is_equal_to(mock_fp)));
        expect(utest_asa__close_fn, will_return(ASTORAGE_RESULT_ACCEPT),
                when(asaobj, is_equal_to(mock_asa_src)));
        atfp_ops_video_mp4.ops.deinit(&mock_fp->super);
        uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    }
    UTEST_ATFP_MP4__INIT_TEARDOWN
} // end of atfp_mp4_test__init_deinit__ok


Ensure(atfp_mp4_test__init_preload_error) {
    UTEST_ATFP_MP4__INIT_SETUP
    { // init
        atfp_ops_video_mp4.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
        expect(utest_mp4__preload_info, will_return(ASTORAGE_RESULT_OS_ERROR));
        expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(1));
    } { // de-init
        json_object_clear(mock_errinfo);
        expect(utest_mp4__av_deinit, when(mp4proc, is_equal_to(mock_fp)));
        expect(utest_asa__close_fn, will_return(ASTORAGE_RESULT_ACCEPT),
                when(asaobj, is_equal_to(mock_asa_src)));
        atfp_ops_video_mp4.ops.deinit(&mock_fp->super);
        uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    }
    UTEST_ATFP_MP4__INIT_TEARDOWN
} // end of atfp_mp4_test__init_preload_error


Ensure(atfp_mp4_test__init_avctx_error) {
    UTEST_ATFP_MP4__INIT_SETUP
    { // init
        atfp_ops_video_mp4.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
        expect(utest_mp4__preload_info, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_mp4__av_init, will_return(ASTORAGE_RESULT_DATA_ERROR));
        expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(1));
    } { // de-init
        json_object_clear(mock_errinfo);
        expect(utest_mp4__av_deinit, when(mp4proc, is_equal_to(mock_fp)));
        expect(utest_asa__close_fn, will_return(ASTORAGE_RESULT_ACCEPT),
                when(asaobj, is_equal_to(mock_asa_src)));
        atfp_ops_video_mp4.ops.deinit(&mock_fp->super);
        uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    }
    UTEST_ATFP_MP4__INIT_TEARDOWN
} // end of atfp_mp4_test__init_avctx_error


Ensure(atfp_mp4_test__init_avctx_validation_failure) {
    UTEST_ATFP_MP4__INIT_SETUP
    { // init
        atfp_ops_video_mp4.ops.init(&mock_fp->super);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
        expect(utest_mp4__preload_info, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_mp4__av_init, will_return(ASTORAGE_RESULT_ACCEPT));
        expect(utest_mp4__av_validate, will_return(-1));
        expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_fp->super)));
        while(!done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(1));
    } { // de-init
        json_object_clear(mock_errinfo);
        expect(utest_mp4__av_deinit, when(mp4proc, is_equal_to(mock_fp)));
        expect(utest_asa__close_fn, will_return(ASTORAGE_RESULT_ACCEPT),
                when(asaobj, is_equal_to(mock_asa_src)));
        atfp_ops_video_mp4.ops.deinit(&mock_fp->super);
        uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    }
    UTEST_ATFP_MP4__INIT_TEARDOWN
} // end of atfp_mp4_test__init_avctx_validation_failure


#define  UTEST_ATFP_MP4_PROCESS_SETUP \
    atfp_av_ctx_t  mock_av_ctx = {0}; \
    atfp_mp4_t  mock_mp4proc = { .av=&mock_av_ctx, .async={0}, .internal={.op={ \
        .preload_pkt=utest_atfp_mockops_preload, .next_pkt=utest_atfp_mockops_next_pkt, \
        .decode_pkt=utest_atfp_mockops_decode_pkt}}, \
        .super={.data={.callback=utest_atfp_usr_cb,  .error=json_object(),}}, \
    }; \
    uv_loop_t *loop =  uv_default_loop(); \
    uv_async_init(loop, &mock_mp4proc.async, utest_atfp__async_usr_callback); \
    mock_mp4proc.async.data = &mock_mp4proc.super;

#define  UTEST_ATFP_MP4_PROCESS_TEARDOWN \
    uv_close((uv_handle_t *)&mock_mp4proc.async, NULL); \
    uv_run(loop, UV_RUN_ONCE); \
    json_decref(mock_mp4proc.super.data.error);

Ensure(atfp_mp4_test__process_one_frame__ok) {
    UTEST_ATFP_MP4_PROCESS_SETUP;
    expect(utest_atfp_mockops_decode_pkt, will_return(0), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_mp4proc.super)));
    atfp_ops_video_mp4 .ops .processing(&mock_mp4proc.super);
    uv_run(loop, UV_RUN_NOWAIT);
    UTEST_ATFP_MP4_PROCESS_TEARDOWN;
} // end of atfp_mp4_test__process_one_frame__ok


Ensure(atfp_mp4_test__fetch_and_process_one_frame__ok) {
    // assume the app fetches number of packets, the decoder cannot produce frames from
    // all these packets except the last one
    UTEST_ATFP_MP4_PROCESS_SETUP;
    int idx = 0, num_pkt_fetched = 5;
    for(idx = 0; idx < num_pkt_fetched; idx++) {
        expect(utest_atfp_mockops_decode_pkt, will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
        expect(utest_atfp_mockops_next_pkt,   will_return(0), when(avctx, is_equal_to(&mock_av_ctx)));
    }
    expect(utest_atfp_mockops_decode_pkt, will_return(0), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_mp4proc.super)));
    atfp_ops_video_mp4 .ops .processing(&mock_mp4proc.super);
    uv_run(loop, UV_RUN_NOWAIT);
    UTEST_ATFP_MP4_PROCESS_TEARDOWN;
} // end of atfp_mp4_test__fetch_and_process_one_frame__ok


Ensure(atfp_mp4_test__process_preload_start__ok) {
    UTEST_ATFP_MP4_PROCESS_SETUP;
    expect(utest_atfp_mockops_decode_pkt, will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_mockops_next_pkt,   will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_mockops_preload,   will_return(ASTORAGE_RESULT_ACCEPT),
            when(mp4proc, is_equal_to(&mock_mp4proc)),  when(cb, is_not_equal_to(NULL)), 
    );
    atfp_ops_video_mp4 .ops .processing(&mock_mp4proc.super);
    UTEST_ATFP_MP4_PROCESS_TEARDOWN;
} // end of atfp_mp4_test__process_preload_start__ok


Ensure(atfp_mp4_test__process_preload_start__error) {
    UTEST_ATFP_MP4_PROCESS_SETUP;
    expect(utest_atfp_mockops_decode_pkt, will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_mockops_next_pkt,   will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_mockops_preload,   will_return(ASTORAGE_RESULT_OS_ERROR),
            when(mp4proc, is_equal_to(&mock_mp4proc)),  when(cb, is_not_equal_to(NULL)), 
    );
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_mp4proc.super)));
    atfp_ops_video_mp4 .ops .processing(&mock_mp4proc.super);
    UTEST_ATFP_MP4_PROCESS_TEARDOWN;
} // end of atfp_mp4_test__process_preload_start__error


Ensure(atfp_mp4_test__process_decode__error) {
    UTEST_ATFP_MP4_PROCESS_SETUP;
    expect(utest_atfp_mockops_decode_pkt, will_return(1), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_mockops_next_pkt,   will_return(AVERROR(EIO)), when(avctx, is_equal_to(&mock_av_ctx)));
    expect(utest_atfp_usr_cb, when(processor, is_equal_to(&mock_mp4proc.super)));
    atfp_ops_video_mp4 .ops .processing(&mock_mp4proc.super);
    UTEST_ATFP_MP4_PROCESS_TEARDOWN;
} // end of atfp_mp4_test__process_decode__error


TestSuite *app_transcoder_mp4_init_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_mp4_test__init_deinit__ok);
    add_test(suite, atfp_mp4_test__init_preload_error);
    add_test(suite, atfp_mp4_test__init_avctx_error);
    add_test(suite, atfp_mp4_test__init_avctx_validation_failure);
    add_test(suite, atfp_mp4_test__process_one_frame__ok);
    add_test(suite, atfp_mp4_test__fetch_and_process_one_frame__ok);
    add_test(suite, atfp_mp4_test__process_preload_start__ok);
    add_test(suite, atfp_mp4_test__process_preload_start__error);
    add_test(suite, atfp_mp4_test__process_decode__error);
    return suite;
}
