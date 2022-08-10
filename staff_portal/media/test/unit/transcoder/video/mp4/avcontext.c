#include <unistd.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <uv.h>

#include "app_cfg.h"
#include "transcoder/datatypes.h"
#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

#define  LOCAL_TMPBUF_BASEPATH  "tmp/buffer/media/test"
#define  UNITTEST_FOLDER_NAME   "utest"
#define  LOCAL_TMPBUF_NAME      "local_tmpbuf"

#define  UNITTEST_FULLPATH   LOCAL_TMPBUF_BASEPATH "/"  UNITTEST_FOLDER_NAME
#define  LOCAL_TMPBUF_PATH   UNITTEST_FULLPATH   "/"  LOCAL_TMPBUF_NAME

#define  NUM_FILE_CHUNKS     1
#define  NUM_STREAMS_FMTCTX  2
#define  NUM_PKTS_VIDEO_STREAM   13
#define  NUM_PKTS_AUDIO_STREAM   11
#define  MDAT_ATOM_OFFSET    32
#define  MDAT_ATOM_BODY_SZ   (193 + 10 - MDAT_ATOM_OFFSET)
#define  PACKET_INDEX_ENTRY_VIDEO  { \
    {.pos=MDAT_ATOM_OFFSET, .size=9}, \
    {.pos=47, .size=10}, \
    {.pos=60, .size=7}, \
    {.pos=67, .size=6}, \
    {.pos=85, .size=11}, \
    {.pos=96, .size=3}, \
    {.pos=116, .size=5}, \
    {.pos=121, .size=4}, \
    {.pos=125, .size=7}, \
    {.pos=142, .size=3}, \
    {.pos=158, .size=9}, \
    {.pos=189, .size=4}, \
    {.pos=193, .size=10}, \
}
#define  PACKET_INDEX_ENTRY_AUDIO  { \
    {.pos=41,  .size=6}, \
    {.pos=57,  .size=3}, \
    {.pos=73,  .size=8}, \
    {.pos=81,  .size=4}, \
    {.pos=99,  .size=9}, \
    {.pos=108, .size=8}, \
    {.pos=132, .size=10}, \
    {.pos=145, .size=6}, \
    {.pos=151, .size=7}, \
    {.pos=167, .size=13}, \
    {.pos=180, .size=9}, \
}

#define  PRELOAD_INIT_PKTSEQ_SZ  (81 + 4 - MDAT_ATOM_OFFSET)
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)


static void utest_atfp_mp4__avctx_init__done_cb(atfp_mp4_t *mp4proc)
{
    mock(mp4proc);
}

static ASA_RES_CODE mock_asa_src_read_fn(asa_op_base_cfg_t *cfg)
{ // skip real storage read function, directly invoke the callback, assume the preloading is done.
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)cfg->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    assert_that(mp4proc->internal.preload_pkts.size, is_equal_to(PRELOAD_INIT_PKTSEQ_SZ));
    assert_that(mp4proc->internal.preload_pkts.nbytes_copied, is_equal_to(0));
    mp4proc->internal.preload_pkts.nbytes_copied = mp4proc->internal.preload_pkts.size;
    mp4proc->internal.callback.preload_done(mp4proc);
    return ASTORAGE_RESULT_ACCEPT;
}

