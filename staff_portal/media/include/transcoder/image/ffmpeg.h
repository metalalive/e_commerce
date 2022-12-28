#ifndef MEDIA__TRANSCODER__IMAGE_FFMPEG_H
#define MEDIA__TRANSCODER__IMAGE_FFMPEG_H
#ifdef __cplusplus
extern "C" {
#endif

#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>

#include "transcoder/file_processor.h"

typedef struct {
    AVCodecContext *enc_ctx;
    AVFilterContext *filt_sink_ctx;
    AVFilterContext *filt_src_ctx;
    AVFilterGraph   *filter_graph;
} atfp_stream_enc_ctx_t;

struct atfp_av_ctx_s {
    AVFormatContext    *fmt_ctx;
    union {
        AVCodecContext        **decode;
        atfp_stream_enc_ctx_t  *encode;
    } stream_ctx;
    union {
        struct {
            AVFrame   frame;
            AVPacket  packet;
            uint8_t  num_decoded_frames;
        } decode;
        struct {
            AVFrame   frame;
            AVPacket  packet;
            uint8_t   num_filtered_frms;
            uint8_t   num_encoded_pkts;
        } encode;
    } intermediate_data;
    uint8_t  decoder_flag:1;
}; // end of struct atfp_av_ctx_s


void     atfp__image_ffm_in__init_transcode(atfp_t *);
uint8_t  atfp__image_ffm_in__deinit_transcode(atfp_t *);
void     atfp__image_ffm_in__proceeding_transcode(atfp_t *);
uint8_t  atfp__image_ffm_in__has_done_processing(atfp_t *);
uint8_t  atfp__image_ffm_in__label_match (const char *label);
struct atfp_s * atfp__image_ffm_in__instantiate_transcoder(void);

void     atfp__image_ffm_out__init_transcode(atfp_t *);
uint8_t  atfp__image_ffm_out__deinit_transcode(atfp_t *);
void     atfp__image_ffm_out__proceeding_transcode(atfp_t *);
uint8_t  atfp__image_ffm_out__has_done_processing(atfp_t *);
uint8_t  atfp__image_ffm_out__label_match (const char *label);
struct atfp_s * atfp__image_ffm_out__instantiate_transcoder(void);


void atfp__image_src__avctx_init (atfp_av_ctx_t *, const char *filepath, json_t *err_info);
void atfp__image_src__avctx_deinit (atfp_av_ctx_t *);

void  atfp__image_dst__avctx_init (atfp_av_ctx_t *, atfp_av_ctx_t *, const char *filepath, json_t *err_info);
void  atfp__image_dst__avfilt_init (atfp_av_ctx_t *, json_t *filt_spec, json_t *err_info);
void  atfp__image_dst__avctx_deinit (atfp_av_ctx_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__IMAGE_FFMPEG_H
