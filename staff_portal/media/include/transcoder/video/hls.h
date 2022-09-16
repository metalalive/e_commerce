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
            int  (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
            int  (*encode)(atfp_av_ctx_t *dst);
            int  (*write)(atfp_av_ctx_t *dst);
            struct {
                int  (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
                int  (*encode)(atfp_av_ctx_t *dst);
                int  (*write)(atfp_av_ctx_t *dst);
            } finalize;
            ASA_RES_CODE  (*move_to_storage)(struct atfp_hls_s *);
            uint8_t  (*has_done_flush_filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
            uint8_t  (*has_done_flush_encoder)(atfp_av_ctx_t *dst);
        } op;
        atfp_segment_t  segment; // TODO, consider to move to parent type `atfp_t`
    } internal;
} atfp_hls_t;

#define  NUM_USRARGS_HLS_ASA_LOCAL  (ASAMAP_INDEX__IN_ASA_USRARG + 1)
// TODO, parameterize
#define  HLS_SEGMENT_FILENAME_PREFIX       "data_seg_"
#define  HLS_SEGMENT_FILENAME_NUM_FORMAT   "%04d"
#define  HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS   4
#define  HLS_SEGMENT_FILENAME_TEMPLATE     HLS_SEGMENT_FILENAME_PREFIX    HLS_SEGMENT_FILENAME_NUM_FORMAT
#define  HLS_FMP4_FILENAME          "init_packet_map"
#define  HLS_PLAYLIST_FILENAME      "playlist.m3u8"

uint8_t  atfp__video_hls__deinit(atfp_t *);
void     atfp_hls__remove_file(atfp_t *, const char *status);

int   atfp_hls__av_init(atfp_hls_t *);
void  atfp_hls__av_deinit(atfp_hls_t *);
int   atfp_hls__avfilter_init(atfp_hls_t *);
uint8_t  atfp_av__has_done_processing(atfp_av_ctx_t *dst);

int   atfp_hls__av_filter_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
int   atfp_hls__av_encode_processing(atfp_av_ctx_t *dst);
int   atfp_hls__av_local_white(atfp_av_ctx_t *dst);
int   atfp_hls__av_filter__finalize_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
int   atfp_hls__av_encode__finalize_processing(atfp_av_ctx_t *dst);
int   atfp_hls__av_local_white_finalize(atfp_av_ctx_t *);
uint8_t  atfp_av_filter__has_done_flushing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
uint8_t  atfp_av_encoder__has_done_flushing(atfp_av_ctx_t *dst);

ASA_RES_CODE  atfp_hls__try_flush_to_storage(atfp_hls_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_HLS_H
