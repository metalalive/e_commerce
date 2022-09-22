#ifndef MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#define MEDIA__TRANSCODER__VIDEO_FFMPEG_H
#ifdef __cplusplus
extern "C" {
#endif

#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>

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
        size_t  fetched; 
    } index_entry;
} atfp_stream_stats_t;

struct atfp_av_ctx_s {
    AVFormatContext    *fmt_ctx;
    atfp_stream_stats_t  *stats;
    union {
        AVCodecContext        **decode;
        atfp_stream_enc_ctx_t  *encode;
    } stream_ctx;
    union {
        struct {
            AVFrame   frame;
            AVPacket  packet;
            size_t    tot_num_pkts_avail; // from all valid streams
            uint16_t  num_decoded_frames;
        } decode;
        struct {
            AVFrame   frame;
            AVPacket  packet;
            uint16_t  num_filtered_frms;
            uint16_t  num_encoded_pkts;
            int8_t    stream_idx;
            struct {
                int8_t   filt_stream_idx;
                int8_t   enc_stream_idx;
                uint8_t  file_trailer_wrote:1;
                uint8_t  file_header_wrote:1;
            } _final;
        } encode;
    } intermediate_data;
    struct {
        uint8_t  num_init_pkts;
        size_t   max_nbytes_bulk; // max nbytes to load for async decoding
    } async_limit;
    uint8_t  decoder_flag:1;
}; // end of struct atfp_av_ctx_s


#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_FFMPEG_H
