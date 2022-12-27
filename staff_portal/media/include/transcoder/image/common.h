#ifndef  MEDIA__TRANSCODER__IMAGE_COMMON_H
#define  MEDIA__TRANSCODER__IMAGE_COMMON_H
#ifdef __cplusplus
extern "C" {
#endif

#include "transcoder/file_processor.h"

struct atfp_img_s;

#define  IMG_S__COMMON_OP_FIELDS \
    int  (*avfilter_init)(struct atfp_img_s *); \
    int  (*avctx_init)(struct atfp_img_s *);    \
    void (*avctx_deinit)(struct atfp_img_s *);

typedef struct atfp_img_s {
    atfp_t        super;
    atfp_av_ctx_t   *av; // low-level A/V context
    union {
        struct {
            IMG_S__COMMON_OP_FIELDS
            ASA_RES_CODE  (*save_to_storage)(struct atfp_img_s *);
            int  (*encode)(atfp_av_ctx_t *dst);
            int  (*filter)(atfp_av_ctx_t *src, atfp_av_ctx_t *dst);
        } dst;
        struct {
            IMG_S__COMMON_OP_FIELDS
            ASA_RES_CODE  (*preload_from_storage)(struct atfp_img_s *, void (*)(struct atfp_img_s *));
            int  (*decode)(atfp_av_ctx_t *);
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

ASA_RES_CODE  atfp__image_src_preload_start(atfp_img_t *, void (*cb)(atfp_img_t *));

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of  MEDIA__TRANSCODER__IMAGE_COMMON_H
