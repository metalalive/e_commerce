#include "utils.h"
#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

static void atfp__image_ffm_in__preload_done_cb(atfp_img_t *imgproc) {
    atfp_t     *processor = &imgproc->super;
    json_t     *err_info = processor->data.error;
    const char *localbuf_path = NULL, *sys_basepath = NULL;
    if (json_object_size(err_info) == 0) {
        asa_op_base_cfg_t    *_asa_src = processor->data.storage.handle;
        atfp_asa_map_t       *_map = _asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_localfs_cfg_t *_asa_local = atfp_asa_map_get_localtmp(_map);
        asa_cfg_t            *storage = _asa_local->super.storage;
        if (storage) {
            sys_basepath = storage->base_path;
            localbuf_path = _asa_local->super.op.open.dst_path;
            if (!localbuf_path || !sys_basepath) {
                json_object_set_new(err_info, "storage", json_string("[img][ff-in][init] path incomplete"));
            }
        } else {
            json_object_set_new(err_info, "storage", json_string("[img][ff-in][init] missing storage"));
        }
    }
    if (json_object_size(err_info) == 0) {
#define RUNNER(fullpath) imgproc->ops.src.avctx_init(imgproc->av, fullpath, err_info)
        PATH_CONCAT_THEN_RUN(sys_basepath, localbuf_path, RUNNER);
#undef RUNNER
    }
    processor->data.callback(processor);
} // end of atfp__image_ffm_in__preload_done_cb

static void atfp_img_ff_in__open_localbuf_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_asa_map_t    *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
    atfp_t            *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t            *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_img_t *imgproc = (atfp_img_t *)processor;
        result = imgproc->ops.src.preload_from_storage(imgproc, atfp__image_ffm_in__preload_done_cb);
        if (result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(
                err_info, "storage", json_string("failed to issue read operation to image input")
            );
            fprintf(
                stderr, "[transcoder][image][ff_in] line:%d, job_id:%s, result:%d \n", __LINE__,
                processor->data.rpc_receipt->job_id.bytes, result
            );
            processor->data.callback(processor);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open local temp buffer"));
        processor->data.callback(processor);
    }
} // end of  atfp_img_ff_in__open_localbuf_cb

void atfp__image_ffm_in__init_transcode(atfp_t *processor) {
    processor->filechunk_seq.curr = processor->filechunk_seq.next = 0;
    processor->filechunk_seq.eof_reached = 0;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    ASA_RES_CODE       result = atfp_src__open_localbuf(asa_src, atfp_img_ff_in__open_localbuf_cb);
    processor->op_async_done.init = result == ASTORAGE_RESULT_ACCEPT;
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            processor->data.error, "storage",
            json_string("failed to issue open operation for local temp buffer")
        );
        processor->data.callback(processor);
    }
} // end of  atfp__image_ffm_in__init_transcode

static void _atfp_img_ffm_in__final_dealloc(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_asa_map_t       *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t    *asa_src = atfp_asa_map_get_source(_map);
    atfp_t               *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    void (*cb)(atfp_t *) = processor->data.callback;
    asa_local->super.deinit(&asa_local->super);
    asa_src->deinit(asa_src);
    free(processor);
    if (cb)
        cb(NULL);
}

static void atfp_img__asalocal_closefile_cb(asa_op_base_cfg_t *asa_local, ASA_RES_CODE result) {
    asa_local->op.unlink.path = asa_local->op.open.dst_path; // local temp buffer file
    asa_local->op.unlink.cb = _atfp_img_ffm_in__final_dealloc;
    fprintf(
        stderr, "[transcoder][img][ff_in][init] line:%d, local buffer path:%s \n", __LINE__,
        asa_local->op.unlink.path
    );
    result = asa_local->storage->ops.fn_unlink(asa_local);
    if (result != ASTORAGE_RESULT_ACCEPT)
        _atfp_img_ffm_in__final_dealloc(asa_local, ASTORAGE_RESULT_COMPLETE);
}

