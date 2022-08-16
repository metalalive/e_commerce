#include <unistd.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>

#include "transcoder/datatypes.h"
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

#define  LOCAL_BASEPATH  "tmp/buffer/media/test"
#define  UNITTEST_FOLDER_NAME   "utest"
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)

#define  UNITTEST_AVCTX_INIT__SETUP \
    int idx = 0; \
    char local_path[] = LOCAL_BASEPATH "/" UNITTEST_FOLDER_NAME; \
    const char *expect_mux_fmt = "hls"; \
    AVStream   mock_av_streams_src[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVStream  *mock_av_streams_src_p[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; \
            mock_av_streams_src_p[idx] = &mock_av_streams_src[idx], idx++); \
    AVFormatContext   mock_ifmt_ctx = {.nb_streams=EXPECT_NB_STREAMS_IFMT_CTX, .streams=mock_av_streams_src_p}; \
    AVFormatContext   mock_ofmt_ctx = {0}; \
    AVFormatContext  *mock_ofmt_ctx_p = &mock_ofmt_ctx; \
    json_t *mock_errinfo = json_object(); \
    json_t *mock_spec = json_object(); \
    json_object_set_new(mock_spec, "container", json_string(expect_mux_fmt)); \
    void *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    void *asadst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_op_base_cfg_t  mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asasrc_cb_args}}; \
    asa_op_base_cfg_t  mock_asa_dst = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asadst_cb_args}}; \
    atfp_asa_map_t  *mock_map = atfp_asa_map_init(1); \
    atfp_asa_map_set_source(mock_map, &mock_asa_src); \
    atfp_asa_map_add_destination(mock_map, &mock_asa_dst); \
    AVCodec mock_codecs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVCodecContext  mock_decoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVCodecContext *mock_decoder_ctx_ptrs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; mock_decoder_ctxs[idx].codec = &mock_codecs[idx] \
            , mock_decoder_ctx_ptrs[idx] = &mock_decoder_ctxs[idx], idx++); \
    atfp_av_ctx_t  mock_avctx_src = {.fmt_ctx=&mock_ifmt_ctx, .stream_ctx={.decode=mock_decoder_ctx_ptrs}}; \
    atfp_av_ctx_t  mock_avctx_dst = {.fmt_ctx=mock_ofmt_ctx_p}; \
    atfp_hls_t  mock_fp_src = { .av=&mock_avctx_src, \
        .super={.data={.spec=mock_spec, .error=mock_errinfo, .storage={.handle=&mock_asa_dst}}, \
            .backend_id=ATFP_BACKEND_LIB__FFMPEG}, \
    }; \
    atfp_hls_t  mock_fp_dst = { .av=&mock_avctx_dst, \
        .super={.data={.spec=mock_spec, .error=mock_errinfo, .storage={.handle=&mock_asa_dst}}, \
            .backend_id=ATFP_BACKEND_LIB__FFMPEG}, \
        .asa_local={.super={.op={.mkdir={.path={.origin=&local_path[0]}}}}} \
    }; \
    asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp_src; \
    asadst_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp_dst;

#define  UNITTEST_AVCTX_INIT__TEARDOWN \
    json_decref(mock_spec); \
    json_decref(mock_errinfo); \
    atfp_asa_map_deinit(mock_map);


