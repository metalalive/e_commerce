#ifndef MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#define MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#ifdef __cplusplus
extern "C" {
#endif

#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>

struct atfp_mp4_stream_ctx_s {
    AVCodecContext *dec_ctx;
    AVCodecContext *enc_ctx;
    AVFilterContext *filt_sink_ctx;
    AVFilterContext *filt_src_ctx;
    AVFilterGraph   *filter_graph;
};

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_FFMPEG_H
