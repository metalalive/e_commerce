#include "transcoder/file_processor.h"

extern atfp_ops_entry_t atfp_ops_video_mp4;
extern atfp_ops_entry_t atfp_ops_video_hls;
extern atfp_ops_entry_t atfp_ops_image_ffmpg_in;
extern atfp_ops_entry_t atfp_ops_image_ffmpg_out;

atfp_ops_entry_t *_atfp_ops_table[] = {
    &atfp_ops_video_mp4, &atfp_ops_video_hls, &atfp_ops_image_ffmpg_in, &atfp_ops_image_ffmpg_out, NULL,
}; // end of _atfp_ops_table
