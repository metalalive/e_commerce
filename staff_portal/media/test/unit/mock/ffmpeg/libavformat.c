#include <libavformat/avformat.h>
#include <libavformat/avio.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVInputFormat *av_find_input_format(const char *short_name)
{ return (AVInputFormat *) mock(short_name); }

AVOutputFormat *av_guess_format(const char *short_name, const char *filename, const char *mime_type)
{ return (AVOutputFormat *) mock(short_name, filename, mime_type); }

AVFormatContext *avformat_alloc_context(void)
{ return (AVFormatContext *) mock(); }

int avformat_alloc_output_context2(AVFormatContext **fmtctx_p, ff_const59 AVOutputFormat *oformat,
                                   const char *fmt_name, const char *filename)
{ return (int) mock(fmtctx_p, oformat, fmt_name, filename); }


AVIOContext *avio_alloc_context(
                  unsigned char *buffer,
                  int buffer_size,
                  int write_flag,
                  void *opaque,
                  int (*read_pkt_fn)(void *opaque, uint8_t *buf, int buf_size),
                  int (*write_pkt_fn)(void *opaque, uint8_t *buf, int buf_size),
                  int64_t (*seek_fn)(void *opaque, int64_t offset, int whence))
{
    return (AVIOContext *) mock(buffer, buffer_size, write_flag, opaque,
             read_pkt_fn, write_pkt_fn, seek_fn);
}

void avio_context_free(AVIOContext **s)
{
    mock(s);
    *s = NULL;
}

void avformat_free_context(AVFormatContext *s)
{ mock(s); }

int avformat_open_input(AVFormatContext **ps, const char *url, ff_const59 AVInputFormat *fmt, AVDictionary **options)
{
    AVFormatContext *_fmt_ctx = *ps;
    return (int) mock(_fmt_ctx, url, fmt, options);
}

void avformat_close_input(AVFormatContext **s)
{
    mock(s);
    *s = NULL;
}

int avformat_find_stream_info(AVFormatContext *ic, AVDictionary **options)
{ return (int) mock(ic, options); }

AVRational av_guess_frame_rate(AVFormatContext *fmtctx, AVStream *stream, AVFrame *frame)
{
    AVRational out = {
        (int) mock(fmtctx, stream, frame),
        (int) mock(),
    };
    return out;
}

AVStream *avformat_new_stream(AVFormatContext *s, const AVCodec *c)
{ return (AVStream *) mock(s, c); }

