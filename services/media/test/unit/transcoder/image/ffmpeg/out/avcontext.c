#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/ffmpeg.h"

#define STRINGIFY(x)       #x
#define UTEST_CROP_WIDTH   450
#define UTEST_CROP_HEIGHT  250
#define UTEST_SCALE_WIDTH  430
#define UTEST_SCALE_HEIGHT 220
#define UTEST_FILT_SPEC \
    "{\"crop\":{\"width\":" STRINGIFY(450) ", \"height\":" STRINGIFY(250 \
    ) "}," \
      "\"scale\":{\"width\":" STRINGIFY(430) ", \"height\":" STRINGIFY(220) "}}"

#define UTEST_FFM_INIT_SETUP \
    const char     *mock_outfile_path = "/path/to/out/file"; \
    AVCodec         mock_av_encoder = {0}; \
    AVCodecContext  mock_av_encctx = {0}; \
    AVCodecContext  mock_dec_ctxs[1] = {{.time_base = {.num = 1, .den = 1}, .codec_type = AVMEDIA_TYPE_VIDEO} \
    }; \
    AVCodecContext *mock_dec_ctxs_p[1] = {&mock_dec_ctxs[0]}; \
    atfp_stream_enc_ctx_t mock_st_enc_ctxs[1] = {0}; \
    AVCodecParameters     mock_codec_param = {0}; \
    AVStream              mock_vdo_stream = {.codecpar = &mock_codec_param}; \
    AVOutputFormat        mock_ofmt = {0}; \
    AVInputFormat         mock_ifmt = {0}; \
    AVFormatContext       mock_ifmt_ctx = {.nb_streams = 1, .iformat = &mock_ifmt}; \
    AVFormatContext       mock_ofmt_ctx = {0}, *mock_ofmt_ctx_p = &mock_ofmt_ctx; \
    AVIOContext           mock_avio = {0}, *mock_avio_p = &mock_avio; \
    atfp_av_ctx_t         mock_avctx_src = { \
                .fmt_ctx = &mock_ifmt_ctx, .stream_ctx = {.decode = (AVCodecContext **)&mock_dec_ctxs_p[0]} \
    }; \
    atfp_av_ctx_t mock_avctx_dst = {0}; \
    json_t       *mock_filtspec = json_loadb(UTEST_FILT_SPEC, sizeof(UTEST_FILT_SPEC) - 1, (size_t)0, NULL); \
    json_t       *mock_err_info = json_object();

#define UTEST_FFM_INIT_TEARDOWN \
    json_decref(mock_filtspec); \
    json_decref(mock_err_info);

Ensure(atfp_img_ffo_test__avinit_ok) {
    UTEST_FFM_INIT_SETUP {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(
            avformat_alloc_output_context2, will_return(0), when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(
            avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder))
        );
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_av_encctx)));
        expect(
            avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_av_encctx))
        );
        expect(
            avio_open, will_return(0), when(filename, is_equal_to_string(mock_outfile_path)),
            will_set_contents_of_parameter(ioc_p, &mock_avio_p, sizeof(AVIOContext *)),
            when(flags, is_equal_to(AVIO_FLAG_WRITE))
        );
        expect(avformat_write_header, will_return(0), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_init(
        &mock_avctx_src, &mock_avctx_dst, mock_outfile_path, mock_filtspec, mock_err_info
    );
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.fmt_ctx->pb, is_equal_to(mock_avio_p));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(&mock_st_enc_ctxs[0]));
    assert_that(mock_avctx_dst.stream_ctx.encode[0].enc_ctx, is_equal_to(&mock_av_encctx));
    assert_that(mock_av_encctx.width, is_equal_to(UTEST_SCALE_WIDTH));
    assert_that(mock_av_encctx.height, is_equal_to(UTEST_SCALE_HEIGHT));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(av_write_trailer, will_return(0), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avio_closep, when(ioc, is_equal_to(mock_avio_p)));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit(&mock_avctx_dst);
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(1));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avinit_ok

Ensure(atfp_img_ffo_test__avinit_encoder_error) {
    UTEST_FFM_INIT_SETUP(void) mock_av_encoder;
    (void)mock_av_encctx;
    (void)mock_avio;
    (void)mock_avio_p;
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(
            avformat_alloc_output_context2, will_return(0), when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(NULL));
    }
    atfp__image_dst__avctx_init(
        &mock_avctx_src, &mock_avctx_dst, mock_outfile_path, mock_filtspec, mock_err_info
    );
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(&mock_st_enc_ctxs[0]));
    assert_that(mock_avctx_dst.stream_ctx.encode[0].enc_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avio_closep, when(ioc, is_equal_to(NULL)));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit(&mock_avctx_dst);
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avinit_encoder_error

