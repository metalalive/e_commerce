#include <libavutil/mem.h>
#include <libavutil/dict.h>
#include <libavutil/rational.h>
#include <libavutil/samplefmt.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

char *av_strdup(const char *s)
{ return s; }

void av_freep(void *ptr)
{ mock(ptr); }

void *av_malloc(size_t sz)
{ return (void *) mock(sz); }

void *av_mallocz_array(size_t nmemb, size_t sz)
{ return (void *) mock(nmemb, sz); }

int av_get_channel_layout_nb_channels(uint64_t channel_layout)
{ return (int) mock(channel_layout); }

int av_dict_set_int(AVDictionary **pm, const char *key, int64_t value, int flags)
{ return (int) mock(pm, key, value, flags); }

int av_dict_set(AVDictionary **pm, const char *key, const char *value, int flags)
{ return (int) mock(pm, key, value, flags); }

void av_dict_free(AVDictionary **m)
{ mock(m); }

int av_opt_set_bin (void *obj, const char *name, const uint8_t *val, int size, int search_flags)
{ return (int) mock(obj, name, val, size); }

int av_strerror(int errnum, char *errbuf, size_t errbuf_size)
{ return (int) mock(errnum, errbuf, errbuf_size); }

void av_log(void *avcl, int level, const char *fmt, ...)
{ mock(avcl, level, fmt); }

AVRational av_mul_q(AVRational b, AVRational c)
{
    int b_num = b.num;
    int c_num = c.num;
    AVRational *out = (AVRational *) mock(b_num, c_num);
    AVRational  _default = {1,1};
    return out ? *out: _default;
}

const char *av_get_sample_fmt_name(enum AVSampleFormat sample_fmt)
{ return (const char *) mock(sample_fmt); }

unsigned int av_int_list_length_for_size(unsigned elsize, const void *list, uint64_t term)
{ return (unsigned int) mock(elsize, list, term); }

int64_t av_get_default_channel_layout(int nb_channels)
{ return (int64_t) mock(nb_channels); }

