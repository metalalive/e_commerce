#include <unistd.h>
#include <string.h>
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

#define  PLAYLIST_FILENAME      "playlist"

void  atfp_hls__av_deinit(atfp_hls_t *hlsproc)
{
    AVFormatContext *fmt_ctx  = hlsproc->av->fmt_ctx;
    atfp_stream_enc_ctx_t *enc_ctxs = hlsproc->av ->stream_ctx.encode;
    if(enc_ctxs) {
        int nb_streams = fmt_ctx ? fmt_ctx->nb_streams: 0;
        for(int idx = 0; idx < nb_streams; idx++) {
            if(enc_ctxs[idx].enc_ctx)
                avcodec_free_context(&enc_ctxs[idx].enc_ctx);
            if(enc_ctxs[idx].filter_graph) {}
        }
        av_freep(&hlsproc->av ->stream_ctx.encode);
    }
    if(fmt_ctx) {
        avformat_free_context(fmt_ctx);
        hlsproc->av->fmt_ctx = NULL;
    }
} // end of atfp_hls__av_deinit


static void _atfp_config_dst_video_codecctx(AVCodecContext *dst, AVCodecContext *src, json_t *spec)
{
    json_t *elm_st_key_obj = json_object_get(json_object_get(spec, "__internal__"), "video_key");
    const char *elm_st_key = json_string_value(elm_st_key_obj);
    json_t *attribute  = json_object_get(json_object_get(json_object_get(spec, "elementary_streams"
                    ), elm_st_key), "attribute");
    uint32_t height = json_integer_value(json_object_get(attribute, "height_pixel"));
    uint32_t width  = json_integer_value(json_object_get(attribute, "width_pixel"));
    uint8_t  fps    = json_integer_value(json_object_get(attribute, "framerate"));
    dst->height = FFMIN(src->height, height);
    dst->width  = FFMIN(src->width , width);
    dst->framerate = (AVRational){num:fps, den:1};
    if (dst->codec->pix_fmts) // take first format from list of supported formats
        dst->pix_fmt = dst->codec->pix_fmts[0];
    else
        dst->pix_fmt = src->pix_fmt;
    dst->sample_aspect_ratio = src->sample_aspect_ratio;
    dst->time_base = src->time_base;
    dst->bit_rate  = src->bit_rate;    
} // end of _atfp_config_dst_video_codecctx

static void _atfp_config_dst_audio_coderctx(AVCodecContext *dst, AVCodecContext *src, json_t *spec)
{
    json_t *elm_st_key_obj = json_object_get(json_object_get(spec, "__internal__"), "audio_key");
    const char *elm_st_key = json_string_value(elm_st_key_obj);
    json_t *attribute  = json_object_get(json_object_get(json_object_get(spec, "elementary_streams"
                    ), elm_st_key), "attribute");
    uint32_t bitrate_kbps = json_integer_value(json_object_get(attribute, "bitrate_kbps"));
    dst->bit_rate  = FFMIN(src->bit_rate, (bitrate_kbps * 1000 - 1));
    dst->sample_rate = src->sample_rate;
    dst->channel_layout = src->channel_layout;
    dst->channels = av_get_channel_layout_nb_channels(dst->channel_layout);
    // take first format from list of supported formats
    dst->sample_fmt = src->codec->sample_fmts[0];
    dst->time_base  = (AVRational){1, dst->sample_rate};
} // end of _atfp_config_dst_audio_coderctx


