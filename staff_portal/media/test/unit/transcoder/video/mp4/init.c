#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

extern const atfp_ops_entry_t  atfp_ops_video_mp4;


static void  utest_atfp__async_usr_callback(uv_async_t* handle)
{
    atfp_t *processor = handle -> data;
    processor -> data.callback(processor);
}

static void  utest_atfp_usr_cb(atfp_t *processor)
{ mock(processor); }

static  int  utest_atfp_mockops_decode_pkt(atfp_av_ctx_t *avctx)
{ return (int) mock(avctx); }

static  int  utest_atfp_mockops_next_pkt(atfp_av_ctx_t *avctx)
{ return (int) mock(avctx); }

static  ASA_RES_CODE  utest_atfp_mockops_preload(atfp_mp4_t *mp4proc, size_t nbytes, void (*cb)(atfp_mp4_t *))
{ return (ASA_RES_CODE) mock(mp4proc,cb); }


#define  UTEST_ATFP_MP4_PROCESS_SETUP \
    atfp_av_ctx_t  mock_av_ctx = {0}; \
    atfp_mp4_t  mock_mp4proc = { .av=&mock_av_ctx, .async={0}, .internal={.op={.preload=utest_atfp_mockops_preload, \
        .next_pkt=utest_atfp_mockops_next_pkt, .decode_pkt=utest_atfp_mockops_decode_pkt}}, \
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
    //add_test(suite, atfp_mp4_test__init_deinit__ok);
    add_test(suite, atfp_mp4_test__process_one_frame__ok);
    add_test(suite, atfp_mp4_test__fetch_and_process_one_frame__ok);
    add_test(suite, atfp_mp4_test__process_preload_start__ok);
    add_test(suite, atfp_mp4_test__process_preload_start__error);
    add_test(suite, atfp_mp4_test__process_decode__error);
    return suite;
}
