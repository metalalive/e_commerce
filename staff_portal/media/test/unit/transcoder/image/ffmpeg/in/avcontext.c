#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/ffmpeg.h"

#define  UTEST_FFM_FILENAME  "/path/to/image/file"

#define  UTEST_FFM_INIT_SETUP \
    AVCodec  mock_av_decoder = {0}; \
    AVCodecContext  mock_dec_ctxs[1] = {{.time_base={.num=0}}}; \
    AVCodecParameters  mock_codec_param = {0}; \
    AVStream   mock_vdo_stream = {.codecpar=&mock_codec_param}; \
    AVStream  *mock_av_streams_ptr[1] = {&mock_vdo_stream}; \
    AVFormatContext  mock_avfmt_ctx = {.nb_streams=1, .streams=mock_av_streams_ptr}; \
    AVFormatContext *mock_avfmt_ctx_p = &mock_avfmt_ctx; \
    atfp_av_ctx_t  mock_avctx = {0}; \
    json_t *mock_err_info = json_object();

#define  UTEST_FFM_INIT_TEARDOWN \
    json_decref(mock_err_info);

Ensure(atfp_img_ffi_test__avctx_init_ok)
{
    UTEST_FFM_INIT_SETUP
    mock_dec_ctxs[0].codec_type = AVMEDIA_TYPE_VIDEO;
    {
        expect(avformat_open_input, will_return(0), when(_fmt_ctx, is_equal_to(NULL)),
               will_set_contents_of_parameter(_fmt_ctx_p, &mock_avfmt_ctx_p, sizeof(AVFormatContext *)),
        );
        expect(avformat_find_stream_info, will_return(0), when(ic, is_equal_to(mock_avfmt_ctx_p)));
        expect(av_mallocz_array, will_return((AVCodecContext **)&mock_dec_ctxs[0]));
        expect(avcodec_find_decoder, will_return(&mock_av_decoder));
        expect(avcodec_alloc_context3, will_return(&mock_dec_ctxs[0]), when(codec, is_equal_to(&mock_av_decoder)));
        expect(avcodec_parameters_to_context, will_return(0),  when(par, is_equal_to(&mock_codec_param)),
                when(codec_ctx, is_equal_to(&mock_dec_ctxs[0])),   );
        expect(avcodec_open2, will_return(0),  when(codec, is_equal_to(&mock_av_decoder)),
                when(ctx, is_equal_to(&mock_dec_ctxs[0])),   );
        expect(av_log);
    }
    atfp__image_src__avctx_init (&mock_avctx, UTEST_FFM_FILENAME, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    {
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_dec_ctxs[0])));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_dec_ctxs[0])));
        expect(avformat_close_input, when(ref_fmtctx, is_equal_to(mock_avfmt_ctx_p)));
    }
    atfp__image_src__avctx_deinit (&mock_avctx);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffi_test__avctx_init_ok


Ensure(atfp_img_ffi_test__avctx_format_error)
{
    UTEST_FFM_INIT_SETUP
    (void) mock_dec_ctxs;
    (void) mock_av_decoder ;
    int expect_errcode = AVERROR(EIO);
    expect(avformat_open_input, will_return(expect_errcode), when(_fmt_ctx, is_equal_to(NULL)),
           will_set_contents_of_parameter(_fmt_ctx_p, &mock_avfmt_ctx_p, sizeof(AVFormatContext *)),
    );
    atfp__image_src__avctx_init (&mock_avctx, UTEST_FFM_FILENAME, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    int actual_errcode = json_integer_value(json_object_get(mock_err_info, "err_code"));
    assert_that(actual_errcode, is_equal_to(expect_errcode));
    expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
    expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
    expect(avformat_close_input, when(ref_fmtctx, is_equal_to(mock_avfmt_ctx_p)));
    atfp__image_src__avctx_deinit (&mock_avctx);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffi_test__avctx_format_error


Ensure(atfp_img_ffi_test__avctx_decoder_error)
{
    UTEST_FFM_INIT_SETUP
    int expect_errcode = AVERROR(ENOENT);
    {
        expect(avformat_open_input, will_return(0), when(_fmt_ctx, is_equal_to(NULL)),
               will_set_contents_of_parameter(_fmt_ctx_p, &mock_avfmt_ctx_p, sizeof(AVFormatContext *)),
        );
        expect(avformat_find_stream_info, will_return(0), when(ic, is_equal_to(mock_avfmt_ctx_p)));
        expect(av_mallocz_array, will_return((AVCodecContext **)&mock_dec_ctxs[0]));
        expect(avcodec_find_decoder, will_return(&mock_av_decoder));
        expect(avcodec_alloc_context3, will_return(&mock_dec_ctxs[0]), when(codec, is_equal_to(&mock_av_decoder)));
        expect(avcodec_parameters_to_context, will_return(expect_errcode), when(par,is_equal_to(&mock_codec_param)),
                when(codec_ctx, is_equal_to(&mock_dec_ctxs[0])),   );
    }
    atfp__image_src__avctx_init (&mock_avctx, UTEST_FFM_FILENAME, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    int actual_errcode = json_integer_value(json_object_get(mock_err_info, "err_code"));
    assert_that(actual_errcode, is_equal_to(expect_errcode));
    {
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_dec_ctxs[0])));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_dec_ctxs[0])));
        expect(avformat_close_input, when(ref_fmtctx, is_equal_to(mock_avfmt_ctx_p)));
    }
    atfp__image_src__avctx_deinit (&mock_avctx);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffi_test__avctx_decoder_error


