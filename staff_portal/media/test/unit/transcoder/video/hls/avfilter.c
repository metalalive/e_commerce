#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>

#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)

#define  UNITTEST_AVFILT_INIT__SETUP \
    int idx = 0; \
    AVCodecParameters  mock_codecpar[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVStream   mock_av_streams_src[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVStream  *mock_av_streams_src_p[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; mock_av_streams_src[idx].codecpar = &mock_codecpar[idx], \
            mock_av_streams_src_p[idx] = &mock_av_streams_src[idx], idx++); \
    AVFormatContext   mock_ifmt_ctx = {.nb_streams=EXPECT_NB_STREAMS_IFMT_CTX, .streams=mock_av_streams_src_p}; \
    AVFormatContext   mock_ofmt_ctx = {0}; \
    AVCodecContext  mock_decoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVCodecContext *mock_decoder_ctx_ptrs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; mock_decoder_ctx_ptrs[idx] = &mock_decoder_ctxs[idx], idx++); \
    atfp_stream_enc_ctx_t  mock_st_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVCodecContext  mock_encoder_ctxs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; mock_st_encoder_ctxs[idx].enc_ctx = &mock_encoder_ctxs[idx], idx++); \
    AVFilterInOut  mock_filt_outputs[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilterInOut  mock_filt_inputs[EXPECT_NB_STREAMS_IFMT_CTX]  = {0}; \
    AVFilterGraph  mock_filt_graph[EXPECT_NB_STREAMS_IFMT_CTX]   = {0}; \
    AVFilter  mock_av_filt_src[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilter  mock_av_filt_sink[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilterContext  mock_filt_ctx_src[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilterContext  mock_filt_ctx_sink[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilterContext *mock_filt_ctx_src_p[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    AVFilterContext *mock_filt_ctx_sink_p[EXPECT_NB_STREAMS_IFMT_CTX] = {0}; \
    for(idx=0; idx<EXPECT_NB_STREAMS_IFMT_CTX; mock_filt_ctx_src_p[idx] = &mock_filt_ctx_src[idx], \
            mock_filt_ctx_sink_p[idx] = &mock_filt_ctx_sink[idx], idx++); \
    atfp_av_ctx_t  mock_avctx_src = {.fmt_ctx=&mock_ifmt_ctx, .stream_ctx={.decode=mock_decoder_ctx_ptrs}}; \
    atfp_av_ctx_t  mock_avctx_dst = {.fmt_ctx=&mock_ofmt_ctx, .stream_ctx={.encode=&mock_st_encoder_ctxs[0]}}; \
    void *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    void *asadst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_op_base_cfg_t  mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asasrc_cb_args}}; \
    asa_op_base_cfg_t  mock_asa_dst = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asadst_cb_args}}; \
    atfp_asa_map_t  *mock_map = atfp_asa_map_init(1); \
    atfp_asa_map_set_source(mock_map, &mock_asa_src); \
    atfp_asa_map_add_destination(mock_map, &mock_asa_dst); \
    atfp_hls_t  mock_fp_src = { .av=&mock_avctx_src, .super={.data={.storage={.handle=&mock_asa_dst}}}}; \
    atfp_hls_t  mock_fp_dst = { .av=&mock_avctx_dst, .super={.data={.storage={.handle=&mock_asa_dst}}}}; \
    asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp_src; \
    asadst_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp_dst;


#define  UNITTEST_AVFILT_INIT__TEARDOWN \
    atfp_asa_map_deinit(mock_map);


Ensure(atfp_hls_test__avfilter_init_ok) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   3
    UNITTEST_AVFILT_INIT__SETUP;
    {
        mock_codecpar[0].codec_type = AVMEDIA_TYPE_SUBTITLE;
        mock_codecpar[1].codec_type = AVMEDIA_TYPE_VIDEO;
        mock_codecpar[2].codec_type = AVMEDIA_TYPE_AUDIO;
        mock_st_encoder_ctxs[0].enc_ctx  = NULL; // assume non-a/v stream doesn't have encoder context
        mock_decoder_ctxs[1].framerate = (AVRational) {.num=17, .den=1};
        mock_encoder_ctxs[1].framerate = (AVRational) {.num=7, .den=1};
        for(idx=1; idx<EXPECT_NB_STREAMS_IFMT_CTX; idx++) {
            expect(avfilter_inout_alloc, will_return(&mock_filt_outputs[idx]));
            expect(avfilter_inout_alloc, will_return(&mock_filt_inputs[idx] ));
            expect(avfilter_graph_alloc, will_return(&mock_filt_graph[idx]  ));
            expect(avfilter_get_by_name, will_return(&mock_av_filt_src[idx]  ));
            expect(avfilter_get_by_name, will_return(&mock_av_filt_sink[idx] ));
            expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("in")),
                    will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_src_p[idx], sizeof(AVFilterContext **)),
                    when(graph_ctx, is_equal_to(&mock_filt_graph[idx])),
                    when(filt, is_equal_to(&mock_av_filt_src[idx])) );
            expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("out")),
                    will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_sink_p[idx], sizeof(AVFilterContext **)),
                    when(graph_ctx, is_equal_to(&mock_filt_graph[idx])),
                    when(filt, is_equal_to(&mock_av_filt_sink[idx])) );
            expect(avfilter_graph_parse_ptr, will_return(0), when(graph, is_equal_to(&mock_filt_graph[idx])),
                    when(outputs, is_equal_to(&mock_filt_outputs[idx])),
                    when(inputs,  is_equal_to(&mock_filt_inputs[idx])),
                  );
            expect(avfilter_graph_config, will_return(0), when(graph, is_equal_to(&mock_filt_graph[idx])) );
            expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_inputs[idx])));
            expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_outputs[idx])));
        } // end of loop
        { // subtitle stream only
            expect(av_log);
        } { // video stream only
            AVRational  mock_frame_ratio = {.num=3, .den=1};
            expect(av_mul_q, will_return(&mock_frame_ratio));
            expect(av_opt_set_bin, will_return(0),   when(name, is_equal_to_string("pix_fmts")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[1])));
        } { // audio stream only
            expect(av_get_sample_fmt_name, will_return("mock_format"));
            expect(av_get_default_channel_layout, will_return(4));
            expect(av_opt_set_bin, will_return(0),   when(name, is_equal_to_string("sample_fmts")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[2])));
            expect(av_opt_set_bin, will_return(0),   when(name, is_equal_to_string("channel_layouts")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[2])));
            // av_opt_set_int_list will expand to `av_opt_set_bin` and `av_int_list_length_for_size`
            expect(av_opt_set_bin, will_return(0),   when(name, is_equal_to_string("sample_rates")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[2])));
            expect(av_int_list_length_for_size, will_return(2));
        }
    }
    int err = atfp_hls__avfilter_init(&mock_fp_dst);
    assert_that(err, is_equal_to(0));
    for(idx=1; idx<EXPECT_NB_STREAMS_IFMT_CTX; idx++) {
        assert_that(mock_st_encoder_ctxs[idx].filter_graph, is_equal_to(&mock_filt_graph[idx]));
        assert_that(mock_st_encoder_ctxs[idx].filt_sink_ctx, is_equal_to(&mock_filt_ctx_sink[idx]));
        assert_that(mock_st_encoder_ctxs[idx].filt_src_ctx, is_equal_to(&mock_filt_ctx_src[idx]));
    }
    UNITTEST_AVFILT_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avfilter_init_ok


