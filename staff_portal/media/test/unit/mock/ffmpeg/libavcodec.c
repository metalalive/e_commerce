#include <libavcodec/avcodec.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVCodec *avcodec_find_encoder_by_name(const char *name)
{ return (AVCodec *)mock(name); }

AVCodec *avcodec_find_decoder_by_name(const char *name)
{ return (AVCodec *)mock(name); }

AVCodec *avcodec_find_decoder(enum AVCodecID id)
{ return (AVCodec *)mock(id); }

AVCodecContext *avcodec_alloc_context3(const AVCodec *codec)
{ return (AVCodecContext *)mock(codec); }

void avcodec_free_context(AVCodecContext **avctx_p)
{ mock(avctx_p); }

int avcodec_open2(AVCodecContext *avctx, const AVCodec *codec, AVDictionary **options)
{ return (int) mock(avctx, codec, options); }

int avcodec_parameters_to_context(AVCodecContext *codec_ctx, const AVCodecParameters *par)
{ return (int) mock(codec_ctx, par); }

int avcodec_parameters_from_context(AVCodecParameters *par, const AVCodecContext *codec_ctx)
{ return (int) mock(par, codec_ctx); }

int avcodec_parameters_copy(AVCodecParameters *dst, const AVCodecParameters *src)
{ return (int) mock(dst, src); }

