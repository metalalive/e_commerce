#include "transcoder/file_processor.h"

extern atfp_ops_entry_t  atfp_ops_video_hls;

atfp_ops_entry_t * _atfp_ops_table[] = {
    &atfp_ops_video_hls,
    NULL,
}; // end of _atfp_ops_table