Ensure(atfp_img_ffi_test__avctx_decctx_error)
{
    UTEST_FFM_INIT_SETUP
    mock_dec_ctxs[0].codec_type = AVMEDIA_TYPE_SUBTITLE;
    {
        expect(avformat_open_input, will_return(0), when(_fmt_ctx, is_equal_to(NULL)),
               will_set_contents_of_parameter(_fmt_ctx_p, &mock_avfmt_ctx_p, sizeof(AVFormatContext *)),
        );
        expect(avformat_find_stream_info, will_return(0), when(ic, is_equal_to(mock_avfmt_ctx_p)));
        expect(av_mallocz_array, will_return((AVCodecContext **)&mock_dec_ctxs[0]));
        expect(avcodec_find_decoder, will_return(&mock_av_decoder));
        expect(avcodec_alloc_context3, will_return(&mock_dec_ctxs[0]), when(codec, is_equal_to(&mock_av_decoder)));
        expect(avcodec_parameters_to_context, will_return(0),  when(par, is_equal_to(&mock_codec_param)),
                when(codec_ctx, is_equal_to(&mock_dec_ctxs[0])),   );
    }
    atfp__image_src__avctx_init (&mock_avctx, UTEST_FFM_FILENAME, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    int actual_errcode = json_integer_value(json_object_get(mock_err_info, "err_code"));
    assert_that(actual_errcode, is_equal_to(AVERROR_INVALIDDATA));
    {
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_dec_ctxs[0])));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_dec_ctxs[0])));
        expect(avformat_close_input, when(ref_fmtctx, is_equal_to(mock_avfmt_ctx_p)));
    }
    atfp__image_src__avctx_deinit (&mock_avctx);
    UTEST_FFM_INIT_TEARDOWN
} // end of atfp_img_ffi_test__avctx_decctx_error


#define  UTEST_AVCTX__PKT_DECODE_FRAME_SETUP \
    AVCodecContext  mock_dec_ctx = {.codec_type=AVMEDIA_TYPE_VIDEO}, *mock_dec_ctx_ptrs[1] = {&mock_dec_ctx}; \
    AVStream   mock_av_stream = {0}, *mock_av_streams_ptr[1] = {&mock_av_stream}; \
    AVFormatContext  mock_avfmt_ctx = {.nb_streams=1, .streams=mock_av_streams_ptr}; \
    atfp_av_ctx_t  mock_avctx = { \
        .fmt_ctx=&mock_avfmt_ctx, .stream_ctx={.decode=mock_dec_ctx_ptrs}, \
        .intermediate_data = {.decode={.packet={.stream_index=0, .size=123}}} \
    };

Ensure(atfp_img_ffi_test__pkt_decode_frames_ok)
{
    UTEST_AVCTX__PKT_DECODE_FRAME_SETUP;
    int err = 0, expect_num_frames_avail = 6, new_frm_ready = 0;
    // invoke only at the first time
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
    expect(avcodec_send_packet,  will_return(0),
            when(avpkt, is_equal_to(&mock_avctx.intermediate_data.decode.packet)));
    for(int idx = 0; idx < expect_num_frames_avail; idx++) {
        expect(avcodec_receive_frame,   will_return(0),
                when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
        err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
        assert_that(err, is_equal_to(new_frm_ready));
        assert_that( mock_avctx.intermediate_data.decode.num_decoded_frames,
                is_equal_to(idx + 1) );
    }
} // end of atfp_img_ffi_test__pkt_decode_frames_ok


Ensure(atfp_img_ffi_test__pkt_decode_more_data_required)
{
    UTEST_AVCTX__PKT_DECODE_FRAME_SETUP;
    int err = 0, next_pkt_required = 1;
    mock_avctx.intermediate_data.decode.num_decoded_frames = 0;
    mock_avctx.intermediate_data.decode.packet.size = 0;
    err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
    assert_that(err, is_equal_to(next_pkt_required));
    mock_avctx.intermediate_data.decode.num_decoded_frames = 0;
    mock_avctx.intermediate_data.decode.packet.size = 135;
    mock_dec_ctx.codec_type = AVMEDIA_TYPE_AUDIO;
    err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
    assert_that(err, is_equal_to(next_pkt_required));
    mock_dec_ctx.codec_type = AVMEDIA_TYPE_VIDEO;
    mock_avctx.intermediate_data.decode.num_decoded_frames = 2;
    mock_avctx.intermediate_data.decode.packet.size = 234;
    expect(avcodec_receive_frame,   will_return(AVERROR(EAGAIN)),
                when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
    err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
    assert_that(err, is_equal_to(next_pkt_required));
} // end of atfp_img_ffi_test__pkt_decode_more_data_required


Ensure(atfp_img_ffi_test__pkt_decode__send_error) 
{
    UTEST_AVCTX__PKT_DECODE_FRAME_SETUP;
    int err = 0, expect_err = AVERROR(EBUSY);
    AVPacket *mock_pkt = &mock_avctx.intermediate_data.decode.packet;
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(mock_pkt)));
    expect(avcodec_send_packet,  will_return(expect_err), when(avpkt, is_equal_to(mock_pkt)));
    expect(av_log);
    err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
    assert_that(err, is_equal_to(expect_err));
} // end of atfp_img_ffi_test__pkt_decode__send_error


