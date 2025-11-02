#include "transcoder/image/ffmpeg.h"
#include "transcoder/common/ffmpeg.h"

static int _atfp_config_dst_video_codecctx(AVCodecContext *dst, AVCodecContext *src, json_t *filt_spec) {
    json_t  *_crop_item = json_object_get(filt_spec, "crop");
    json_t  *_scale_item = json_object_get(filt_spec, "scale");
    uint32_t crop_h = json_integer_value(json_object_get(_crop_item, "height"));
    uint32_t crop_w = json_integer_value(json_object_get(_crop_item, "width"));
    uint32_t scale_h = json_integer_value(json_object_get(_scale_item, "height"));
    uint32_t scale_w = json_integer_value(json_object_get(_scale_item, "width"));
    dst->height = FFMIN(crop_h, scale_h);
    dst->width = FFMIN(crop_w, scale_w);
    if (dst->codec->pix_fmts) // take first format from list of supported formats
        dst->pix_fmt = dst->codec->pix_fmts[0];
    else
        dst->pix_fmt = src->pix_fmt;
    dst->sample_aspect_ratio = src->sample_aspect_ratio;
    dst->framerate = src->framerate;
    // dst->bit_rate  = src->bit_rate;
    dst->time_base = src->time_base;
    return 0;
} // end of _atfp_config_dst_video_codecctx

static int _atfp_img_ff_out__av_encoder_init(
    atfp_av_ctx_t *dst, atfp_av_ctx_t *src, json_t *filt_spec, json_t *err_info
) {
    AVFormatContext *ofmt_ctx = dst->fmt_ctx, *ifmt_ctx = src->fmt_ctx;
    AVCodecContext **dec_ctxs = src->stream_ctx.decode, *_dec_ctx = NULL;
    int              err = 0, idx = 0; // TODO, the output file should have only video stream
    for (idx = 0; (!_dec_ctx) && (idx < ifmt_ctx->nb_streams); idx++) {
        if (dec_ctxs[idx]->codec_type == AVMEDIA_TYPE_VIDEO)
            _dec_ctx = dec_ctxs[idx];
    }
    if (!_dec_ctx) {
        json_object_set_new(err_info, "transcoder", json_string("[img][ff_out] missing input video stream"));
        err = AVERROR(EINVAL);
        goto done;
    }
    atfp_stream_enc_ctx_t *enc_ctxs = av_mallocz_array(1, sizeof(atfp_stream_enc_ctx_t));
    dst->stream_ctx.encode = enc_ctxs;
    AVStream *stream_out = avformat_new_stream(ofmt_ctx, NULL);
    if (!stream_out) {
        json_object_set_new(err_info, "transcoder", json_string("[img][ff_out] failed to create stream"));
        err = AVERROR(ENOMEM);
        goto done;
    }
    const AVCodec *encoder = avcodec_find_encoder(_dec_ctx->codec_id); // do NOT reference dec_ctx->codec;
    if (!encoder) {
        json_object_set_new(err_info, "transcoder", json_string("[img][ff_out] invalid decoder ID"));
        err = AVERROR(EINVAL);
        goto done;
    }
    AVCodecContext *enc_ctx = avcodec_alloc_context3(encoder);
    enc_ctxs[0].enc_ctx = enc_ctx;
    if (!enc_ctx) {
        json_object_set_new(
            err_info, "transcoder",
            json_string("[img][ff_out] failed to create encoder context of the stream")
        );
        err = AVERROR(ENOMEM);
        goto done;
    }
    err = _atfp_config_dst_video_codecctx(enc_ctx, _dec_ctx, filt_spec);
    if (err < 0) {
        json_object_set_new(
            err_info, "transcoder", json_string("[img][ff_out] failed to configure video encoder context")
        );
        goto done;
    }
    err = avcodec_open2(enc_ctx, encoder, NULL);
    if (err < 0) {
        json_object_set_new(
            err_info, "transcoder", json_string("[img][ff_out] failed to open encoder context of the stream")
        );
        goto done;
    }
    err = avcodec_parameters_from_context(stream_out->codecpar, enc_ctx);
    if (err < 0) {
        json_object_set_new(
            err_info, "transcoder",
            json_string("[img][ff_out] Failed to copy encoder parameters to output stream")
        );
        goto done;
    }
    stream_out->time_base = enc_ctx->time_base;
done:
    if (json_object_size(err_info) > 0)
        json_object_set_new(err_info, "err_code", json_integer(err));
    return err;
} // end of  _atfp_img_ff_out__av_encoder_init

