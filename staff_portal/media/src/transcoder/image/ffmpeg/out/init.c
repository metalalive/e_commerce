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
    processor->op_async_done.init = 0;
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


static  void _atfp__img_ffm_out__save_transcoded_file_done_cb (atfp_img_t *imgproc)
{
    atfp_t *processor = &imgproc->super;
    imgproc->internal.dst._has_done_processing = 1;
    processor -> data.callback(processor);
}


// __attribute__((optimize("O0")))
void  atfp__image_ffm_out__proceeding_transcode(atfp_t *processor)
{
    int ret = ATFP_AVCTX_RET__OK;
    json_t *err_info = processor->data.error;
    atfp_img_t *imgproc_dst = (atfp_img_t *)processor, *imgproc_src = NULL;
    {
        asa_op_base_cfg_t *asa_dst = processor -> data.storage.handle;
        atfp_asa_map_t    *_map = (atfp_asa_map_t *)asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
        imgproc_src = (atfp_img_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    }
    while(ret == ATFP_AVCTX_RET__OK) {
        ret = imgproc_dst->ops.dst.filter(imgproc_src->av, imgproc_dst->av);
        if(ret) {
            if(ret < ATFP_AVCTX_RET__OK)
                json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] error when filtering"));
            continue;
        } // may return error (ret < 0), or no more frames to filter (ret == 1)
        int ret2 = ATFP_AVCTX_RET__OK;
        while(ret2 == ATFP_AVCTX_RET__OK) {
            ret2 = imgproc_dst->ops.dst.encode(imgproc_dst->av);
            if(ret2 == ATFP_AVCTX_RET__OK) {
                ret2 = imgproc_dst->ops.dst.write_pkt(imgproc_dst->av);
            } else if(ret2 == ATFP_AVCTX_RET__NEED_MORE_DATA) {
                // no more encoded frames to write, break to outer loop for next filtered frame
            } else if(ret2 == ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER) {
                ret = ret2;
            } // all packets already flushed from all encoders, break both loops
            if(ret2 < ATFP_AVCTX_RET__OK) {
                json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] error when encoding"));
                ret = ret2;
            }
        }
    } // end of outer loop
    ASA_RES_CODE result = ASTORAGE_RESULT_COMPLETE;
    uint8_t  src_done = imgproc_src->super.ops->has_done_processing(&imgproc_src->super);
    uint8_t  flush_filt_done = imgproc_dst->ops.dst.has_done_flush_filter(imgproc_dst->av);
    if(src_done) // switch functions as soon as source file processor no longer provides data
        imgproc_dst->ops.dst.filter = imgproc_dst->ops.dst.finalize.filter;
    if(flush_filt_done)
        imgproc_dst->ops.dst.encode = imgproc_dst->ops.dst.finalize.encode;
    if(ret == ATFP_AVCTX_RET__NEED_MORE_DATA) {
        // pass, TODO, for animated picture which includes lot of frames, wait and resume in next event-loop
        //  cycle after recursively invoking this function multiple times, to avoid potential stack overflow
    } else if(ret == ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER) {
        ret = imgproc_dst->ops.dst.finalize.write(imgproc_dst->av);
        // usually there's only one output file in picture processing,
        // it would return `ASTORAGE_RESULT_ACCEPT` on success
        if(ret == ATFP_AVCTX_RET__OK) {
            result = imgproc_dst->ops.dst.save_to_storage(imgproc_dst,
                _atfp__img_ffm_out__save_transcoded_file_done_cb);
        } else {
            json_object_set_new(err_info, "transcoder", json_string("[img][ff-out] error on finalized write"));
        }
    } else if(ret < ATFP_AVCTX_RET__OK) {
        json_object_set_new(err_info, "err_code", json_integer(ret));
        result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    }
    processor->op_async_done .processing = result == ASTORAGE_RESULT_ACCEPT;
} // end of  atfp__image_ffm_out__proceeding_transcode


uint8_t  atfp__image_ffm_out__has_done_processing(atfp_t *processor)
{
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    return  imgproc->internal.dst._has_done_processing;
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
    out->ops.dst.filter = atfp__image_dst__filter_frame;
    out->ops.dst.encode = atfp__image_dst__encode_frame;
    out->ops.dst.write_pkt = atfp__image_dst__write_encoded_packet;
    out->ops.dst.finalize.filter = atfp__image_dst__flushing_filter;
    out->ops.dst.finalize.encode = atfp__image_dst__flushing_encoder;
    out->ops.dst.finalize.write  = atfp__image_dst__final_writefile;
    out->ops.dst.has_done_flush_filter = atfp__image_dst__has_done_flush_filter;
    out->ops.dst.save_to_storage = atfp__image_dst__save_to_storage;
    out->super.transfer.transcoded_dst.update_metadata = atfp_image__dst_update_metadata;
    char *ptr = (char *)out + sizeof(atfp_img_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
} // end of  atfp__image_ffm_out__instantiate_transcoder
