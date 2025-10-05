#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavutil/opt.h>

#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"
#include "transcoder/common/ffmpeg.h"

typedef struct {
    AVCodecContext        *dec_ctx;
    atfp_stream_enc_ctx_t *st_enc_ctx;
    AVFilterInOut         *filt_out;
    AVFilterInOut         *filt_in;
    struct {
        char  *bytes;
        size_t sz;
    } spec;
} atfp_avfilter_data_t;

#define CREATE_AVFILTER_COMMON_CODE(args, bufsrc, bufsink, st_enc_ctx) \
    { \
        AVFilterContext *_filt_sink_ctx = NULL; \
        AVFilterContext *_filt_src_ctx = NULL; \
        err = avfilter_graph_create_filter( \
            &_filt_src_ctx, bufsrc, "in", args, NULL, st_enc_ctx->filter_graph \
        ); \
        if ((err < 0) || (!_filt_src_ctx)) { \
            av_log(NULL, AV_LOG_ERROR, "[Filter] Failed to create buffer source\n"); \
            goto done; \
        } \
        st_enc_ctx->filt_src_ctx = _filt_src_ctx; \
        err = avfilter_graph_create_filter( \
            &_filt_sink_ctx, bufsink, "out", NULL, NULL, st_enc_ctx->filter_graph \
        ); \
        if ((err < 0) || (!_filt_sink_ctx)) { \
            av_log(NULL, AV_LOG_ERROR, "[Filter] Failed to create buffer sink\n"); \
            goto done; \
        } \
        st_enc_ctx->filt_sink_ctx = _filt_sink_ctx; \
    }

static int atfp_hls__init_video_filter(atfp_avfilter_data_t *data) {
    int                    err = 0, nwrite = 0;
    atfp_stream_enc_ctx_t *st_enc_ctx = data->st_enc_ctx;
    AVCodecContext        *enc_ctx = st_enc_ctx->enc_ctx;
    AVCodecContext        *dec_ctx = data->dec_ctx;
    AVRational             frm_ratio = av_mul_q(dec_ctx->framerate, dec_ctx->time_base);
    frm_ratio = av_inv_q(frm_ratio);
    nwrite = snprintf(
        data->spec.bytes, data->spec.sz, "fps=%d,setpts=PTS*%f,scale=%d:%d",
        enc_ctx->framerate.num / enc_ctx->framerate.den,
        (1.0 * ((float)frm_ratio.num / frm_ratio.den) *
         ((float)dec_ctx->framerate.num / enc_ctx->framerate.num)),
        enc_ctx->width, enc_ctx->height
    );
    if (nwrite >= data->spec.sz) {
        av_log(NULL, AV_LOG_ERROR, "[atfp][HLS][filter] line:%d, video spec string exceeding \n", __LINE__);
        err = AVERROR(ENOMEM);
        goto done;
    }
    const AVFilter *buffersrc = avfilter_get_by_name("buffer");
    const AVFilter *buffersink = avfilter_get_by_name("buffersink");
    if (!buffersrc || !buffersink) {
        av_log(NULL, AV_LOG_ERROR, "filtering source or sink element not found\n");
        err = AVERROR_UNKNOWN;
        goto done;
    }
#define FILTER_ARG_SZ 512
    char args[FILTER_ARG_SZ] = {0};
    nwrite = snprintf(
        args, FILTER_ARG_SZ, "video_size=%dx%d:pix_fmt=%d:time_base=%d/%d:pixel_aspect=%d/%d", dec_ctx->width,
        dec_ctx->height, dec_ctx->pix_fmt, dec_ctx->time_base.num, dec_ctx->time_base.den,
        dec_ctx->sample_aspect_ratio.num, dec_ctx->sample_aspect_ratio.den
    );
    if (nwrite >= FILTER_ARG_SZ) {
        av_log(NULL, AV_LOG_ERROR, "[atfp][HLS][filter] line:%d, video spec string exceeding \n", __LINE__);
        err = AVERROR(ENOMEM);
        goto done;
    } // TODO, refactor code
    CREATE_AVFILTER_COMMON_CODE(args, buffersrc, buffersink, st_enc_ctx);
    err = av_opt_set_bin(
        st_enc_ctx->filt_sink_ctx, "pix_fmts", (uint8_t *)&enc_ctx->pix_fmt, sizeof(enc_ctx->pix_fmt),
        AV_OPT_SEARCH_CHILDREN
    );
    if (err < 0)
        av_log(NULL, AV_LOG_ERROR, "[atfp][HLS][filter] Cannot set output pixel format\n");
#undef FILTER_ARG_SZ
done:
    return err;
} // end of atfp_hls__init_video_filter

