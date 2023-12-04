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
        struct { // TODO, replace with union
            // ---- only for VOD/live stream
            const char * (*get_crypto_key) (json_t *map, const char *key_id, json_t **item_out);
            int  (*encrypt_document_id) (atfp_data_t *, json_t *key_item, unsigned char **out, size_t *out_sz);
            void  (*build_master_playlist)(struct atfp_hls_s *);
            void  (*build_secondary_playlist)(struct atfp_hls_s *);
            void  (*acquire_key)(struct atfp_hls_s *);
            void  (*encrypt_segment)(struct atfp_hls_s *);
            // ---- only for transcoding
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
        atfp_segment_t  segment;
        uint8_t  num_plist_merged; // for master playlist
    } internal;
} atfp_hls_t;

#define  NUM_USRARGS_HLS_ASA_LOCAL  (ASAMAP_INDEX__IN_ASA_USRARG + 1)
// TODO, parameterize
#define  HLS_SEGMENT_FILENAME_PREFIX       "data_seg_"
#define  HLS_SEGMENT_FILENAME_NUM_FORMAT   "%04d"
#define  HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS   4
#define  HLS_SEGMENT_FILENAME_TEMPLATE     HLS_SEGMENT_FILENAME_PREFIX    HLS_SEGMENT_FILENAME_NUM_FORMAT
#define  HLS_FMP4_FILENAME              "init_packet_map"
#define  HLS_PLAYLIST_FILENAME          "lvl2_plist.m3u8"
#define  HLS_MASTER_PLAYLIST_FILENAME   "mst_plist.m3u8"
#define  HLS_CRYPTO_KEY_FILENAME        "crypto_key.json"
#define  HLS_REQ_KEYFILE_LABEL          "key_request"
#define  HLS_PLIST_TARGET_DURATION_MAX_BYTES    15 // max nbytes occupied in tag EXT-X-TARGETDURATION

#define  HLS__NBYTES_KEY_ID     8
#define  HLS__NBYTES_KEY        16 // can be 16, 24, 32 octets
#define  HLS__NBYTES_IV         16 // for AES-CBC, it has to be as the same as block size (16 octets)

// common
atfp_t  *atfp__video_hls__instantiate(void);
uint8_t  atfp__video_hls__label_match(const char *label);

// api server
atfp_t  *atfp__video_hls__instantiate_stream(void);
void     atfp__video_hls__init_stream(atfp_t *);
uint8_t  atfp__video_hls__deinit_stream_element(atfp_t *);
void     atfp__video_hls__seek_stream_element (atfp_t *);

// rpc consumer
atfp_t  *atfp__video_hls__instantiate_transcoder(void);
void     atfp__video_hls__init_transcode(atfp_t *);
void     atfp__video_hls__proceeding_transcode(atfp_t *);
uint8_t  atfp__video_hls__has_done_processing(atfp_t *);
uint8_t  atfp__video_hls__deinit_transcode(atfp_t *);

// internal
int   atfp_hls__av_init(atfp_hls_t *);
void  atfp_hls__av_deinit(atfp_hls_t *);
int   atfp_hls__avfilter_init(atfp_hls_t *);
uint8_t  atfp_av__has_done_processing(atfp_av_ctx_t *dst);

int   atfp_hls__av_filter_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
int   atfp_hls__av_encode_processing(atfp_av_ctx_t *dst);
int   atfp_hls__av_local_write(atfp_av_ctx_t *dst);
int   atfp_hls__av_filter__finalize_processing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
int   atfp_hls__av_encode__finalize_processing(atfp_av_ctx_t *dst);
int   atfp_hls__av_local_write_finalize(atfp_av_ctx_t *);
uint8_t  atfp_av_filter__has_done_flushing(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
uint8_t  atfp_av_encoder__has_done_flushing(atfp_av_ctx_t *dst);

ASA_RES_CODE  atfp_hls__try_flush_to_storage(atfp_hls_t *);

// commonly used in streaming file seeker
void  atfp_hls_stream_seeker__init_common (atfp_hls_t *, ASA_RES_CODE (*)(asa_op_base_cfg_t *, atfp_t *));
void  _atfp_hls__stream_seeker_asalocal_deinit (asa_op_base_cfg_t *_asa_local);
ASA_RES_CODE  atfp_hls_stream__load_crypto_key__async (atfp_hls_t *, asa_close_cb_t);

void  atfp_hls_stream__build_mst_plist(atfp_hls_t *);
void  atfp_hls_stream__build_mst_plist__continue (atfp_hls_t *);
void  atfp_hls_stream__build_lvl2_plist(atfp_hls_t *);
void  atfp_hls_stream__lvl2_plist__parse_header (atfp_hls_t *hlsproc);
void  atfp_hls_stream__lvl2_plist__parse_extinf (atfp_hls_t *hlsproc);
void  atfp_hls_stream__acquire_key(atfp_hls_t *);
void  atfp_hls_stream__acquire_key__final (atfp_hls_t *);
void  atfp_hls_stream__encrypt_segment__start(atfp_hls_t *);
void  atfp_hls_stream__encrypt_segment__continue(atfp_hls_t *);

char * atfp_hls_lvl2pl__load_curr_rd_ptr(atfp_hls_t *);
void   atfp_hls_lvl2pl__save_curr_rd_ptr(atfp_hls_t *, char *ptr);
size_t  atfp_hls_lvl2pl__load_num_unread(atfp_hls_t *);
size_t  atfp_hls_lvl2pl__load_segment_idx(atfp_hls_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_HLS_H
