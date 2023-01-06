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

int avio_open(AVIOContext **ioc_p, const char *filename, int flags)
{
    AVIOContext *ioc = *ioc_p;
    return (int) mock(ioc, ioc_p, filename, flags);
}

int avio_closep(AVIOContext **ioc_p)
{
    AVIOContext *ioc = *ioc_p;
    return (int) mock(ioc, ioc_p);
}

void avformat_free_context(AVFormatContext *s)
{ mock(s); }

int avformat_open_input(AVFormatContext **ps, const char *url, ff_const59 AVInputFormat *fmt, AVDictionary **options)
{
    AVFormatContext *_fmt_ctx = *ps, **_fmt_ctx_p = &_fmt_ctx;
    int ret = (int) mock(_fmt_ctx, _fmt_ctx_p, url, fmt, options);
    *ps = _fmt_ctx;
    return ret;
}

void avformat_close_input(AVFormatContext **s)
{
    AVFormatContext *ref_fmtctx = *s;
    mock(s, ref_fmtctx);
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
{
    AVStream *out = (AVStream *) mock(s, c);
    if(s)
       s-> nb_streams += 1;
    return out;
}

int avformat_write_header(AVFormatContext *fmt_ctx, AVDictionary **options)
{ return (int) mock(fmt_ctx, options); }

void av_dump_format(AVFormatContext *ic, int index, const char *url, int is_output)
{ mock(ic, index, url, is_output); }

int av_read_frame(AVFormatContext *fmt_ctx, AVPacket *pkt)
{
    int corrupted = 0, *corrupted_p = &corrupted;
    int ret = (int) mock(fmt_ctx, pkt, corrupted_p);
    if(corrupted)
        pkt->flags |= AV_PKT_FLAG_CORRUPT;
    return ret;
}

int av_interleaved_write_frame(AVFormatContext *fmt_ctx, AVPacket *pkt)
{ return (int) mock(fmt_ctx, pkt); }

int av_write_frame(AVFormatContext *fmt_ctx, AVPacket *pkt)
{ return (int) mock(fmt_ctx, pkt); }

int av_write_trailer(AVFormatContext *fmt_ctx)
{ return (int) mock(fmt_ctx); }

