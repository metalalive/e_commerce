#include "transcoder/image/ffmpeg.h"

const atfp_ops_entry_t atfp_ops_image_ffmpg_in = {
    .backend_id = ATFP_BACKEND_LIB__FFMPEG,
    .ops =
        {
            .init = atfp__image_ffm_in__init_transcode,
            .deinit = atfp__image_ffm_in__deinit_transcode,
            .processing = atfp__image_ffm_in__proceeding_transcode,
            .instantiate = atfp__image_ffm_in__instantiate_transcoder,
            .label_match = atfp__image_ffm_in__label_match,
            .has_done_processing = atfp__image_ffm_in__has_done_processing,
        },
};