Ensure(atfp_img_ffi_test__pkt_decode__recv_error)
{
    UTEST_AVCTX__PKT_DECODE_FRAME_SETUP;
    int err = 0, expect_err = AVERROR(EBADMSG);
    mock_avctx.intermediate_data.decode.num_decoded_frames = 2;
    expect(avcodec_receive_frame,   will_return(expect_err),
                when(frame, is_equal_to(&mock_avctx.intermediate_data.decode.frame)));
    expect(av_log);
    err = atfp__image_src__avctx_decode_curr_packet(&mock_avctx);
    assert_that(err, is_equal_to(expect_err));
} // end of atfp_img_ffi_test__pkt_decode__recv_error


#define  UTEST_AVCTX__FETCH_PKT_SETUP \
    AVFormatContext  mock_avfmt_ctx = {.nb_streams=1}; \
    atfp_av_ctx_t  mock_avctx = {.fmt_ctx=&mock_avfmt_ctx };

Ensure(atfp_img_ffi_test__fetch_nxt_pkt_ok)
{
    UTEST_AVCTX__FETCH_PKT_SETUP
    // assume previous packet can be decoded to several frames
    mock_avctx.intermediate_data.decode.num_decoded_frames = 31;
    AVPacket *mock_pkt = &mock_avctx.intermediate_data.decode.packet;
    expect(av_packet_unref, when(pkt, is_equal_to(mock_pkt)));
    expect(av_read_frame,  will_return(0), when(pkt, is_equal_to(mock_pkt)),
            when(fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
    int err = atfp__image_src__avctx_fetch_next_packet(&mock_avctx);
    assert_that(err, is_equal_to(0));
    assert_that(mock_avctx.intermediate_data.decode.num_decoded_frames, is_equal_to(0));
} // end of  atfp_img_ffi_test__fetch_nxt_pkt_ok

Ensure(atfp_img_ffi_test__fetch_nxt_pkt_eof)
{
    UTEST_AVCTX__FETCH_PKT_SETUP
    int expect_err = 1;
    AVPacket *mock_pkt = &mock_avctx.intermediate_data.decode.packet;
    expect(av_packet_unref, when(pkt, is_equal_to(mock_pkt)));
    expect(av_read_frame,  will_return(AVERROR_EOF), when(pkt, is_equal_to(mock_pkt)),
            when(fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
    int err = atfp__image_src__avctx_fetch_next_packet(&mock_avctx);
    assert_that(err, is_equal_to(expect_err));
} // end of  atfp_img_ffi_test__fetch_nxt_pkt_eof

Ensure(atfp_img_ffi_test__fetch_nxt_pkt_corrupted)
{
    UTEST_AVCTX__FETCH_PKT_SETUP
    int  expect_pkt_corruption = 1;
    AVPacket *mock_pkt = &mock_avctx.intermediate_data.decode.packet;
    expect(av_packet_unref, when(pkt, is_equal_to(mock_pkt)));
    expect(av_read_frame,  will_return(0), when(pkt, is_equal_to(mock_pkt)),
           will_set_contents_of_parameter(corrupted_p, &expect_pkt_corruption, sizeof(int)),
           when(fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
    int err = atfp__image_src__avctx_fetch_next_packet(&mock_avctx);
    assert_that(err, is_less_than(0));
} // end of  atfp_img_ffi_test__fetch_nxt_pkt_corrupted


TestSuite *app_transcoder_img_ffm_in_avctx_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffi_test__avctx_init_ok);
    add_test(suite, atfp_img_ffi_test__avctx_format_error);
    add_test(suite, atfp_img_ffi_test__avctx_decoder_error);
    add_test(suite, atfp_img_ffi_test__avctx_decctx_error);
    add_test(suite, atfp_img_ffi_test__pkt_decode_frames_ok);
    add_test(suite, atfp_img_ffi_test__pkt_decode_more_data_required);
    add_test(suite, atfp_img_ffi_test__pkt_decode__send_error);
    add_test(suite, atfp_img_ffi_test__pkt_decode__recv_error);
    add_test(suite, atfp_img_ffi_test__fetch_nxt_pkt_ok);
    add_test(suite, atfp_img_ffi_test__fetch_nxt_pkt_eof);
    add_test(suite, atfp_img_ffi_test__fetch_nxt_pkt_corrupted);
    return suite;
}
