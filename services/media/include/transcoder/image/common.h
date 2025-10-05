#ifndef MEDIA__TRANSCODER__IMAGE_COMMON_H
#define MEDIA__TRANSCODER__IMAGE_COMMON_H
#ifdef __cplusplus
extern "C" {
#endif

#include "transcoder/file_processor.h"

struct atfp_img_s;

typedef struct atfp_img_s {
    atfp_t         super;
    atfp_av_ctx_t *av; // low-level A/V context
    union {
        struct {
            void (*avctx_init)(
                atfp_av_ctx_t *, atfp_av_ctx_t *, const char *filepath, json_t *filt_spec, json_t *err_info
            );
            void (*avctx_deinit)(atfp_av_ctx_t *);
            void (*avfilter_init)(atfp_av_ctx_t *, atfp_av_ctx_t *, json_t *filt_spec, json_t *err_info);
            int (*write_pkt)(atfp_av_ctx_t *);
            int (*encode)(atfp_av_ctx_t *);
            int (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
            struct {
                int (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
                int (*encode)(atfp_av_ctx_t *);
                int (*write)(atfp_av_ctx_t *);
            } finalize;
            int (*has_done_flush_filter)(atfp_av_ctx_t *);
            ASA_RES_CODE (*save_to_storage)(struct atfp_img_s *);
        } dst;
        struct {
            void (*avctx_init)(atfp_av_ctx_t *, const char *filepath, json_t *err_info);
            void (*avctx_deinit)(atfp_av_ctx_t *);
            ASA_RES_CODE(*preload_from_storage)
            (struct atfp_img_s *, void (*)(struct atfp_img_s *));
            int (*decode_pkt)(atfp_av_ctx_t *);
            int (*next_pkt)(atfp_av_ctx_t *);
            uint8_t (*done_decoding)(atfp_av_ctx_t *);
        } src;
    } ops;
    union {
        struct {
            asa_op_localfs_cfg_t asa_local;
            atfp_segment_t       seginfo;
            void (*deinit_final_cb)(struct atfp_img_s *);
            uint8_t _has_done_processing : 1;
        } dst;
        struct {
            struct {
                void (*done_cb)(struct atfp_img_s *);
                uint32_t nbytes_required;
                uint32_t nbytes_copied;
            } preload;
        } src;
    } internal;
} atfp_img_t;

#define NUM_USRARGS_IMG_ASA_LOCAL (ASAMAP_INDEX__IN_ASA_USRARG + 1)

ASA_RES_CODE atfp__image_src_preload_start(atfp_img_t *, void (*)(atfp_img_t *));
ASA_RES_CODE atfp__image_dst__save_to_storage(atfp_img_t *);

uint8_t     atfp_img_dst_common_deinit(atfp_img_t *, void (*)(atfp_img_t *));
void        atfp_storage_image_remove_version(atfp_t *, const char *status);
void        atfp_image__dst_update_metadata(atfp_t *, void *loop);
const char *atfp_image__metadata_dbtable_name(void);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of  MEDIA__TRANSCODER__IMAGE_COMMON_H
