#include "transcoder/image/ffmpeg.h"

static void  _atfp_img_src__avctx_init_decoder(atfp_av_ctx_t *_avctx, json_t *err_info)
{
    AVFormatContext  *_fmt_ctx = _avctx ->fmt_ctx;
    if(_fmt_ctx->nb_streams != 1) {
        json_object_set_new(err_info, "transcoder", json_string("[ff_in] invalid image type"));
        return;
    }
    int ret = avformat_find_stream_info(_fmt_ctx, NULL);
    if(ret < 0) {
        json_object_set_new(err_info, "transcoder", json_string("failed to analyze stream info"));
        json_object_set_new(err_info, "err_code", json_integer(ret));
    } else {
        AVCodecContext **dec_ctxs = av_mallocz_array(_fmt_ctx->nb_streams, sizeof(AVCodecContext *));
        _avctx->stream_ctx.decode = dec_ctxs;
        AVStream *stream  = _fmt_ctx->streams[0];
        AVCodec  *decoder = avcodec_find_decoder(stream->codecpar->codec_id);
        if(!decoder) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to find decoder for the stream"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
            return;
        }
        AVCodecContext *codec_ctx = avcodec_alloc_context3(decoder);
        dec_ctxs[0] = codec_ctx;
        if(!codec_ctx) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to create decoder context of the stream"));
            return;
        }
        ret = avcodec_parameters_to_context(codec_ctx, stream->codecpar);
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to copy parameters from stream to decoder context"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
            return;
        }
        if(codec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
            ret = avcodec_open2(codec_ctx, decoder, NULL);
        } else {
            ret = AVERROR_INVALIDDATA; // unsupported stream type
        }
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to open decoder for stream"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
        }
    }
} // end of  _atfp_img_src__avctx_init_decoder


void atfp__image_src__avctx_init (atfp_av_ctx_t *_avctx, const char *filepath, json_t *err_info)
{
    AVFormatContext *_fmt_ctx = NULL;
    int ret = avformat_open_input(&_fmt_ctx, filepath, NULL, NULL);
    *_avctx = (atfp_av_ctx_t){.fmt_ctx=_fmt_ctx, .decoder_flag=1};
    if (ret == 0) {
        _atfp_img_src__avctx_init_decoder (_avctx, err_info);
    } else {
        json_object_set_new(err_info, "transcoder",
                json_string("[ff_in] failed to initialize input format context"));
        json_object_set_new(err_info, "err_code", json_integer(ret));
    }
} // end of  atfp__image_src__avctx_init


void atfp__image_src__avctx_deinit (atfp_av_ctx_t *_avctx)
{
    AVCodecContext **dec_ctxs = _avctx->stream_ctx.decode;
    if(dec_ctxs) {
        if(dec_ctxs[0])
            avcodec_free_context(&dec_ctxs[0]);
        av_freep(&_avctx->stream_ctx.decode);
    }
    if(_avctx->fmt_ctx)
        avformat_close_input(&_avctx->fmt_ctx);
    *_avctx = (atfp_av_ctx_t) {0};
} // end of  atfp__image_src__avctx_deinit
