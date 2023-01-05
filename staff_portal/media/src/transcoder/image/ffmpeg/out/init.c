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
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_av_ctx_t *_avctx_src = NULL, *_avctx_dst = imgproc->av;
    atfp_asa_map_t *_map = asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
    atfp_t *fp_dst = processor, *fp_src = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if((fp_dst->backend_id != fp_src->backend_id) || (fp_dst->backend_id == ATFP_BACKEND_LIB__UNKNOWN))
    {
        json_object_set_new(err_info, "transcoder", json_string("[ff_out] invalid backend"
                    " library in source file processor"));
    } else { // create file lock, to address concurrent transcoding requests
        asa_op_localfs_cfg_t  *asalocal_src =  atfp_asa_map_get_localtmp(_map);
        asa_op_localfs_cfg_t  *asalocal_dst = &imgproc->internal.dst.asa_local;
        const char *_version = processor->data.version;
        json_t  *filt_spec = json_object_get(json_object_get(spec, "outputs"), _version);
#define  PATH_PATTERN   "%s.%s"
        const char *local_tmpfile_basepath = asalocal_src->super.op.open.dst_path;
        size_t path_sz = strlen(local_tmpfile_basepath) + sizeof(PATH_PATTERN) + strlen(_version);
        char fullpath[path_sz];
        size_t nwrite = snprintf(&fullpath[0], path_sz, PATH_PATTERN, local_tmpfile_basepath, _version);
        assert(nwrite < path_sz);
#undef  PATH_PATTERN
        asalocal_dst->super.op.open.dst_path = strdup(&fullpath[0]);
        _avctx_src = ((atfp_img_t *)fp_src)->av;
        imgproc->ops.dst.avctx_init(_avctx_src, _avctx_dst, &fullpath[0], filt_spec, err_info);
        if(json_object_size(err_info) == 0)
            imgproc->ops.dst.avfilter_init(_avctx_src, _avctx_dst, filt_spec, err_info);
    }
    processor -> data.callback(processor); 
} // end of  atfp__image_ffm_out__init_transcode


uint8_t  atfp__image_ffm_out__deinit_transcode(atfp_t *processor)
{
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    imgproc->ops.dst.avctx_deinit(imgproc->av);
    asa_op_base_cfg_t  *asalocal_dst = &imgproc->internal.dst.asa_local.super;
    DEINIT_IF_EXISTS(asalocal_dst->cb_args.entries, free);
    DEINIT_IF_EXISTS(asalocal_dst->op.open.dst_path, free);
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    asaremote->deinit(asaremote);
    processor->data.version = NULL; // app caller should dealloc it
    DEINIT_IF_EXISTS(processor, free);
    return  0;
} // end of  atfp__image_ffm_out__deinit_transcode


void     atfp__image_ffm_out__proceeding_transcode(atfp_t *processor)
{
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    uint8_t   _num_encoded_pkts = imgproc->av->intermediate_data.encode.num_encoded_pkts;
    json_t *err_info = processor->data.error;
    if(++_num_encoded_pkts < 3) {
        imgproc->av->intermediate_data.encode.num_encoded_pkts = _num_encoded_pkts;
        processor -> data.callback(processor);
    } else {
        json_object_set_new(err_info, "dev", json_string("implementation not finished"));
    }
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
