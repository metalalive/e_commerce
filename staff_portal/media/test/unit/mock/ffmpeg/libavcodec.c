#include <libavcodec/avcodec.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVCodec *avcodec_find_encoder_by_name(const char *name)
{ return (AVCodec *)mock(name); }

AVCodec *avcodec_find_decoder_by_name(const char *name)
{ return (AVCodec *)mock(name); }

