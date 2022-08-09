#ifndef MEDIA__TRANSCODER__VIDEO_HLS_H
#define MEDIA__TRANSCODER__VIDEO_HLS_H
#ifdef __cplusplus
extern "C" {
#endif

#include "transcoder/file_processor.h"

struct atfp_hls_s;

typedef struct atfp_hls_s {
    atfp_t  super;
    atfp_av_ctx_t  *av;
    asa_op_localfs_cfg_t  asa_local;
    struct {
        struct {
            int  (*avfilter_init)(struct atfp_hls_s *);
            int  (*avctx_init)(struct atfp_hls_s *);
            void (*avctx_deinit)(struct atfp_hls_s *);
        } op;
        uint32_t curr_segment_idx;
    } internal;
} atfp_hls_t;

int   atfp_hls__avfilter_init(atfp_hls_t *);
int   atfp_hls__av_init(atfp_hls_t *);
void  atfp_hls__av_deinit(atfp_hls_t *);


#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_HLS_H
