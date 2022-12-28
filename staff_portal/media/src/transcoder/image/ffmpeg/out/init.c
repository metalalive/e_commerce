#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

void     atfp__image_ffm_out__init_transcode(atfp_t *processor)
{
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    json_t *err_info = processor->data.error, *spec = processor->data.spec;
    json_object_set_new(err_info, "dev", json_string("implementation not finished"));
} // end of  atfp__image_ffm_out__init_transcode


uint8_t  atfp__image_ffm_out__deinit_transcode(atfp_t *processor)
{
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    asaremote->deinit(asaremote);
    processor->data.version = NULL; // app caller should dealloc it
    DEINIT_IF_EXISTS(processor, free);
    return  0;
} // end of  atfp__image_ffm_out__deinit_transcode


void     atfp__image_ffm_out__proceeding_transcode(atfp_t *processor)
{
} // end of  atfp__image_ffm_out__proceeding_transcode

uint8_t  atfp__image_ffm_out__has_done_processing(atfp_t *processor)
{
    return  0;
} // end of  atfp__image_ffm_out__has_done_processing


uint8_t  atfp__image_ffm_out__label_match (const char *label)
{
    const char *exp_labels[1] = {"ffmpeg"};
    return atfp_common__label_match(label, 1, exp_labels);
} // end of  atfp__image_ffm_out__label_match


struct atfp_s * atfp__image_ffm_out__instantiate_transcoder(void)
{
    size_t obj_sz = sizeof(atfp_img_t) + sizeof(atfp_av_ctx_t);
    atfp_img_t *out = calloc(0x1, obj_sz);
    out->ops.dst.avctx_init = atfp__image_dst__avctx_init;
    out->ops.dst.avctx_deinit = atfp__image_dst__avctx_deinit;
    out->ops.dst.avfilter_init = atfp__image_dst__avfilt_init;
    out->ops.dst.encode = NULL;
    char *ptr = (char *)out + sizeof(atfp_img_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
} // end of  atfp__image_ffm_out__instantiate_transcoder