// assume mdat starts at the first chunk, and currently mp4 processor object positions the same chunk
// assume there is only one chunk in the source
#define  UNITTEST_AVCTX_INIT__SETUP \
    uint8_t   mock_avio_ctx_buffer[20] = {0}; \
    AVCodecParameters  mock_codec_param = {0}; \
    AVIndexEntry  mock_idx_entry_video[NUM_PKTS_VIDEO_STREAM] = PACKET_INDEX_ENTRY_VIDEO; \
    AVIndexEntry  mock_idx_entry_audio[NUM_PKTS_AUDIO_STREAM] = PACKET_INDEX_ENTRY_AUDIO; \
    AVStream  mock_av_streams[NUM_STREAMS_FMTCTX] = { \
        {.nb_index_entries=NUM_PKTS_VIDEO_STREAM, .index_entries=&mock_idx_entry_video[0], \
            .index=0, .codecpar=&mock_codec_param}, \
        {.nb_index_entries=NUM_PKTS_AUDIO_STREAM, .index_entries=&mock_idx_entry_audio[0], \
            .index=1, .codecpar=&mock_codec_param}, \
    }; \
    AVCodec  mock_av_decoders[NUM_STREAMS_FMTCTX] = {0}; \
    AVCodecContext  mock_av_codec_ctx[NUM_STREAMS_FMTCTX] = { \
        {.codec_type=AVMEDIA_TYPE_AUDIO}, {.codec_type=AVMEDIA_TYPE_VIDEO} \
    }; \
    AVStream  *mock_av_streams_ptr[NUM_STREAMS_FMTCTX] = {&mock_av_streams[0], &mock_av_streams[1]}; \
    AVCodecContext  *mock_dec_ctxs[NUM_STREAMS_FMTCTX] = {0}; \
    AVIOContext      mock_avio_ctx  = {0}; \
    AVFormatContext  mock_avfmt_ctx = {.nb_streams=NUM_STREAMS_FMTCTX, .streams=mock_av_streams_ptr}; \
    asa_op_base_cfg_t  asa_cfg_src = {0}; \
    asa_op_localfs_cfg_t  asa_local = {0}; \
    atfp_asa_map_t  *mock_map = atfp_asa_map_init(2); \
    asa_cfg_t   storage_src = {.ops={.fn_read=mock_asa_src_read_fn}}; \
    atfp_av_ctx_t  *mock_av_ctx = calloc(1, sizeof(atfp_av_ctx_t)); \
    atfp_mp4_t  mp4proc = { \
        .internal={.mdat={.pos=MDAT_ATOM_OFFSET, .fchunk_seq=0, .size=MDAT_ATOM_BODY_SZ}} , \
        .super={.data={.storage={.handle=&asa_cfg_src, .config=&storage_src}, \
            .spec=json_object(), .error=json_object() }}, .av=mock_av_ctx \
    }; \
    void *asacfg_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    { \
        json_t *fchunks_sz = json_array(); \
        json_object_set_new(mp4proc.super.data.spec, "parts_size", fchunks_sz); \
        json_array_append_new(fchunks_sz, json_integer(MDAT_ATOM_OFFSET + MDAT_ATOM_BODY_SZ)); \
        asacfg_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mp4proc; \
        asa_cfg_src.cb_args.entries = asacfg_cb_args; \
        asa_cfg_src.cb_args.size    = NUM_CB_ARGS_ASAOBJ; \
        atfp_asa_map_set_source(mock_map, &asa_cfg_src); \
        atfp_asa_map_set_localtmp(mock_map, &asa_local); \
        mkdir(LOCAL_TMPBUF_PATH, S_IRWXU); \
        asa_local .file.file = open(LOCAL_TMPBUF_PATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    }


#define  UNITTEST_AVCTX_INIT__TEARDOWN \
    { \
        close(asa_local.file.file); \
        unlink(LOCAL_TMPBUF_PATH); \
        rmdir(UNITTEST_FULLPATH); \
        json_decref(mp4proc.super.data.spec); \
        json_decref(mp4proc.super.data.error); \
        free(mock_av_ctx); \
        atfp_asa_map_deinit(mock_map); \
    }


Ensure(atfp_mp4_test__avctx_init_ok) {
    UNITTEST_AVCTX_INIT__SETUP;
    {
        expect(avformat_alloc_context,  will_return(&mock_avfmt_ctx));
        expect(av_malloc,  will_return(mock_avio_ctx_buffer));
        expect(avio_alloc_context,  will_return(&mock_avio_ctx), when(buffer, is_equal_to(mock_avio_ctx_buffer)));
        expect(avformat_open_input, will_return(0), when(_fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
        expect(avformat_find_stream_info,  will_return(0), when(ic, is_equal_to(&mock_avfmt_ctx)));
        expect(av_mallocz_array,  will_return(mock_dec_ctxs),
                when(nmemb, is_equal_to(NUM_STREAMS_FMTCTX)),
                when(sz, is_equal_to(sizeof(AVCodecContext *))) );
        for(int idx = 0; idx < NUM_STREAMS_FMTCTX; idx++) {
            AVCodecContext *expect_codec_ctx = &mock_av_codec_ctx[idx];
            expect(avcodec_find_decoder,    will_return(&mock_av_decoders[idx]));
            expect(avcodec_alloc_context3,  will_return(expect_codec_ctx));
            expect(avcodec_parameters_to_context,  will_return(0),
                    when(codec_ctx, is_equal_to(expect_codec_ctx)) ,
                    when(par, is_equal_to(mock_av_streams[idx].codecpar))  );
            expect(avcodec_open2,  will_return(0), when(ctx, is_equal_to(expect_codec_ctx)) );
            if(expect_codec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
                expect(av_guess_frame_rate,  will_return(18));
                expect(av_guess_frame_rate,  will_return(1));
            }
        }
        expect(utest_atfp_mp4__avctx_init__done_cb);
    } {
        ASA_RES_CODE result = atfp_mp4__av_init(&mp4proc, utest_atfp_mp4__avctx_init__done_cb);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        assert_that(mp4proc.av->fmt_ctx, is_equal_to(&mock_avfmt_ctx));
        assert_that(mp4proc.av->stream_ctx.decode, is_equal_to(mock_dec_ctxs));
        for(int idx = 0; idx < NUM_STREAMS_FMTCTX; idx++) {
            assert_that(mock_dec_ctxs[idx], is_equal_to(&mock_av_codec_ctx[idx]));
        }
        assert_that(json_object_size(mp4proc.super.data.error), is_equal_to(0));
    }
    UNITTEST_AVCTX_INIT__TEARDOWN;
} // end of atfp_mp4_test__avctx_init_ok


Ensure(atfp_mp4_test__avctx_init__fmtctx_error) {
#pragma GCC diagnostic ignored "-Wunused-variable"
    UNITTEST_AVCTX_INIT__SETUP;
    {
        expect(avformat_alloc_context,  will_return(&mock_avfmt_ctx));
        expect(av_malloc,  will_return(mock_avio_ctx_buffer));
        expect(avio_alloc_context,  will_return(&mock_avio_ctx), when(buffer, is_equal_to(mock_avio_ctx_buffer)));
        expect(avformat_open_input, will_return(AVERROR(ENOMEM)), when(_fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
        expect(av_freep, when(ptr, is_equal_to(&mock_avio_ctx.buffer))); // the address to &mock_avio_ctx_buffer[0]
        expect(avio_context_free, when(s, is_equal_to(&mock_avfmt_ctx.pb)));
        expect(avformat_close_input);
    } {
        ASA_RES_CODE result = atfp_mp4__av_init(&mp4proc, utest_atfp_mp4__avctx_init__done_cb);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_OS_ERROR));
        assert_that(mp4proc.av->fmt_ctx, is_equal_to(NULL));
        assert_that(mp4proc.av->stream_ctx.decode, is_equal_to(NULL));
        assert_that(json_object_size(mp4proc.super.data.error), is_equal_to(1));
    }
    UNITTEST_AVCTX_INIT__TEARDOWN;
#pragma GCC diagnostic pop
} // end of atfp_mp4_test__avctx_init__fmtctx_error


Ensure(atfp_mp4_test__avctx_init__codec_error) {
    UNITTEST_AVCTX_INIT__SETUP;
    {
        expect(avformat_alloc_context,  will_return(&mock_avfmt_ctx));
        expect(av_malloc,  will_return(mock_avio_ctx_buffer));
        expect(avio_alloc_context,  will_return(&mock_avio_ctx), when(buffer, is_equal_to(mock_avio_ctx_buffer)));
        expect(avformat_open_input, will_return(0), when(_fmt_ctx, is_equal_to(&mock_avfmt_ctx)));
        expect(avformat_find_stream_info,  will_return(0), when(ic, is_equal_to(&mock_avfmt_ctx)));
        expect(av_mallocz_array,  will_return(mock_dec_ctxs),
                when(nmemb, is_equal_to(NUM_STREAMS_FMTCTX)),
                when(sz, is_equal_to(sizeof(AVCodecContext *))) );
        {
            AVCodecContext *expect_codec_ctx = &mock_av_codec_ctx[0];
            expect(avcodec_find_decoder,    will_return(&mock_av_decoders[0]));
            expect(avcodec_alloc_context3,  will_return(expect_codec_ctx));
            expect(avcodec_parameters_to_context,  will_return(0),
                    when(codec_ctx, is_equal_to(expect_codec_ctx)) ,
                    when(par, is_equal_to(mock_av_streams[0].codecpar))  );
            expect(avcodec_open2,  will_return(AVERROR(ENOMEM)), when(ctx, is_equal_to(expect_codec_ctx)) );
        }
        expect(utest_atfp_mp4__avctx_init__done_cb);
    } {
        ASA_RES_CODE result = atfp_mp4__av_init(&mp4proc, utest_atfp_mp4__avctx_init__done_cb);
        assert_that(result, is_equal_to(ASTORAGE_RESULT_ACCEPT));
        assert_that(mp4proc.av->fmt_ctx, is_equal_to(&mock_avfmt_ctx));
        assert_that(mp4proc.av->stream_ctx.decode, is_equal_to(mock_dec_ctxs));
        assert_that(mock_dec_ctxs[0], is_equal_to(&mock_av_codec_ctx[0]));
        assert_that(mock_dec_ctxs[1], is_equal_to(NULL));
        assert_that(json_object_size(mp4proc.super.data.error), is_equal_to(1));
    }
    UNITTEST_AVCTX_INIT__TEARDOWN;
} // end of atfp_mp4_test__avctx_init__codec_error


#define  NUM_AV_INPUT_FORMATS  3
#define  NUM_AUDIO_CODEC_CTXS  4
#define  NUM_VIDEO_CODEC_CTXS  5

#define  UNITTEST_AVCTX_VALIDATE_SOURCE__SETUP \
    app_cfg_t *app_cfg = app_get_global_cfg(); \
    aav_cfg_input_t *aav_cfg_in = &app_cfg->transcoder.input; \
    AVInputFormat    mock_avinfmts[NUM_AV_INPUT_FORMATS] = {0}; \
    AVInputFormat   *mock_avinfmt_ptrs[NUM_AV_INPUT_FORMATS] = {0}; \
    struct AVCodec   mock_codecs[NUM_AUDIO_CODEC_CTXS + NUM_VIDEO_CODEC_CTXS] = {0}; \
    struct AVCodec   *mock_audio_codec_ptrs[NUM_AUDIO_CODEC_CTXS] = {0}; \
    struct AVCodec   *mock_video_codec_ptrs[NUM_VIDEO_CODEC_CTXS] = {0}; \
    AVCodecContext    mock_audio_codec_ctxs[NUM_AUDIO_CODEC_CTXS] = {0}; \
    AVCodecContext    mock_video_codec_ctxs[NUM_VIDEO_CODEC_CTXS] = {0}; \
    AVFormatContext  mock_avfmt_ctx = {.nb_streams=NUM_STREAMS_FMTCTX}; \
    AVCodecContext  *mock_dec_ctxs[NUM_STREAMS_FMTCTX] = {0}; \
    atfp_av_ctx_t    mock_av_ctx = {.fmt_ctx=&mock_avfmt_ctx, .stream_ctx={.decode=&mock_dec_ctxs[0]}}; \
    atfp_mp4_t  mp4proc = { \
        .super={.data={.error=json_object(), .spec=json_object()}}, \
        .av=&mock_av_ctx }; \
    { \
        int idx = 0; \
        for(idx = 0; idx < NUM_AV_INPUT_FORMATS; idx++) \
            mock_avinfmt_ptrs[idx] = &mock_avinfmts[idx]; \
        for(idx = 0; idx < NUM_AUDIO_CODEC_CTXS; idx++) { \
            mock_audio_codec_ptrs[idx]       = &mock_codecs[idx]; \
            mock_audio_codec_ctxs[idx].codec = &mock_codecs[idx]; \
            mock_codecs[idx].type = AVMEDIA_TYPE_AUDIO; \
        } \
        for(idx = 0; idx < NUM_VIDEO_CODEC_CTXS; idx++) { \
            int jdx = idx + NUM_AUDIO_CODEC_CTXS; \
            mock_video_codec_ptrs[idx]       = &mock_codecs[jdx]; \
            mock_video_codec_ctxs[idx].codec = &mock_codecs[jdx]; \
            mock_codecs[jdx].type = AVMEDIA_TYPE_VIDEO; \
        } \
        aav_cfg_in->demuxers .entries = (void **)mock_avinfmt_ptrs; \
        aav_cfg_in->demuxers .size    = NUM_AV_INPUT_FORMATS; \
        aav_cfg_in->decoder.audio.entries = (void **)mock_audio_codec_ptrs; \
        aav_cfg_in->decoder.audio.size    = NUM_AUDIO_CODEC_CTXS; \
        aav_cfg_in->decoder.video.entries = (void **)mock_video_codec_ptrs; \
        aav_cfg_in->decoder.video.size    = NUM_VIDEO_CODEC_CTXS; \
    }

#define  UNITTEST_AVCTX_VALIDATE_SOURCE__TEARDOWN \
    memset(aav_cfg_in, 0x0, sizeof(aav_cfg_input_t)); \
    json_decref(mp4proc.super.data.spec); \
    json_decref(mp4proc.super.data.error);

Ensure(atfp_mp4_test__avctx_validate__ok) {
    UNITTEST_AVCTX_VALIDATE_SOURCE__SETUP;
    mock_avfmt_ctx.iformat = &mock_avinfmts[2];
    mock_dec_ctxs[0] = &mock_audio_codec_ctxs[0];
    mock_dec_ctxs[1] = &mock_video_codec_ctxs[3];
    int err = atfp_mp4__validate_source_format(&mp4proc);
    assert_that(err, is_equal_to(0));
    assert_that(json_object_size(mp4proc.super.data.error), is_equal_to(0));
    UNITTEST_AVCTX_VALIDATE_SOURCE__TEARDOWN;
} // end of atfp_mp4_test__avctx_validate__ok


Ensure(atfp_mp4_test__avctx_validate__demuxer_unsupported) {
    UNITTEST_AVCTX_VALIDATE_SOURCE__SETUP;
    AVInputFormat    mock_excluded_avinfmt = {0};
    mock_avfmt_ctx.iformat = &mock_excluded_avinfmt;
    mock_dec_ctxs[0] = &mock_audio_codec_ctxs[3];
    mock_dec_ctxs[1] = &mock_video_codec_ctxs[1];
    int err = atfp_mp4__validate_source_format(&mp4proc);
    assert_that(err, is_greater_than(0));
    assert_that(json_object_size(mp4proc.super.data.error), is_greater_than(0));
    const char *expect_errmsg = "[mp4] unsupported demuxer";
    const char *actual_errmsg = json_string_value(json_object_get(mp4proc.super.data.error, "transcoder"));
    assert_that(actual_errmsg, is_equal_to_string(expect_errmsg));
    UNITTEST_AVCTX_VALIDATE_SOURCE__TEARDOWN;
}  // end of atfp_mp4_test__avctx_validate__demuxer_unsupported


Ensure(atfp_mp4_test__avctx_validate__decoder_unsupported) {
    UNITTEST_AVCTX_VALIDATE_SOURCE__SETUP;
    struct AVCodec   mock_excluded_codec = {0};
    mock_video_codec_ctxs[1].codec = &mock_excluded_codec;
    mock_avfmt_ctx.iformat = &mock_avinfmts[1];
    mock_dec_ctxs[0] = &mock_audio_codec_ctxs[2];
    mock_dec_ctxs[1] = &mock_video_codec_ctxs[1];
    int err = atfp_mp4__validate_source_format(&mp4proc);
    assert_that(err, is_greater_than(0));
    assert_that(json_object_size(mp4proc.super.data.error), is_greater_than(0));
    const char *expect_errmsg = "[mp4] unsupported video codec";
    const char *actual_errmsg = json_string_value(json_object_get(mp4proc.super.data.error, "transcoder"));
    assert_that(actual_errmsg, is_equal_to_string(expect_errmsg));
    UNITTEST_AVCTX_VALIDATE_SOURCE__TEARDOWN;
}  // end of atfp_mp4_test__avctx_validate__decoder_unsupported

#undef   NUM_AV_INPUT_FORMATS
#undef   NUM_AUDIO_CODEC_CTXS
#undef   NUM_VIDEO_CODEC_CTXS


TestSuite *app_transcoder_mp4_avcontext_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_mp4_test__avctx_init_ok);
    add_test(suite, atfp_mp4_test__avctx_init__fmtctx_error);
    add_test(suite, atfp_mp4_test__avctx_init__codec_error);
    add_test(suite, atfp_mp4_test__avctx_validate__ok);
    add_test(suite, atfp_mp4_test__avctx_validate__demuxer_unsupported);
    add_test(suite, atfp_mp4_test__avctx_validate__decoder_unsupported);
    return suite;
}

#undef  PRELOAD_INIT_PKTSEQ_SZ
#undef  NUM_STREAMS_FMTCTX
#undef  NUM_FILE_CHUNKS
#undef  NUM_PKTS_VIDEO_STREAM
#undef  NUM_PKTS_AUDIO_STREAM
#undef  PACKET_INDEX_ENTRY_VIDEO
#undef  PACKET_INDEX_ENTRY_AUDIO