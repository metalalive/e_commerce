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
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_dec_ctxs[0])));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_dec_ctxs[0])));
        expect(avformat_close_input, when(ref_fmtctx, is_equal_to(mock_avfmt_ctx_p)));
    }
    atfp__image_src__avctx_deinit (&mock_avctx);
    UTEST_FFM_INIT_TEARDOWN
} // end of atfp_img_ffi_test__avctx_decctx_error


TestSuite *app_transcoder_img_ffm_in_avctx_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffi_test__avctx_init_ok);
    add_test(suite, atfp_img_ffi_test__avctx_format_error);
    add_test(suite, atfp_img_ffi_test__avctx_decoder_error);
    add_test(suite, atfp_img_ffi_test__avctx_decctx_error);
    return suite;
}