Ensure(atfp_hls_test__avfilter_init_video_error) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   1
    UNITTEST_AVFILT_INIT__SETUP;
    int expect_err = AVERROR(E2BIG);
    {
        mock_codecpar[0].codec_type = AVMEDIA_TYPE_VIDEO;
        mock_decoder_ctxs[0].framerate = (AVRational) {.num=18, .den=1};
        mock_encoder_ctxs[0].framerate = (AVRational) {.num=7, .den=1};
        expect(avfilter_inout_alloc, will_return(&mock_filt_outputs[0]));
        expect(avfilter_inout_alloc, will_return(&mock_filt_inputs[0] ));
        expect(avfilter_graph_alloc, will_return(&mock_filt_graph[0]  ));
        expect(avfilter_get_by_name, will_return(&mock_av_filt_src[0]  ));
        expect(avfilter_get_by_name, will_return(&mock_av_filt_sink[0] ));
        expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("in")),
                will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_src_p[0], sizeof(AVFilterContext **)),
                when(graph_ctx, is_equal_to(&mock_filt_graph[0])),
                when(filt, is_equal_to(&mock_av_filt_src[0])) );
        expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("out")),
                will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_sink_p[0], sizeof(AVFilterContext **)),
                when(graph_ctx, is_equal_to(&mock_filt_graph[0])),
                when(filt, is_equal_to(&mock_av_filt_sink[0])) );
        {
            AVRational  mock_frame_ratio = {.num=4, .den=1};
            expect(av_mul_q, will_return(&mock_frame_ratio));
            expect(av_opt_set_bin, will_return(expect_err),   when(name, is_equal_to_string("pix_fmts")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[0])));
            expect(av_log);
        }
        expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_inputs[0])));
        expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_outputs[0])));
    }
    int err = atfp_hls__avfilter_init(&mock_fp_dst);
    assert_that(err, is_equal_to(expect_err));
    assert_that(mock_st_encoder_ctxs[0].filter_graph,  is_equal_to(&mock_filt_graph[0]));
    assert_that(mock_st_encoder_ctxs[0].filt_sink_ctx, is_equal_to(&mock_filt_ctx_sink[0]));
    assert_that(mock_st_encoder_ctxs[0].filt_src_ctx,  is_equal_to(&mock_filt_ctx_src[0]));
    UNITTEST_AVFILT_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avfilter_init_video_error


