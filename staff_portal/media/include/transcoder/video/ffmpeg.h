#ifndef MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#define MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#ifdef __cplusplus
extern "C" {
#endif

#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>

#include "utils.h"

typedef struct {
    AVCodecContext *enc_ctx;
    AVFilterContext *filt_sink_ctx;
    AVFilterContext *filt_src_ctx;
    AVFilterGraph   *filter_graph;
} atfp_stream_enc_ctx_t;

typedef struct {
    struct {
        size_t  preloading;
        size_t  preloaded;
        size_t  decoded; 
    } index_entry;
} atfp_stream_stats_t;

struct atfp_av_ctx_s {
    AVFormatContext    *fmt_ctx;
    union {
        AVCodecContext        **decode;
        atfp_stream_enc_ctx_t  *encode;
    } stream_ctx;
    atfp_stream_stats_t  *stats;
    uint8_t      decoder_flag:1;
};


#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_FFMPEG_H
