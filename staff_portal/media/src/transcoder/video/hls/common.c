#include "storage/cfg_parser.h"
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

atfp_t  *atfp__video_hls__instantiate(void) {
    // at this point, `atfp_av_ctx_t` should NOT be incomplete type
    size_t tot_sz = sizeof(atfp_hls_t) + sizeof(atfp_av_ctx_t);
    atfp_hls_t  *out = calloc(0x1, tot_sz);
    char *ptr = (char *)out + sizeof(atfp_hls_t);
    out->av = (atfp_av_ctx_t *) ptr;
    out->asa_local.super.storage = app_storage_cfg_lookup("localfs") ; 
    return &out->super;
} // end of atfp__video_hls__instantiate

uint8_t    atfp__video_hls__label_match(const char *label) {
    const char *exp_labels[2] = {"hls", "application/x-mpegURL"};
    return atfp_common__label_match(label, 2, exp_labels);
}