Ensure(atfp_hls_test__avfilter_init_audio_error) {
#define  EXPECT_NB_STREAMS_IFMT_CTX   1
    UNITTEST_AVFILT_INIT__SETUP;
    int expect_err = AVERROR(ENOSYS);
    {
        mock_codecpar[0].codec_type = AVMEDIA_TYPE_AUDIO;
        expect(avfilter_inout_alloc, will_return(&mock_filt_outputs[0]));
        expect(avfilter_inout_alloc, will_return(&mock_filt_inputs[0] ));
        expect(avfilter_graph_alloc, will_return(&mock_filt_graph[0]  ));
        expect(avfilter_get_by_name, will_return(&mock_av_filt_src[0]  ));
        expect(avfilter_get_by_name, will_return(&mock_av_filt_sink[0] ));
        expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("in")),
                will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_src_p[0], sizeof(AVFilterContext **)),
                when(graph_ctx, is_equal_to(&mock_filt_graph[0])),
                when(filt, is_equal_to(&mock_av_filt_src[0])) );
        expect(avfilter_graph_create_filter, will_return(0),   when(name, is_equal_to_string("out")),
                will_set_contents_of_parameter(filt_ctx_p, &mock_filt_ctx_sink_p[0], sizeof(AVFilterContext **)),
                when(graph_ctx, is_equal_to(&mock_filt_graph[0])),
                when(filt, is_equal_to(&mock_av_filt_sink[0])) );
        {
            expect(av_get_sample_fmt_name, will_return("mock_format"));
            expect(av_get_default_channel_layout, will_return(4));
            expect(av_opt_set_bin, will_return(expect_err),   when(name, is_equal_to_string("sample_fmts")),
                    when(obj, is_equal_to(mock_filt_ctx_sink_p[0])));
            expect(av_log);
        }
        expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_inputs[0])));
        expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_outputs[0])));
    }
    int err = atfp_hls__avfilter_init(&mock_fp_dst);
    assert_that(err, is_equal_to(expect_err));
    assert_that(mock_st_encoder_ctxs[0].filter_graph,  is_equal_to(&mock_filt_graph[0]));
    assert_that(mock_st_encoder_ctxs[0].filt_sink_ctx, is_equal_to(&mock_filt_ctx_sink[0]));
    assert_that(mock_st_encoder_ctxs[0].filt_src_ctx,  is_equal_to(&mock_filt_ctx_src[0]));
    UNITTEST_AVFILT_INIT__TEARDOWN;
#undef  EXPECT_NB_STREAMS_IFMT_CTX
} // end of atfp_hls_test__avfilter_init_audio_error


TestSuite *app_transcoder_hls_avfilter_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__avfilter_init_ok);
    add_test(suite, atfp_hls_test__avfilter_init_video_error);
    add_test(suite, atfp_hls_test__avfilter_init_audio_error);
    return suite;
} // end of app_transcoder_hls_avfilter_tests