void atfp__image_dst__avctx_init(
    atfp_av_ctx_t *src, atfp_av_ctx_t *dst, const char *filepath, json_t *filt_spec, json_t *err_info
) {
    int ret = 0;
    if (dst->fmt_ctx && src->fmt_ctx && src->fmt_ctx->iformat) {
        json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] missing format context"));
        ret = AVERROR(EINVAL);
        goto done;
    }
    const char *o_fmt_label = src->fmt_ctx->iformat->name;
    if (!o_fmt_label)
        o_fmt_label = "image2";
    // image2 supports most of static picture types e.g. jpeg, png.
    // For animated picture, this processor currently only supports GIF (TODO, support webp)
    AVOutputFormat *oformat = av_guess_format(o_fmt_label, NULL, NULL);
    if (!oformat) {
        oformat = av_guess_format("image2", NULL, NULL);
        if (!oformat) {
            json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] missing output format"));
            json_object_set_new(err_info, "ofmt_label", json_string(o_fmt_label));
            ret = AVERROR(EINVAL);
            goto done;
        }
    }
    ret = avformat_alloc_output_context2(&dst->fmt_ctx, oformat, NULL, filepath);
    if (ret < 0) {
        json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] failed to init failure"));
        goto done;
    }
    ret = _atfp_img_ff_out__av_encoder_init(dst, src, filt_spec, err_info);
    if (ret != 0)
        goto done;
    if ((oformat->flags & AVFMT_NOFILE) == 0) {
        ret = avio_open(&dst->fmt_ctx->pb, filepath, AVIO_FLAG_WRITE);
        if (ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] failed to open file"));
            goto done;
        }
    }
    ret = avformat_write_header(dst->fmt_ctx, NULL);
    if (ret >= 0) { // return 1 --> already initialized
        dst->intermediate_data.encode._final.file_header_wrote = 1;
    } else {
        char errbuf[128];
        av_strerror(ret, &errbuf[0], 128);
        av_log(NULL, AV_LOG_ERROR, "Error occurred when opening output file, %s \n", &errbuf[0]);
        json_object_set_new(err_info, "transcoder", json_string("[img][ff_out] Failed to write header"));
    }
done:
    if (ret < 0)
        json_object_set_new(err_info, "err_code", json_integer(ret));
} // end of  atfp__image_dst__avctx_init

void atfp__image_dst__avctx_deinit(atfp_av_ctx_t *_avctx) {
    atfp_stream_enc_ctx_t *ig_enc_ctxs = _avctx->stream_ctx.encode;
    uint8_t                f_hdr_wrote = _avctx->intermediate_data.encode._final.file_header_wrote;
    uint8_t                f_trail_wrote = _avctx->intermediate_data.encode._final.file_trailer_wrote;
    if (f_hdr_wrote && !f_trail_wrote) {
        av_write_trailer(_avctx->fmt_ctx);
        _avctx->intermediate_data.encode._final.file_trailer_wrote = 1;
    }
    if (ig_enc_ctxs) {
        int nb_streams = _avctx->fmt_ctx ? _avctx->fmt_ctx->nb_streams : 0;
        for (int idx = 0; idx < nb_streams; idx++) {
            if (ig_enc_ctxs[idx].enc_ctx)
                avcodec_free_context(&ig_enc_ctxs[idx].enc_ctx);
            if (ig_enc_ctxs[idx].filter_graph)
                avfilter_graph_free(&ig_enc_ctxs[idx].filter_graph);
        }
        av_freep(&_avctx->stream_ctx.encode);
    }
    if (_avctx->fmt_ctx) {
        avio_closep(&_avctx->fmt_ctx->pb);
        avformat_free_context(_avctx->fmt_ctx);
    }
    _avctx->fmt_ctx = NULL;
    _avctx->stream_ctx.encode = NULL;
} // end of  atfp__image_dst__avctx_deinit

int atfp__image_dst__encode_frame(atfp_av_ctx_t *_avctx) {
    int ret = AVERROR(EINVAL);
    if (_avctx && _avctx->fmt_ctx && _avctx->fmt_ctx->streams) {
        AVFrame *frame = &_avctx->intermediate_data.encode.frame;
        ret = atfp_common__ffm_encode_processing(_avctx, frame, 0);
    }
    return ret;
}

int atfp__image_dst__flushing_encoder(atfp_av_ctx_t *_avctx) {
    int ret = AVERROR(EINVAL);
    if (_avctx && _avctx->fmt_ctx && _avctx->fmt_ctx->streams) {
        if (!_avctx->intermediate_data.encode._final.encoder_flush_done) {
            ret = atfp_common__ffm_encode_processing(_avctx, NULL, 0);
            if (ret == ATFP_AVCTX_RET__NEED_MORE_DATA)
                _avctx->intermediate_data.encode._final.encoder_flush_done = 1;
        } else { // packets already flushed from all encoders, skip
            ret = ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER;
        }
    }
    return ret;
}

int atfp__image_dst__write_encoded_packet(atfp_av_ctx_t *_avctx) {
    AVPacket *packet = &_avctx->intermediate_data.encode.packet;
    int       ret = av_write_frame(_avctx->fmt_ctx, packet);
    return ret;
}

int atfp__image_dst__final_writefile(atfp_av_ctx_t *_avctx) {
    int     ret = ATFP_AVCTX_RET__OK;
    uint8_t trailer_wrote = _avctx->intermediate_data.encode._final.file_trailer_wrote;
    uint8_t header_wrote = _avctx->intermediate_data.encode._final.file_header_wrote;
    if (header_wrote && !trailer_wrote) {
        ret = av_write_trailer(_avctx->fmt_ctx);
        trailer_wrote = 1;
    }
    _avctx->intermediate_data.encode._final.file_trailer_wrote = trailer_wrote;
    return ret;
}
