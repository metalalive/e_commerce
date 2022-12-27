#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

static void atfp__image_ffm_in__preload_done_cb (atfp_img_t *imgproc)
{
    atfp_t *processor = &imgproc->super;
    json_t *err_info = processor->data.error;
    json_object_set_new(err_info, "dev", json_string("implementation not finished"));
    processor -> data.callback(processor);
} // end of atfp__image_ffm_in__preload_done_cb


static void atfp_img_ff_in__open_localbuf_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_asa_map_t  *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
    atfp_t *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_img_t *imgproc = (atfp_img_t *)processor;
        result = imgproc->ops.src.preload_from_storage(imgproc, atfp__image_ffm_in__preload_done_cb);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("failed to issue read operation to image input"));
            fprintf(stderr, "[transcoder][image][ff_in] line:%d, job_id:%s, result:%d \n",
                  __LINE__, processor->data.rpc_receipt->job_id.bytes, result);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open local temp buffer"));
    }
    if(json_object_size(err_info) > 0) 
        processor -> data.callback(processor);
} // end of  atfp_img_ff_in__open_localbuf_cb


void     atfp__image_ffm_in__init_transcode(atfp_t *processor)
{
    processor->filechunk_seq.curr = processor->filechunk_seq.next = 0;
    processor->filechunk_seq.eof_reached = 0;
    asa_op_base_cfg_t *asaobj = processor->data.storage.handle;
    ASA_RES_CODE  result = atfp_src__open_localbuf(asaobj, atfp_img_ff_in__open_localbuf_cb);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("failed to issue open operation for local temp buffer"));
        processor -> data.callback(processor);
    }
} // end of  atfp__image_ffm_in__init_transcode



static  void  _atfp_img_ffm_in__final_dealloc (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_asa_map_t  *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t    *asa_src   = atfp_asa_map_get_source(_map);
    atfp_t *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    void (*cb)(atfp_t *) = processor->data.callback;
    asa_local->super.deinit(&asa_local->super);
    asa_src->deinit(asa_src);
    free(processor);
    if(cb)
        cb(NULL);
}

static void  atfp_img__asalocal_closefile_cb(asa_op_base_cfg_t *asa_local, ASA_RES_CODE result)
{
    asa_local->op.unlink.path = asa_local->op.open.dst_path; // local temp buffer file
    asa_local->op.unlink.cb   = _atfp_img_ffm_in__final_dealloc;
    fprintf(stderr, "[transcoder][img][ff_in][init] line:%d, local buffer path:%s \n",
              __LINE__, asa_local->op.unlink.path);
    result = asa_local->storage->ops.fn_unlink(asa_local);
    if(result != ASTORAGE_RESULT_ACCEPT)
        _atfp_img_ffm_in__final_dealloc(asa_local, ASTORAGE_RESULT_COMPLETE);
}

static void  atfp_img__asaremote_closefile_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    atfp_asa_map_t  *_map = asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local =  atfp_asa_map_get_localtmp(_map);
    if(asa_local->file.file >= 0) {
        asa_local->super.op.close.cb =  atfp_img__asalocal_closefile_cb;
        result = asa_local->super.storage->ops.fn_close(&asa_local->super);
        if(result != ASTORAGE_RESULT_ACCEPT)
            atfp_img__asalocal_closefile_cb(&asa_local->super, ASTORAGE_RESULT_COMPLETE);
    } else {
        atfp_img__asalocal_closefile_cb(&asa_local->super, ASTORAGE_RESULT_COMPLETE);
    }
}

uint8_t  atfp__image_ffm_in__deinit_transcode(atfp_t *processor)
{
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_src->op.close.cb = atfp_img__asaremote_closefile_cb;
    uint8_t still_ongoing = asa_src->storage->ops.fn_close(asa_src) == ASTORAGE_RESULT_ACCEPT;
    if(!still_ongoing)
        atfp_img__asaremote_closefile_cb(asa_src, ASTORAGE_RESULT_COMPLETE);
    return  still_ongoing;
} // end of  atfp__image_ffm_in__deinit_transcode





void     atfp__image_ffm_in__proceeding_transcode(atfp_t *processor)
{
} // end of  atfp__image_ffm_in__proceeding_transcode

uint8_t  atfp__image_ffm_in__has_done_processing(atfp_t *processor)
{
    return 0;
} // end of  atfp__image_ffm_in__has_done_processing

uint8_t  atfp__image_ffm_in__label_match (const char *label)
{ // this processor supports several image types
    const char *exp_labels[7] = {"image/jpeg", "image/x-apple-ios-png", "image/tiff",
            "image/bmp", "jpg", "png", "bmp"};
    return atfp_common__label_match(label, 7, exp_labels);
} // end of  atfp__image_ffm_in__label_match


struct atfp_s * atfp__image_ffm_in__instantiate_transcoder(void)
{
    size_t obj_sz = sizeof(atfp_img_t) + sizeof(atfp_av_ctx_t);
    atfp_img_t *out = calloc(0x1, obj_sz);
    out->ops.src.preload_from_storage = atfp__image_src_preload_start;
    out->ops.src.avfilter_init = NULL;
    out->ops.src.avctx_init = NULL;
    out->ops.src.avctx_deinit = NULL;
    out->ops.src.decode = NULL;
    char *ptr = (char *)out + sizeof(atfp_img_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
} // end of  atfp__image_ffm_in__instantiate_transcoder
