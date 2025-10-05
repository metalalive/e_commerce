#include <libavcodec/avcodec.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVCodec *avcodec_find_encoder_by_name(const char *name) { return (AVCodec *)mock(name); }

AVCodec *avcodec_find_decoder_by_name(const char *name) { return (AVCodec *)mock(name); }

AVCodec *avcodec_find_decoder(enum AVCodecID id) { return (AVCodec *)mock(id); }

AVCodecContext *avcodec_alloc_context3(const AVCodec *codec) {
    AVCodecContext *out = (AVCodecContext *)mock(codec);
    if (out)
        out->codec = codec;
    return out;
}

void avcodec_free_context(AVCodecContext **ctx_p) {
    AVCodecContext *ctx = *ctx_p;
    mock(ctx);
}

int avcodec_open2(AVCodecContext *ctx, const AVCodec *codec, AVDictionary **options) {
    return (int)mock(ctx, codec, options);
}

int avcodec_parameters_to_context(AVCodecContext *codec_ctx, const AVCodecParameters *par) {
    return (int)mock(codec_ctx, par);
}

int avcodec_parameters_from_context(AVCodecParameters *par, const AVCodecContext *codec_ctx) {
    return (int)mock(par, codec_ctx);
}

int avcodec_parameters_copy(AVCodecParameters *dst, const AVCodecParameters *src) {
    return (int)mock(dst, src);
}

void av_packet_unref(AVPacket *pkt) { mock(pkt); }

void av_packet_rescale_ts(AVPacket *pkt, AVRational tb_src, AVRational tb_dst) { mock(pkt); }

int avcodec_send_packet(AVCodecContext *codec_ctx, const AVPacket *avpkt) {
    return (int)mock(codec_ctx, avpkt);
}

int avcodec_receive_frame(AVCodecContext *codec_ctx, AVFrame *frame) { return (int)mock(codec_ctx, frame); }

int avcodec_send_frame(AVCodecContext *codec_ctx, const AVFrame *frame) {
    return (int)mock(codec_ctx, frame);
}

int avcodec_receive_packet(AVCodecContext *codec_ctx, AVPacket *avpkt) { return (int)mock(codec_ctx, avpkt); }

AVCodec *avcodec_find_encoder(enum AVCodecID id) { return (AVCodec *)mock(id); }