static void atfp_img__asaremote_closefile_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    atfp_asa_map_t       *_map = asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(_map);
    if (asa_local->file.file >= 0) {
        asa_local->super.op.close.cb = atfp_img__asalocal_closefile_cb;
        result = asa_local->super.storage->ops.fn_close(&asa_local->super);
        if (result != ASTORAGE_RESULT_ACCEPT)
            atfp_img__asalocal_closefile_cb(&asa_local->super, ASTORAGE_RESULT_COMPLETE);
    } else {
        atfp_img__asalocal_closefile_cb(&asa_local->super, ASTORAGE_RESULT_COMPLETE);
    }
}

uint8_t atfp__image_ffm_in__deinit_transcode(atfp_t *processor) {
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    imgproc->ops.src.avctx_deinit(imgproc->av);
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_src->op.close.cb = atfp_img__asaremote_closefile_cb;
    uint8_t still_ongoing = asa_src->storage->ops.fn_close(asa_src) == ASTORAGE_RESULT_ACCEPT;
    if (!still_ongoing)
        atfp_img__asaremote_closefile_cb(asa_src, ASTORAGE_RESULT_COMPLETE);
    return still_ongoing;
} // end of  atfp__image_ffm_in__deinit_transcode

void atfp__image_ffm_in__proceeding_transcode(atfp_t *processor) {
    atfp_img_t *_imgproc = (atfp_img_t *)processor;
    json_t     *err_info = processor->data.error;
    uint8_t     frame_avail = 0, err = 0, end_of_file = 1;
    do {
        err = _imgproc->ops.src.decode_pkt(_imgproc->av);
        if (!err) {
            frame_avail = 1;
        } else if (err == 1) { // new packet required
            err = _imgproc->ops.src.next_pkt(_imgproc->av);
            if (err) {
                if (err != end_of_file)
                    json_object_set_new(
                        err_info, "transcoder",
                        json_string("[img][ff-in] "
                                    "error when getting next packet from local temp buffer")
                    );
                break;
            }
        } else {
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff-in] "
                            "failed to decode next packet")
            );
            break;
        }
    } while (!frame_avail);
    processor->op_async_done.processing = 0;
    processor->data.callback(processor);
} // end of  atfp__image_ffm_in__proceeding_transcode

uint8_t atfp__image_ffm_in__has_done_processing(atfp_t *processor) {
    atfp_img_t *_imgproc = (atfp_img_t *)processor;
    return _imgproc->ops.src.done_decoding(_imgproc->av);
} // end of  atfp__image_ffm_in__has_done_processing

uint8_t atfp__image_ffm_in__label_match(const char *label) { // this processor supports several image types
    const char *exp_labels[10] = {"image/jpeg", "image/png", "image/tiff", "image/bmp", "image/gif",
                                  "jpg",        "png",       "bmp",        "tiff",      "gif"};
    return atfp_common__label_match(label, 10, exp_labels);
} // end of  atfp__image_ffm_in__label_match

struct atfp_s *atfp__image_ffm_in__instantiate_transcoder(void) {
    size_t      obj_sz = sizeof(atfp_img_t) + sizeof(atfp_av_ctx_t);
    atfp_img_t *out = calloc(0x1, obj_sz);
    out->ops.src.preload_from_storage = atfp__image_src_preload_start;
    out->ops.src.avctx_init = atfp__image_src__avctx_init;
    out->ops.src.avctx_deinit = atfp__image_src__avctx_deinit;
    out->ops.src.decode_pkt = atfp__image_src__avctx_decode_curr_packet;
    out->ops.src.next_pkt = atfp__image_src__avctx_fetch_next_packet;
    out->ops.src.done_decoding = atfp__image_src__avctx_has_done_decoding;
    char *ptr = (char *)out + sizeof(atfp_img_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
} // end of  atfp__image_ffm_in__instantiate_transcoder
