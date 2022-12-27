#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

void     atfp__image_ffm_out__init_transcode(atfp_t *processor)
{
} // end of  atfp__image_ffm_out__init_transcode

uint8_t  atfp__image_ffm_out__deinit_transcode(atfp_t *processor)
{
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    asaremote->deinit(asaremote);
    processor->data.version = NULL; // app caller should dealloc it
    free(processor);
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
    out->ops.dst.avfilter_init = NULL;
    out->ops.dst.avctx_init = NULL;
    out->ops.dst.avctx_deinit = NULL;
    out->ops.dst.encode = NULL;
    char *ptr = (char *)out + sizeof(atfp_img_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
} // end of  atfp__image_ffm_out__instantiate_transcoder