static  __attribute__((optimize("O0"))) int _atfp_hls__av_encoder_init(atfp_hls_t *hlsproc)
{
    atfp_t   *fp_dst = &hlsproc->super;
    json_t *err_info = fp_dst ->data.error;
    asa_op_base_cfg_t  *asa_dst = fp_dst->data.storage.handle;
    atfp_asa_map_t     *map = asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(map);
    // TODO, refactor in case app uses different multimedia codec library
    atfp_t   *fp_src =  asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if(fp_dst->backend_id != fp_src->backend_id) {
        json_object_set_new(err_info, "transcoder", json_string("[hls] invalid backend library in source file processor"));
        return AVERROR(EINVAL);
    }
    // from this point, it ensures both src/dst sides apply the same backend
    // library for transcoding process.
    atfp_av_ctx_t   *avctx_src = ((atfp_hls_t *)fp_src)->av; // TODO, better design for identifying source format
    atfp_av_ctx_t   *avctx_dst = hlsproc->av;
    AVFormatContext *ofmt_ctx  = avctx_dst->fmt_ctx;
    AVFormatContext *ifmt_ctx  = avctx_src->fmt_ctx;

    AVCodecContext       **dec_ctxs = avctx_src ->stream_ctx.decode;
    atfp_stream_enc_ctx_t *enc_ctxs = av_mallocz_array(ifmt_ctx->nb_streams, sizeof(atfp_stream_enc_ctx_t));
    avctx_dst ->stream_ctx.encode = enc_ctxs;
    int err = 0, idx = 0;
    for(idx = 0; idx < ifmt_ctx->nb_streams; idx++) {
        AVStream  *stream_out = avformat_new_stream(ofmt_ctx, NULL);
        if(!stream_out) {
            json_object_set_new(err_info, "transcoder", json_string("[hls] failed to create stream"));
            err = AVERROR(ENOMEM);
            break;
        }
        AVCodecContext *dec_ctx = dec_ctxs[idx];
        if(dec_ctx->codec_type == AVMEDIA_TYPE_VIDEO || dec_ctx->codec_type == AVMEDIA_TYPE_AUDIO) {
            const AVCodec *encoder = dec_ctx->codec;
            AVCodecContext *enc_ctx = avcodec_alloc_context3(encoder);
            enc_ctxs[idx].enc_ctx = enc_ctx;
            if(!enc_ctx) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] failed to create encoder context of the stream"));
                err = AVERROR(ENOMEM);
                break;
            }
            if(dec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
                _atfp_config_dst_video_codecctx(enc_ctx, dec_ctx, fp_dst->data.spec);
            } else if(dec_ctx->codec_type == AVMEDIA_TYPE_AUDIO) {
                _atfp_config_dst_audio_coderctx(enc_ctx, dec_ctx, fp_dst->data.spec);
            }
            err = avcodec_open2(enc_ctx, encoder, NULL);
            if(err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] failed to open encoder context of the stream"));
                break;
            }
            err = avcodec_parameters_from_context(stream_out->codecpar, enc_ctx);
            if (err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] Failed to copy encoder parameters to output stream"));
                break;
            }
            stream_out->time_base = enc_ctx->time_base;            
        } else { // for other valid stream types
            AVStream  *stream_in  = ifmt_ctx->streams[idx];
            err = avcodec_parameters_copy(stream_out->codecpar, stream_in->codecpar);
            if (err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[mp4] Failed to copy parameters from input stream"));
                break;
            }
            stream_out->time_base = stream_in->time_base;            
        }
    } // end of stream iteration
    return err;
} // end of _atfp_hls__av_encoder_init


int  atfp_hls__av_init(atfp_hls_t *hlsproc)
{
    int err = 0;
    AVFormatContext *fmt_ctx = NULL;
    atfp_t  *processor = &hlsproc->super;
    json_t  *req_spec  =  processor->data.spec;
    { // In this project, everything in `spec` field has to be validated at app server
        const char *fmt_name = json_string_value(json_object_get(req_spec, "container"));
        const char *basepath = hlsproc->asa_local.super.op.mkdir.path.origin;
        size_t playlist_path_sz = strlen(basepath) + 1 + sizeof(PLAYLIST_FILENAME);
        char   playlist_path [playlist_path_sz];
        size_t nwrite = snprintf(&playlist_path[0], playlist_path_sz, "%s/%s", basepath, PLAYLIST_FILENAME);
        playlist_path[nwrite++] = 0;
        err = avformat_alloc_output_context2(&fmt_ctx, NULL, fmt_name, &playlist_path[0]);
        hlsproc->av->fmt_ctx = fmt_ctx;
    } // does output format context require low-level AVIO context ?
    if (!err)
        err  = _atfp_hls__av_encoder_init(hlsproc);
    // TODO, add function and data structure to monitor how many segment files are done by encoding
    //  function and ready to traansfer to destination storage. This is for certain types of output
    //  formats which support content segmentation such as HLS or mpeg-DASH
    if(err) {
        json_object_set_new(processor->data.error, "transcoder",
            json_string("[mp4] failed to initialize output format context"));
        atfp_hls__av_deinit(hlsproc);
    }
    return err;
} // end of atfp_hls__av_init
#undef  PLAYLIST_FILENAME
