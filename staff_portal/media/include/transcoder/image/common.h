#ifndef  MEDIA__TRANSCODER__IMAGE_COMMON_H
#define  MEDIA__TRANSCODER__IMAGE_COMMON_H
#ifdef __cplusplus
extern "C" {
#endif

#include "transcoder/file_processor.h"

struct atfp_img_s;

typedef struct atfp_img_s {
    atfp_t        super;
    atfp_av_ctx_t   *av; // low-level A/V context
    union {
        struct {
            void (*avctx_init)(atfp_av_ctx_t *, atfp_av_ctx_t *, const char *filepath,
                    json_t *filt_spec, json_t *err_info);
            void (*avctx_deinit)(atfp_av_ctx_t *);
            ASA_RES_CODE  (*save_to_storage)(struct atfp_img_s *);
            int  (*encode)(atfp_av_ctx_t *);
            int  (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
            void (*avfilter_init)(atfp_av_ctx_t *, atfp_av_ctx_t *, json_t *filt_spec, json_t *err_info);
        } dst;
        struct {
            void (*avctx_init)(atfp_av_ctx_t *, const char *filepath, json_t *err_info);
            void (*avctx_deinit)(atfp_av_ctx_t *);
            ASA_RES_CODE  (*preload_from_storage)(struct atfp_img_s *, void (*)(struct atfp_img_s *));
            int  (*decode_pkt)(atfp_av_ctx_t *);
            int  (*next_pkt)(atfp_av_ctx_t *);
            uint8_t (*done_decoding)(atfp_av_ctx_t *);
        } src;
    } ops;
    union {
        struct {
            void (*_save_done_cb)(struct atfp_img_s *);
            asa_op_localfs_cfg_t  asa_local;
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

#define  NUM_USRARGS_IMG_ASA_LOCAL   (ASAMAP_INDEX__IN_ASA_USRARG + 1)

ASA_RES_CODE  atfp__image_src_preload_start(atfp_img_t *, void (*cb)(atfp_img_t *));

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of  MEDIA__TRANSCODER__IMAGE_COMMON_H
