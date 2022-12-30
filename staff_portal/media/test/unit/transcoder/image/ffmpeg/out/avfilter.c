#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"
#include "transcoder/image/ffmpeg.h"

#define  STRINGIFY(x)       #x 
#define  UTEST_ORIGIN__WIDTH   960
#define  UTEST_ORIGIN__HEIGHT  670
#define  UTEST_SPEC__CROP_WIDTH    450
#define  UTEST_SPEC__CROP_HEIGHT   250
#define  UTEST_SPEC__CROP_POS_X    13
#define  UTEST_SPEC__CROP_POS_Y    29
#define  UTEST_SPEC__SCALE_WIDTH   430
#define  UTEST_SPEC__SCALE_HEIGHT  220
#define  UTEST_MSK_PATT_LABEL    "whateverShape"
#define  UTEST_FILT_SPEC \
    "{\"crop\":{\"width\":"STRINGIFY(450)", \"height\":"STRINGIFY(250)", \"x\":"STRINGIFY(13) \
     ", \"y\":"STRINGIFY(29)"},\"scale\":{\"width\":"STRINGIFY(430)", \"height\":"STRINGIFY(220)"}, " \
     "\"mask\":{\"pattern\":\"" UTEST_MSK_PATT_LABEL "\"}}"


#define  UTEST_IMGFILT_INIT__SETUP(_msk_patt_idxpath) \
    AVFilter   mock_filt4srcbuf = {0}, mock_filt4dstbuf = {0}; \
    AVFilterInOut    mock_filt_out = {0}, mock_filt_in = {0}; \
    AVFilterContext  mock_filt_src_ctx = {0}, mock_filt_sink_ctx = {0}; \
    AVFilterContext *mock_filt_src_ctx_p = &mock_filt_src_ctx, *mock_filt_sink_ctx_p = &mock_filt_sink_ctx; \
    AVFilterGraph   mock_filt_grf = {0}; \
    AVCodecContext  mock_enc_ctxs[1] = {0}; \
    AVCodecContext  mock_dec_ctxs[1] = {{.width=UTEST_ORIGIN__WIDTH, .height=UTEST_ORIGIN__HEIGHT}}; \
    AVCodecContext *mock_dec_ctxs_p[1] = {&mock_dec_ctxs[0]}; \
    atfp_stream_enc_ctx_t  mock_st_enc_ctxs[1] = {{.enc_ctx=&mock_enc_ctxs[0]}}; \
    AVCodecParameters  mock_codecpar = {.codec_type=AVMEDIA_TYPE_VIDEO}; \
    AVStream  mock_vdo_streams[1] = {{.codecpar=&mock_codecpar}}; \
    AVStream *mock_vdo_streams_p[1] = {&mock_vdo_streams[0]} ; \
    AVFormatContext  mock_ofmt_ctx = {.nb_streams=1, .streams=(AVStream **)&mock_vdo_streams_p[0]}; \
    atfp_av_ctx_t  mock_avctx_src = {.stream_ctx={.decode=(AVCodecContext **)&mock_dec_ctxs_p[0] }}; \
    atfp_av_ctx_t  mock_avctx_dst = {.stream_ctx={.encode=&mock_st_enc_ctxs[0]}, .fmt_ctx=&mock_ofmt_ctx, }; \
    app_cfg_t     *mock_acfg = app_get_global_cfg(); \
    aav_cfg_img_t *mock__imgcfg = &mock_acfg->transcoder.output.image; \
    mock__imgcfg->mask.basepath = _msk_patt_idxpath; \
    json_t *mock_filtspec = json_loadb(UTEST_FILT_SPEC, sizeof(UTEST_FILT_SPEC) - 1, (size_t)0, NULL); \
    json_t *mock_err_info = json_object();

#define  UTEST_IMGFILT_INIT__TEARDOWN \
    mock__imgcfg->mask.basepath = NULL; \
    json_decref(mock_filtspec); \
    json_decref(mock_err_info);

