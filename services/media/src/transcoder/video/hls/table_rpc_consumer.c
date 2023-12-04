#include "transcoder/video/hls.h"

atfp_ops_entry_t  atfp_ops_video_hls = {
    .backend_id = ATFP_BACKEND_LIB__FFMPEG,
    .ops = {
        .init   = atfp__video_hls__init_transcode,
        .deinit = atfp__video_hls__deinit_transcode,
        .processing  = atfp__video_hls__proceeding_transcode,
        .instantiate = atfp__video_hls__instantiate_transcoder,
        .label_match = atfp__video_hls__label_match,
        .has_done_processing = atfp__video_hls__has_done_processing,
    },
};

