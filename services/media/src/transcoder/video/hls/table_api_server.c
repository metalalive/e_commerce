#include "transcoder/video/hls.h"

atfp_ops_entry_t atfp_ops_video_hls = {
    .backend_id = ATFP_BACKEND_LIB__FFMPEG,
    .ops =
        {
            .init = atfp__video_hls__init_stream,
            .deinit = atfp__video_hls__deinit_stream_element,
            .processing = atfp__video_hls__seek_stream_element,
            .instantiate = atfp__video_hls__instantiate_stream,
            .label_match = atfp__video_hls__label_match,
        },
};