Ensure(atfp_img_ffo_test__filt_init_ok)
{
    UTEST_IMGFILT_INIT__SETUP("media/data/test/image/mask")
    expect(avfilter_inout_alloc, will_return(&mock_filt_out));
    expect(avfilter_inout_alloc, will_return(&mock_filt_in));
    expect(avfilter_graph_alloc, will_return(&mock_filt_grf));
    expect(avfilter_get_by_name, will_return(&mock_filt4srcbuf), when(name, is_equal_to_string("buffer")));
    expect(avfilter_get_by_name, will_return(&mock_filt4dstbuf), when(name, is_equal_to_string("buffersink")));
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4srcbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_src_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_not_equal_to(NULL)),
       );
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4dstbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_sink_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_equal_to(NULL)),
       );
    expect(avfilter_graph_parse_ptr, will_return(0), when(graph, is_equal_to(&mock_filt_grf)),
            when(inputs, is_equal_to(&mock_filt_in)), when(outputs, is_equal_to(&mock_filt_out)),
       );
    expect(avfilter_graph_config, will_return(0));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_in)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_out)));
    atfp__image_dst__avfilt_init (&mock_avctx_src, &mock_avctx_dst, mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    assert_that(mock_st_enc_ctxs[0].filter_graph, is_equal_to(&mock_filt_grf));
    assert_that(mock_st_enc_ctxs[0].filt_src_ctx, is_equal_to(&mock_filt_src_ctx));
    assert_that(mock_st_enc_ctxs[0].filt_sink_ctx, is_equal_to(&mock_filt_sink_ctx));
    UTEST_IMGFILT_INIT__TEARDOWN
} // end of  atfp_img_ffo_test__filt_init_ok


Ensure(atfp_img_ffo_test__gen_filt_spec_error)
{
    UTEST_IMGFILT_INIT__SETUP("invalid/path/to/nowhere")
    (void)mock_filt_src_ctx_p;
    (void)mock_filt_sink_ctx_p;
    (void)mock_filt4srcbuf;
    (void)mock_filt4dstbuf;
    expect(avfilter_inout_alloc, will_return(&mock_filt_out));
    expect(avfilter_inout_alloc, will_return(&mock_filt_in));
    expect(avfilter_graph_alloc, will_return(&mock_filt_grf));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_in)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_out)));
    atfp__image_dst__avfilt_init (&mock_avctx_src, &mock_avctx_dst, mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    UTEST_IMGFILT_INIT__TEARDOWN
} // end of  atfp_img_ffo_test__gen_filt_spec_error


Ensure(atfp_img_ffo_test__filt_ctx_spec_error)
{
    UTEST_IMGFILT_INIT__SETUP("media/data/test/image/mask")
    expect(avfilter_inout_alloc, will_return(&mock_filt_out));
    expect(avfilter_inout_alloc, will_return(&mock_filt_in));
    expect(avfilter_graph_alloc, will_return(&mock_filt_grf));
    expect(avfilter_get_by_name, will_return(&mock_filt4srcbuf), when(name, is_equal_to_string("buffer")));
    expect(avfilter_get_by_name, will_return(&mock_filt4dstbuf), when(name, is_equal_to_string("buffersink")));
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4srcbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_src_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_not_equal_to(NULL)),
       );
    expect(avfilter_graph_create_filter, will_return(AVERROR(ENOMEM)), when(filt, is_equal_to(&mock_filt4dstbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_sink_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_equal_to(NULL)),
       );
    expect(av_log);
    expect(avfilter_free, when(filt, is_equal_to(mock_filt_sink_ctx_p)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_in)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_out)));
    atfp__image_dst__avfilt_init (&mock_avctx_src, &mock_avctx_dst, mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_st_enc_ctxs[0].filter_graph, is_equal_to(&mock_filt_grf));
    assert_that(mock_st_enc_ctxs[0].filt_src_ctx, is_equal_to(&mock_filt_src_ctx));
    assert_that(mock_st_enc_ctxs[0].filt_sink_ctx, is_equal_to(NULL));
    UTEST_IMGFILT_INIT__TEARDOWN
} // end of  atfp_img_ffo_test__filt_ctx_spec_error