Ensure(atfp_hls_test__avctx_init_ok) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   3
    // setup  for all test cases of current file
    UNITTEST_AVCTX_INIT__SETUP;
    // setup  only for this test case
    AVStream  mock_av_streams_dst[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    atfp_stream_enc_ctx_t  mock_st_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    AVCodecContext  mock_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    enum AVSampleFormat  mock_audio_sample_fmt[2] = {AV_SAMPLE_FMT_U8, AV_SAMPLE_FMT_FLTP};
    {
        expect(avformat_alloc_output_context2, will_return(0), when(fmt_name, is_equal_to_string(expect_mux_fmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext **)),
        );
        expect(av_mallocz_array, will_return(&mock_st_encoder_ctxs[0]),
                when(nmemb, is_equal_to(EXPECT_NB_STREAMS_IFMT_CTX)),
                when(sz, is_equal_to(sizeof(atfp_stream_enc_ctx_t)))
            );
        mock_decoder_ctxs[0].codec_type = AVMEDIA_TYPE_VIDEO;
        mock_decoder_ctxs[1].codec_type = AVMEDIA_TYPE_AUDIO;
        mock_decoder_ctxs[2].codec_type = AVMEDIA_TYPE_SUBTITLE;
        mock_codecs[1].sample_fmts = &mock_audio_sample_fmt[0];
        for(idx = 0; idx < EXPECT_NB_STREAMS_IFMT_CTX; idx++) {
            expect(avformat_new_stream, will_return(&mock_av_streams_dst[idx]),
                    when(s, is_equal_to(&mock_ofmt_ctx)));
        } // end of for-loop
        expect(avcodec_alloc_context3, will_return(&mock_encoder_ctxs[0]));
        expect(avcodec_alloc_context3, will_return(&mock_encoder_ctxs[1]));
        int expect_num_channels_audio = 5;
        expect(av_get_channel_layout_nb_channels, will_return(expect_num_channels_audio));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_encoder_ctxs[1])));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctxs[1])));
        expect(avcodec_parameters_copy); // for other stream types 
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_playlist_type")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_segment_type")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_time")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_delete_threshold")));
        expect(av_dict_set,  when(key, is_equal_to_string("hls_segment_filename")));
        expect(av_dict_set,  when(key, is_equal_to_string("hls_fmp4_init_filename")));
        expect(avformat_write_header, will_return(0),  when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
        expect(av_dict_free);
        expect(av_dump_format);
    }
    int err = atfp_hls__av_init(&mock_fp_dst);
    assert_that(err, is_equal_to(0));
    UNITTEST_AVCTX_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avctx_init_ok


Ensure(atfp_hls_test__avctx_init__fmtctx_error) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   1
    UNITTEST_AVCTX_INIT__SETUP;
    int expect_err = AVERROR(ENOMEM);
    {
        mock_ofmt_ctx_p = NULL;
        expect(avformat_alloc_output_context2, will_return(expect_err), when(fmt_name, is_equal_to_string(expect_mux_fmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext **)),
        );
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx_dst.intermediate_data.encode.packet)));
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx_dst.intermediate_data.encode.frame)));
    }
    int err = atfp_hls__av_init(&mock_fp_dst);
    assert_that(err, is_equal_to(expect_err));
    json_t *err_detail = json_object_get(mock_fp_dst.super.data.error, "transcoder");
    assert_that(err_detail, is_not_equal_to(NULL));
    UNITTEST_AVCTX_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avctx_init__fmtctx_error


