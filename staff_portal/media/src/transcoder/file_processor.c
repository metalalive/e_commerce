#include <string.h>
#include "transcoder/file_processor.h"

extern atfp_ops_entry_t  atfp_ops_video_mp4;

static  const atfp_ops_entry_t * _atfp_ops_table[] = {
    &atfp_ops_video_mp4,
    NULL,
}; // end of _atfp_ops_table


static const atfp_ops_t * atfp_file_processor_lookup(const char *mimetype)
{
    const atfp_ops_t *found = NULL;
    uint32_t idx = 0;
    for(idx = 0; !found && _atfp_ops_table[idx]; idx++) {
        const atfp_ops_entry_t *item  = _atfp_ops_table[idx];
        int ret = strncmp(mimetype, item->mimetype, strlen(item->mimetype));
        if(ret == 0)
            found = &item->ops;
    }
    return found;
} // end of atfp_file_processor_lookup


atfp_t *app_transcoder_file_processor(const char *mimetype)
{
    atfp_t *out = NULL;
    const atfp_ops_t *ops = atfp_file_processor_lookup(mimetype);
    if(ops) {
        out = calloc(0x1, sizeof(atfp_t));
        out->ops = ops;
    }
    return out;
} // end of app_transcoder_file_processor

