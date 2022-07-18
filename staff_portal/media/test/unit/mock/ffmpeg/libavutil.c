#include <libavutil/mem.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

void av_freep(void *ptr)
{ mock(ptr); }

void *av_malloc(size_t sz)
{ return (void *) mock(sz); }