Ensure(atfp_hls_test__avctx_init__invalid_backend_lib) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   1
    UNITTEST_AVCTX_INIT__SETUP;
    {
        mock_fp_src.super.backend_id = ATFP_BACKEND_LIB__LIBVLC;
        mock_fp_dst.super.backend_id = ATFP_BACKEND_LIB__FFMPEG;
        expect(avformat_alloc_output_context2, will_return(0), when(fmt_name, is_equal_to_string(expect_mux_fmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext **)),
        );
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx_dst.intermediate_data.encode.packet)));
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx_dst.intermediate_data.encode.frame)));
        expect(avformat_free_context,  when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    int err = atfp_hls__av_init(&mock_fp_dst);
    assert_that(err, is_equal_to(AVERROR(EINVAL)));
    json_t *err_detail = json_object_get(mock_fp_dst.super.data.error, "transcoder");
    assert_that(err_detail, is_not_equal_to(NULL));
    UNITTEST_AVCTX_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avctx_init__invalid_backend_lib


Ensure(atfp_hls_test__avctx_init__audio_codec_error) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   3
    UNITTEST_AVCTX_INIT__SETUP;
    AVStream  mock_av_streams_dst[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    atfp_stream_enc_ctx_t   mock_st_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    AVCodecContext  mock_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    enum AVSampleFormat  mock_audio_sample_fmt[2] = {AV_SAMPLE_FMT_U8, AV_SAMPLE_FMT_FLTP};
    int expect_err = AVERROR(EIO);
    {
        expect(avformat_alloc_output_context2, will_return(0), when(fmt_name, is_equal_to_string(expect_mux_fmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext **)),
        );
        expect(av_mallocz_array, will_return(&mock_st_encoder_ctxs[0]),
                when(nmemb, is_equal_to(EXPECT_NB_STREAMS_IFMT_CTX)),
                when(sz, is_equal_to(sizeof(atfp_stream_enc_ctx_t)))
            );
        mock_decoder_ctxs[0].codec_type = AVMEDIA_TYPE_VIDEO;
        mock_decoder_ctxs[1].codec_type = AVMEDIA_TYPE_SUBTITLE;
        mock_decoder_ctxs[2].codec_type = AVMEDIA_TYPE_AUDIO;
        mock_codecs[2].sample_fmts = &mock_audio_sample_fmt[0];
        for(idx = 0; idx < EXPECT_NB_STREAMS_IFMT_CTX; idx++) {
            expect(avformat_new_stream, will_return(&mock_av_streams_dst[idx]),
                    when(s, is_equal_to(&mock_ofmt_ctx)));
        } // end of for-loop
        expect(avcodec_alloc_context3, will_return(&mock_encoder_ctxs[0]));
        expect(avcodec_alloc_context3, will_return(&mock_encoder_ctxs[2]));
        int expect_num_channels_audio = 7;
        expect(av_get_channel_layout_nb_channels, will_return(expect_num_channels_audio));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_open2, will_return(expect_err), when(ctx, is_equal_to(&mock_encoder_ctxs[2])));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_parameters_copy); // for other stream types 
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx_dst.intermediate_data.encode.packet)));
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx_dst.intermediate_data.encode.frame)));
        expect(av_freep,  when(ptr, is_equal_to(&mock_avctx_dst.stream_ctx.encode))); // pointer to mock_st_encoder_ctxs
        expect(avcodec_free_context,  when(ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_free_context,  when(ctx, is_equal_to(&mock_encoder_ctxs[2])));
        expect(avformat_free_context,  when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    int err = atfp_hls__av_init(&mock_fp_dst);
    assert_that(err, is_equal_to(expect_err));
    UNITTEST_AVCTX_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avctx_init__audio_codec_error


Ensure(atfp_hls_test__avctx_init__white_header_error) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   1
    UNITTEST_AVCTX_INIT__SETUP;
    AVStream  mock_av_streams_dst[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    atfp_stream_enc_ctx_t   mock_st_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    AVCodecContext  mock_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0};
    int expect_err = AVERROR(EPERM);
    {
        expect(avformat_alloc_output_context2, will_return(0), when(fmt_name, is_equal_to_string(expect_mux_fmt)),
            will_set_contents_of_parameter(fmtctx_p, &mock_ofmt_ctx_p, sizeof(AVFormatContext **)),
        );
        expect(av_mallocz_array, will_return(&mock_st_encoder_ctxs[0]),
                when(nmemb, is_equal_to(EXPECT_NB_STREAMS_IFMT_CTX)),
                when(sz, is_equal_to(sizeof(atfp_stream_enc_ctx_t)))
            );
        mock_decoder_ctxs[0].codec_type = AVMEDIA_TYPE_VIDEO;
        expect(avformat_new_stream, will_return(&mock_av_streams_dst[0]),
                when(s, is_equal_to(&mock_ofmt_ctx)));
        expect(avcodec_alloc_context3, will_return(&mock_encoder_ctxs[0]));
        expect(avcodec_open2, will_return(0), when(ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avcodec_parameters_from_context, will_return(0), when(codec_ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_playlist_type")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_segment_type")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_time")));
        expect(av_dict_set_int,  when(key, is_equal_to_string("hls_delete_threshold")));
        expect(av_dict_set,  when(key, is_equal_to_string("hls_segment_filename")));
        expect(av_dict_set,  when(key, is_equal_to_string("hls_fmp4_init_filename")));
        expect(avformat_write_header, will_return(expect_err),  when(fmt_ctx, is_equal_to(mock_ofmt_ctx_p)));
        expect(av_strerror);
        expect(av_log);
        expect(av_dict_free);
        expect(av_packet_unref, when(pkt, is_equal_to(&mock_avctx_dst.intermediate_data.encode.packet)));
        expect(av_frame_unref,  when(frame, is_equal_to(&mock_avctx_dst.intermediate_data.encode.frame)));
        expect(av_freep,  when(ptr, is_equal_to(&mock_avctx_dst.stream_ctx.encode))); // pointer to mock_st_encoder_ctxs
        expect(avcodec_free_context,  when(ctx, is_equal_to(&mock_encoder_ctxs[0])));
        expect(avformat_free_context,  when(s, is_equal_to(mock_ofmt_ctx_p)));
    }
    int err = atfp_hls__av_init(&mock_fp_dst);
    assert_that(err, is_equal_to(expect_err));
    UNITTEST_AVCTX_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avctx_init__white_header_error


TestSuite *app_transcoder_hls_avcontext_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__avctx_init_ok);
    add_test(suite, atfp_hls_test__avctx_init__fmtctx_error);
    add_test(suite, atfp_hls_test__avctx_init__invalid_backend_lib);
    add_test(suite, atfp_hls_test__avctx_init__audio_codec_error);
    add_test(suite, atfp_hls_test__avctx_init__white_header_error);
    return suite;
}
