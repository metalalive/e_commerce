#include <libavutil/mem.h>
#include <libavutil/dict.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

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
{ return mock(m); }

int av_strerror(int errnum, char *errbuf, size_t errbuf_size)
{ return (int) mock(errnum, errbuf, errbuf_size); }

void av_log(void *avcl, int level, const char *fmt, ...)
{ return mock(avcl, level, fmt); }