Ensure(atfp_img_ffo_test__filt_grf_parse_spec_error)
{
    UTEST_IMGFILT_INIT__SETUP("media/data/test/image/mask")
    expect(avfilter_inout_alloc, will_return(&mock_filt_out));
    expect(avfilter_inout_alloc, will_return(&mock_filt_in));
    expect(avfilter_graph_alloc, will_return(&mock_filt_grf));
    expect(avfilter_get_by_name, will_return(&mock_filt4srcbuf), when(name, is_equal_to_string("buffer")));
    expect(avfilter_get_by_name, will_return(&mock_filt4dstbuf), when(name, is_equal_to_string("buffersink")));
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4srcbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_src_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_not_equal_to(NULL)),
       );
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4dstbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_sink_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_equal_to(NULL)),
       );
    expect(avfilter_graph_parse_ptr, will_return(AVERROR(EBUSY)), when(graph, is_equal_to(&mock_filt_grf)),
            when(inputs, is_equal_to(&mock_filt_in)), when(outputs, is_equal_to(&mock_filt_out)),
       );
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_in)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_out)));
    atfp__image_dst__avfilt_init (&mock_avctx_src, &mock_avctx_dst, mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_st_enc_ctxs[0].filter_graph, is_equal_to(&mock_filt_grf));
    assert_that(mock_st_enc_ctxs[0].filt_src_ctx, is_equal_to(&mock_filt_src_ctx));
    assert_that(mock_st_enc_ctxs[0].filt_sink_ctx, is_equal_to(&mock_filt_sink_ctx));
    UTEST_IMGFILT_INIT__TEARDOWN
} // end of  atfp_img_ffo_test__filt_grf_parse_spec_error


Ensure(atfp_img_ffo_test__filt_grf_conn_error)
{
    UTEST_IMGFILT_INIT__SETUP("media/data/test/image/mask")
    expect(avfilter_inout_alloc, will_return(&mock_filt_out));
    expect(avfilter_inout_alloc, will_return(&mock_filt_in));
    expect(avfilter_graph_alloc, will_return(&mock_filt_grf));
    expect(avfilter_get_by_name, will_return(&mock_filt4srcbuf), when(name, is_equal_to_string("buffer")));
    expect(avfilter_get_by_name, will_return(&mock_filt4dstbuf), when(name, is_equal_to_string("buffersink")));
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4srcbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_src_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_not_equal_to(NULL)),
       );
    expect(avfilter_graph_create_filter, will_return(0), when(filt, is_equal_to(&mock_filt4dstbuf)),
            will_set_contents_of_parameter(filt_ctx_p, &mock_filt_sink_ctx_p, sizeof(AVFilterContext *)),
            when(args, is_equal_to(NULL)),
       );
    expect(avfilter_graph_parse_ptr, will_return(0), when(graph, is_equal_to(&mock_filt_grf)),
            when(inputs, is_equal_to(&mock_filt_in)), when(outputs, is_equal_to(&mock_filt_out)),
       );
    expect(avfilter_graph_config, will_return(AVERROR(EFAULT)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_in)));
    expect(avfilter_inout_free, when(inout, is_equal_to(&mock_filt_out)));
    atfp__image_dst__avfilt_init (&mock_avctx_src, &mock_avctx_dst, mock_filtspec, mock_err_info);
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    assert_that(mock_st_enc_ctxs[0].filter_graph, is_equal_to(&mock_filt_grf));
    assert_that(mock_st_enc_ctxs[0].filt_src_ctx, is_equal_to(&mock_filt_src_ctx));
    assert_that(mock_st_enc_ctxs[0].filt_sink_ctx, is_equal_to(&mock_filt_sink_ctx));
    UTEST_IMGFILT_INIT__TEARDOWN
} // end of  atfp_img_ffo_test__filt_grf_conn_error


TestSuite *app_transcoder_img_ffm_out_avfilt_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffo_test__filt_init_ok);
    add_test(suite, atfp_img_ffo_test__gen_filt_spec_error);
    add_test(suite, atfp_img_ffo_test__filt_ctx_spec_error);
    add_test(suite, atfp_img_ffo_test__filt_grf_parse_spec_error);
    add_test(suite, atfp_img_ffo_test__filt_grf_conn_error);
    return suite;
}