Ensure(atfp_img_ffo_test__avinit_encctx_open_error) {
    UTEST_FFM_INIT_SETUP(void) mock_avio;
    (void)mock_avio_p;
    {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(
            avformat_alloc_output_context2, will_return(0), when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(
            avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder))
        );
        expect(avcodec_open2, will_return(AVERROR(EIO)), when(ctx, is_equal_to(&mock_av_encctx)));
    }
    atfp__image_dst__avctx_init(
        &mock_avctx_src, &mock_avctx_dst, mock_outfile_path, mock_filtspec, mock_err_info
    );
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avio_closep, when(ioc, is_equal_to(NULL)));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit(&mock_avctx_dst);
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avinit_encctx_open_error

Ensure(atfp_img_ffo_test__avinit_write_header_error) {
    UTEST_FFM_INIT_SETUP {
        expect(av_guess_format, will_return(&mock_ofmt));
        expect(
            avformat_alloc_output_context2, will_return(0), when(oformat, is_equal_to(&mock_ofmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext *)),
            when(filename, is_equal_to_string(mock_outfile_path)),
        );
        expect(av_mallocz_array, will_return(&mock_st_enc_ctxs[0]), when(nmemb, is_equal_to(1)));
        expect(avformat_new_stream, will_return(&mock_vdo_stream), when(s, is_equal_to(mock_ofmt_ctx_p)));
        expect(avcodec_find_encoder, will_return(&mock_av_encoder));
        expect(
            avcodec_alloc_context3, will_return(&mock_av_encctx), when(codec, is_equal_to(&mock_av_encoder))
        );
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_av_encctx)));
        expect(
            avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_av_encctx))
        );
        expect(
            avio_open, will_return(0), when(filename, is_equal_to_string(mock_outfile_path)),
            will_set_contents_of_parameter(ioc_p, &mock_avio_p, sizeof(AVIOContext *)),
            when(flags, is_equal_to(AVIO_FLAG_WRITE))
        );
        expect(
            avformat_write_header, will_return(AVERROR(EPERM)), when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p))
        );
        expect(av_strerror);
        expect(av_log);
    }
    atfp__image_dst__avctx_init(
        &mock_avctx_src, &mock_avctx_dst, mock_outfile_path, mock_filtspec, mock_err_info
    );
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(mock_ofmt_ctx_p));
    assert_that(mock_avctx_dst.fmt_ctx->pb, is_equal_to(mock_avio_p));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    {
        expect(avcodec_free_context, when(ctx, is_equal_to(&mock_av_encctx)));
        expect(av_freep, when(ptr2obj, is_equal_to(&mock_st_enc_ctxs[0])));
        expect(avio_closep, when(ioc, is_equal_to(mock_avio_p)));
        expect(avformat_free_context, when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    atfp__image_dst__avctx_deinit(&mock_avctx_dst);
    assert_that(mock_avctx_dst.fmt_ctx, is_equal_to(NULL));
    assert_that(mock_avctx_dst.stream_ctx.encode, is_equal_to(NULL));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_header_wrote, is_equal_to(0));
    assert_that(mock_avctx_dst.intermediate_data.encode._final.file_trailer_wrote, is_equal_to(0));
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avinit_write_header_error

#define UTEST_FFM_ENCODE_PROCESS_SETUP \
    AVStream              mock_av_streams[1] = {0}, *mock_av_streams_p[1] = {&mock_av_streams[0]}; \
    AVFormatContext       mock_av_ofmt_ctx = {.streams = &mock_av_streams_p[0]}; \
    AVCodecContext        mock_encoder_ctx = {0}; \
    atfp_stream_enc_ctx_t mock_st_encode_ctx = {.enc_ctx = &mock_encoder_ctx}; \
    atfp_av_ctx_t mock_avctx = {.stream_ctx = {.encode = &mock_st_encode_ctx}, .fmt_ctx = &mock_av_ofmt_ctx};

Ensure(atfp_img_ffo_test__av_encode_ok) {
    UTEST_FFM_ENCODE_PROCESS_SETUP
    expect(
        avcodec_send_frame, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(frame, is_equal_to(&mock_avctx.intermediate_data.encode.frame))
    );
    expect(
        avcodec_receive_packet, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(avpkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet))
    );
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet)));
    int ret = atfp__image_dst__encode_frame(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__OK));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(1));
    // assume there is another encoded packet from the same filtered frame
    expect(
        avcodec_receive_packet, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(avpkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet))
    );
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet)));
    ret = atfp__image_dst__encode_frame(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__OK));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(2));
} // end of atfp_img_ffo_test__av_encode_ok

