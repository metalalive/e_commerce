#include <libavformat/avformat.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVInputFormat *av_find_input_format(const char *short_name)
{ return (AVInputFormat *) mock(short_name); }

AVOutputFormat *av_guess_format(const char *short_name, const char *filename, const char *mime_type)
{ return (AVOutputFormat *) mock(short_name, filename, mime_type); }