static int atfp_hls__init_audio_filter(atfp_avfilter_data_t *data) {
    int                    err = 0;
    atfp_stream_enc_ctx_t *st_enc_ctx = data->st_enc_ctx;
    AVCodecContext        *enc_ctx = st_enc_ctx->enc_ctx;
    AVCodecContext        *dec_ctx = data->dec_ctx;
    snprintf(data->spec.bytes, data->spec.sz, "aresample=%d", enc_ctx->sample_rate);
    const AVFilter *buffersrc = avfilter_get_by_name("abuffer");
    const AVFilter *buffersink = avfilter_get_by_name("abuffersink");
    if (!buffersrc || !buffersink) {
        av_log(NULL, AV_LOG_ERROR, "filtering source or sink element not found\n");
        err = AVERROR_UNKNOWN;
        goto done;
    }
    if (!dec_ctx->channel_layout)
        dec_ctx->channel_layout = av_get_default_channel_layout(dec_ctx->channels);
    char args[512] = {0};
    snprintf(
        args, sizeof(args), "time_base=%d/%d:sample_rate=%d:sample_fmt=%s:channel_layout=0x%" PRIx64,
        dec_ctx->time_base.num, dec_ctx->time_base.den, dec_ctx->sample_rate,
        av_get_sample_fmt_name(dec_ctx->sample_fmt), dec_ctx->channel_layout
    );
    CREATE_AVFILTER_COMMON_CODE(args, buffersrc, buffersink, st_enc_ctx);
    err = av_opt_set_bin(
        st_enc_ctx->filt_sink_ctx, "sample_fmts", (uint8_t *)&enc_ctx->sample_fmt,
        sizeof(enc_ctx->sample_fmt), AV_OPT_SEARCH_CHILDREN
    );
    if (err < 0) {
        av_log(NULL, AV_LOG_ERROR, "Cannot set output sample format\n");
        goto done;
    }
    err = av_opt_set_bin(
        st_enc_ctx->filt_sink_ctx, "channel_layouts", (uint8_t *)&enc_ctx->channel_layout,
        sizeof(enc_ctx->channel_layout), AV_OPT_SEARCH_CHILDREN
    );
    if (err < 0) {
        av_log(NULL, AV_LOG_ERROR, "Cannot set output channel layout\n");
        goto done;
    }
    const int out_sample_rates[] = {enc_ctx->sample_rate, -1};
    err = av_opt_set_int_list(
        st_enc_ctx->filt_sink_ctx, "sample_rates", out_sample_rates, -1, AV_OPT_SEARCH_CHILDREN
    );
    if (err < 0) {
        av_log(NULL, AV_LOG_ERROR, "Cannot set output sample rate\n");
    }
done:
    return err;
} // end of atfp_hls__init_audio_filter

