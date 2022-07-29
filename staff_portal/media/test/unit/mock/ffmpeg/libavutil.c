#include <libavutil/mem.h>
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

