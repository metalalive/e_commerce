#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/ffmpeg.h"

#define  STRINGIFY(x)       #x 
#define  UTEST_CROP_WIDTH    450
#define  UTEST_CROP_HEIGHT   250
#define  UTEST_SCALE_WIDTH   430
#define  UTEST_SCALE_HEIGHT  220
#define  UTEST_FILT_SPEC   "{\"crop\":{\"width\":"STRINGIFY(450)", \"height\":"STRINGIFY(250)"}," \
    "\"scale\":{\"width\":"STRINGIFY(430)", \"height\":"STRINGIFY(220)"}}"

#define   UTEST_FFM_INIT_SETUP \
    const char *mock_outfile_path = "/path/to/out/file"; \
    AVCodec  mock_av_encoder = {0}; \
    AVCodecContext  mock_av_encctx = {0}; \
    AVCodecContext  mock_dec_ctxs[1] = {{.time_base={.num=1, .den=1}, .codec_type=AVMEDIA_TYPE_VIDEO}}; \
    AVCodecContext *mock_dec_ctxs_p[1] = {&mock_dec_ctxs[0]}; \
    atfp_stream_enc_ctx_t  mock_st_enc_ctxs[1] = {0}; \
    AVCodecParameters  mock_codec_param = {0}; \
    AVStream   mock_vdo_stream = {.codecpar=&mock_codec_param}; \
    AVOutputFormat  mock_ofmt = {0}; \
    AVFormatContext  mock_ifmt_ctx = {.nb_streams=1}; \
    AVFormatContext  mock_ofmt_ctx = {0}, *mock_ofmt_ctx_p = &mock_ofmt_ctx; \
    atfp_av_ctx_t  mock_avctx_src = {.fmt_ctx=&mock_ifmt_ctx, .stream_ctx={ \
        .decode=(AVCodecContext **)&mock_dec_ctxs_p[0] }}; \
    atfp_av_ctx_t  mock_avctx_dst = {0}; \
    json_t *mock_filtspec = json_loadb(UTEST_FILT_SPEC, sizeof(UTEST_FILT_SPEC) - 1, (size_t)0, NULL); \
    json_t *mock_err_info = json_object();

#define   UTEST_FFM_INIT_TEARDOWN \
    json_decref(mock_filtspec); \
    json_decref(mock_err_info);

Ensure(atfp_img_ffo_test__avinit_ok)
{
    UTEST_FFM_INIT_SETUP
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(avformat_alloc_output_context2, will_return(0),  when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder)));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_av_encctx)));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_av_encctx)));
        expect(avformat_write_header, will_return(0), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_init (&mock_avctx_src, &mock_avctx_dst, mock_outfile_path,
         mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(&mock_st_enc_ctxs[0]));
    assert_that(mock_avctx_dst.stream_ctx.encode[0].enc_ctx, is_equal_to(&mock_av_encctx));
    assert_that(mock_av_encctx.width,  is_equal_to(UTEST_SCALE_WIDTH));
    assert_that(mock_av_encctx.height, is_equal_to(UTEST_SCALE_HEIGHT));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(av_write_trailer, will_return(0), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit (&mock_avctx_dst);
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avinit_ok


Ensure(atfp_img_ffo_test__av_encoder_error)
{
    UTEST_FFM_INIT_SETUP
    (void)mock_av_encoder;
    (void)mock_av_encctx;
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(avformat_alloc_output_context2, will_return(0),  when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(NULL));
    }
    atfp__image_dst__avctx_init (&mock_avctx_src, &mock_avctx_dst, mock_outfile_path,
         mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(&mock_st_enc_ctxs[0]));
    assert_that(mock_avctx_dst.stream_ctx.encode[0].enc_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit (&mock_avctx_dst);
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__av_encoder_error


Ensure(atfp_img_ffo_test__av_encctx_open_error)
{
    UTEST_FFM_INIT_SETUP
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(avformat_alloc_output_context2, will_return(0),  when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder)));
        expect(avcodec_open2, will_return(AVERROR(EIO)), when(ctx, is_equal_to(&mock_av_encctx)));
    }
    atfp__image_dst__avctx_init (&mock_avctx_src, &mock_avctx_dst, mock_outfile_path,
         mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit (&mock_avctx_dst);
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__av_encctx_open_error


Ensure(atfp_img_ffo_test__av_write_header_error)
{
    UTEST_FFM_INIT_SETUP
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(avformat_alloc_output_context2, will_return(0),  when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder)));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_av_encctx)));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_av_encctx)));
        expect(avformat_write_header, will_return(AVERROR(EPERM)), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
        expect(av_strerror);
        expect(av_log);
    }
    atfp__image_dst__avctx_init (&mock_avctx_src, &mock_avctx_dst, mock_outfile_path,
         mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit (&mock_avctx_dst);
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__av_write_header_error


TestSuite *app_transcoder_img_ffm_out_avctx_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffo_test__avinit_ok);
    add_test(suite, atfp_img_ffo_test__av_encoder_error);
    add_test(suite, atfp_img_ffo_test__av_encctx_open_error);
    add_test(suite, atfp_img_ffo_test__av_write_header_error);
    return suite;
}