int atfp_hls__avfilter_init(atfp_hls_t *hlsproc) {
    int            err = 0, idx = 0;
    atfp_av_ctx_t *avctx_dst = hlsproc->av;
    atfp_av_ctx_t *avctx_src = NULL;
    {
        asa_op_base_cfg_t *asa_dst = hlsproc->super.data.storage.handle;
        atfp_asa_map_t    *map = asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
        atfp_t            *fp_src = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        avctx_src = ((atfp_hls_t *)fp_src)->av;
    }
    AVFormatContext *ifmt_ctx = avctx_src->fmt_ctx;
    for (idx = 0; (!err) && (idx < ifmt_ctx->nb_streams); idx++) {
        enum AVMediaType       codectype = ifmt_ctx->streams[idx]->codecpar->codec_type;
        AVCodecContext        *dec_ctx = avctx_src->stream_ctx.decode[idx];
        atfp_stream_enc_ctx_t *st_enc_ctx = &avctx_dst->stream_ctx.encode[idx];
        if (!dec_ctx || !st_enc_ctx || !st_enc_ctx->enc_ctx) { // ignore then log warning/info
            av_log(NULL, AV_LOG_INFO, "no decode/encode context provided, the stream type: %d \n", codectype);
            continue;
        }
        AVFilterInOut *outputs = avfilter_inout_alloc();
        AVFilterInOut *inputs = avfilter_inout_alloc();
        st_enc_ctx->filter_graph = avfilter_graph_alloc();
        if (!outputs || !inputs || !st_enc_ctx->filter_graph) {
            err = AVERROR(ENOMEM);
            goto end;
        }
#define NBYTES_FILTER_SPEC_RAW 128
        char                 filter_spec_raw[NBYTES_FILTER_SPEC_RAW] = {0};
        atfp_avfilter_data_t data = {
            .dec_ctx = dec_ctx,
            .st_enc_ctx = st_enc_ctx,
            .filt_in = inputs,
            .filt_out = outputs,
            .spec = {.bytes = &filter_spec_raw[0], .sz = NBYTES_FILTER_SPEC_RAW}
        };
        switch (codectype) {
        case AVMEDIA_TYPE_VIDEO:
            err = atfp_hls__init_video_filter(&data);
            break;
        case AVMEDIA_TYPE_AUDIO:
            err = atfp_hls__init_audio_filter(&data);
            break;
        default: // skip
            goto end;
            break;
        }
        if (err)
            goto end;
        // Endpoints for the filter graph.
        outputs->name = av_strdup("in");
        outputs->filter_ctx = st_enc_ctx->filt_src_ctx;
        outputs->pad_idx = 0;
        outputs->next = NULL;

        inputs->name = av_strdup("out");
        inputs->filter_ctx = st_enc_ctx->filt_sink_ctx;
        inputs->pad_idx = 0;
        inputs->next = NULL;

        err =
            avfilter_graph_parse_ptr(st_enc_ctx->filter_graph, &filter_spec_raw[0], &inputs, &outputs, NULL);
        if (err < 0) {
            goto end;
        }
        err = avfilter_graph_config(st_enc_ctx->filter_graph, NULL);
#undef NBYTES_FILTER_SPEC_RAW
    end:
        avfilter_inout_free(&inputs);
        avfilter_inout_free(&outputs);
    } // end of loop
    return err;
} // end of atfp_hls__avfilter_init

int atfp_hls__av_filter_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst) {
    int ret = ATFP_AVCTX_RET__OK;
    if (!src || !dst) {
        ret = AVERROR(EINVAL);
    } else {
        int    nb_streams_in = src->fmt_ctx->nb_streams;
        int8_t _stream_idx = (int8_t)src->intermediate_data.decode.packet.stream_index;
        if (_stream_idx < 0 || _stream_idx >= nb_streams_in) {
            ret = AVERROR(EINVAL);
        } else {
            ret = atfp_common__ffm_filter_processing(dst, &src->intermediate_data.decode.frame, _stream_idx);
            if (ret == ATFP_AVCTX_RET__OK)
                dst->intermediate_data.encode.stream_idx = _stream_idx;
        }
    }
    return ret;
} // end of atfp_hls__av_filter_processing

int atfp_hls__av_filter__finalize_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst) {
    int    ret = ATFP_AVCTX_RET__OK;
    int8_t nb_streams_in = (int8_t)src->fmt_ctx->nb_streams;
    int8_t _stream_idx = (int8_t)dst->intermediate_data.encode._final.filt_stream_idx;
    if (nb_streams_in > _stream_idx) {
        ret = atfp_common__ffm_filter_processing(dst, NULL, _stream_idx);
        if (ret == ATFP_AVCTX_RET__OK)
            dst->intermediate_data.encode.stream_idx = _stream_idx;
        else if (ret == ATFP_AVCTX_RET__NEED_MORE_DATA)
            dst->intermediate_data.encode._final.filt_stream_idx = _stream_idx + 1;
    } else {
        ret = ATFP_AVCTX_RET__OK; // all frames were already flushed, skip
    }
    return ret;
} // end of atfp_hls__av_filter__finalize_processing

uint8_t atfp_av_filter__has_done_flushing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst) {
    int8_t nb_streams_in = (int8_t)src->fmt_ctx->nb_streams;
    int8_t _stream_idx = (int8_t)dst->intermediate_data.encode._final.filt_stream_idx;
    return (nb_streams_in > 0) && (nb_streams_in == _stream_idx);
}