Ensure(atfp_img_ffo_test__av_encode_no_more_pkt) {
    UTEST_FFM_ENCODE_PROCESS_SETUP
    mock_avctx.intermediate_data.encode.num_encoded_pkts = 3;
    expect(avcodec_receive_packet, will_return(AVERROR_EOF), when(codec_ctx, is_equal_to(&mock_encoder_ctx)));
    int ret = atfp__image_dst__encode_frame(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__NEED_MORE_DATA));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(0));
}

Ensure(atfp_img_ffo_test__av_encode_final_ok) {
    UTEST_FFM_ENCODE_PROCESS_SETUP
    expect(
        avcodec_send_frame, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(frame, is_equal_to(NULL))
    );
    expect(
        avcodec_receive_packet, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(avpkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet))
    );
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet)));
    int ret = atfp__image_dst__flushing_encoder(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__OK));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(1));
    // assume there is another encoded packet from the same filtered frame
    expect(
        avcodec_receive_packet, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctx)),
        when(avpkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet))
    );
    expect(av_packet_rescale_ts, when(pkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet)));
    ret = atfp__image_dst__flushing_encoder(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__OK));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(2));
    expect(avcodec_receive_packet, will_return(AVERROR_EOF), when(codec_ctx, is_equal_to(&mock_encoder_ctx)));
    ret = atfp__image_dst__flushing_encoder(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__NEED_MORE_DATA));
    assert_that(mock_avctx.intermediate_data.encode.num_encoded_pkts, is_equal_to(0));
    ret = atfp__image_dst__flushing_encoder(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER));
    ret = atfp__image_dst__flushing_encoder(&mock_avctx);
    assert_that(ret, is_equal_to(ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER));
} // end of  atfp_img_ffo_test__av_encode_final_ok

Ensure(atfp_img_ffo_test__av_write_pkt_ok) {
    AVFormatContext mock_av_ofmt_ctx = {0};
    atfp_av_ctx_t   mock_avctx = {.fmt_ctx = &mock_av_ofmt_ctx};
    expect(
        av_write_frame, will_return(0), when(fmt_ctx, is_equal_to(&mock_av_ofmt_ctx)),
        when(pkt, is_equal_to(&mock_avctx.intermediate_data.encode.packet))
    );
    int ret = atfp__image_dst__write_encoded_packet(&mock_avctx);
    assert_that(ret, is_equal_to(0));
}

Ensure(atfp_img_ffo_test__av_write_final_ok) {
    AVFormatContext mock_av_ofmt_ctx = {0};
    atfp_av_ctx_t   mock_avctx = {
          .fmt_ctx = &mock_av_ofmt_ctx,
          .intermediate_data = {.encode = {._final = {.file_trailer_wrote = 0, .file_header_wrote = 1}}}
    };
    expect(av_write_trailer, will_return(0), when(fmt_ctx, is_equal_to(&mock_av_ofmt_ctx)));
    int ret = atfp__image_dst__final_writefile(&mock_avctx);
    assert_that(ret, is_equal_to(0));
    ret = atfp__image_dst__final_writefile(&mock_avctx);
    assert_that(ret, is_equal_to(0));
    ret = atfp__image_dst__final_writefile(&mock_avctx);
    assert_that(ret, is_equal_to(0));
}

TestSuite *app_transcoder_img_ffm_out_avctx_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffo_test__avinit_ok);
    add_test(suite, atfp_img_ffo_test__avinit_encoder_error);
    add_test(suite, atfp_img_ffo_test__avinit_encctx_open_error);
    add_test(suite, atfp_img_ffo_test__avinit_write_header_error);
    add_test(suite, atfp_img_ffo_test__av_encode_ok);
    add_test(suite, atfp_img_ffo_test__av_encode_no_more_pkt);
    add_test(suite, atfp_img_ffo_test__av_encode_final_ok);
    add_test(suite, atfp_img_ffo_test__av_write_pkt_ok);
    add_test(suite, atfp_img_ffo_test__av_write_final_ok);
    return suite;
}
