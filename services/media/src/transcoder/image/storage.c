#include "datatypes.h"
#include "transcoder/image/common.h"

static void atfp_img__preload_read_done_cb(asa_op_base_cfg_t *, ASA_RES_CODE, size_t nread);

static void atfp_img__preload_write_done_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nwrite) {
    atfp_asa_map_t    *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
    atfp_t            *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_img_t        *imgproc = (atfp_img_t *)processor;
    json_t            *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint32_t nbytes_copied = imgproc->internal.src.preload.nbytes_copied;
        uint32_t nbytes_total = imgproc->internal.src.preload.nbytes_required;
        nbytes_copied += nwrite;
        imgproc->internal.src.preload.nbytes_copied = nbytes_copied;
        if (nbytes_total <= nbytes_copied) {
            if (nbytes_total < nbytes_copied) {
                json_object_set_new(
                    err_info, "storage", json_string("[img] unknown corruption when loading file")
                );
                fprintf(
                    stderr,
                    "[transcoder][image][preload] line:%d, job_id:%s, nbytes_total:%u, "
                    "nbytes_copied:%u"
                    "\n",
                    __LINE__, processor->data.rpc_receipt->job_id.bytes, nbytes_total, nbytes_copied
                );
            }
            imgproc->internal.src.preload.done_cb(imgproc);
        } else {
            if (processor->filechunk_seq.eof_reached) {
                json_object_set_new(
                    err_info, "storage", json_string("[img] unknown corruption when loading file")
                );
                fprintf(
                    stderr,
                    "[transcoder][image][preload] line:%d, job_id:%s, nbytes_total:%u, "
                    "nbytes_copied:%u"
                    ", eof_reached:%d \n",
                    __LINE__, processor->data.rpc_receipt->job_id.bytes, nbytes_total, nbytes_copied,
                    processor->filechunk_seq.eof_reached
                );
            } else {
                size_t nbytes_max_rdbuf = asa_src->op.read.dst_max_nbytes;
                size_t nbytes_unread = nbytes_total - nbytes_copied;
                size_t expect_nread = MIN(nbytes_max_rdbuf, nbytes_unread);
                asa_src->op.read.dst_sz = expect_nread;
                asa_src->op.read.offset = asa_src->op.seek.pos;
                assert(asa_src->op.read.cb == atfp_img__preload_read_done_cb);
                result = asa_src->storage->ops.fn_read(asa_src);
                if (result != ASTORAGE_RESULT_ACCEPT)
                    json_object_set_new(
                        err_info, "storage", json_string("[img] failed to issue next read operation")
                    );
            } // do not use APP_STORAGE_USE_CURRENT_FILE_OFFSET , it will start reading from last
              // read pointer of the opened file
            if (json_object_size(err_info) > 0)
                imgproc->internal.src.preload.done_cb(imgproc);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("[img] failed to write to local temp buffer"));
        imgproc->internal.src.preload.done_cb(imgproc);
    }
} // end of  atfp_img__preload_write_done_cb

static void atfp_img__preload_read_done_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread) {
    atfp_img_t *imgproc = (atfp_img_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    int         err = atfp_src__rd4localbuf_done_cb(asa_src, result, nread, atfp_img__preload_write_done_cb);
    if (err)
        imgproc->internal.src.preload.done_cb(imgproc);
}

ASA_RES_CODE atfp__image_src_preload_start(atfp_img_t *imgproc, void (*cb)(atfp_img_t *)) {
    atfp_t *processor = &imgproc->super;
    json_t *spec = processor->data.spec, *file_parts = json_object_get(spec, "parts_size");
    size_t  nbytes_to_load = 0;
    if (file_parts && json_is_array(file_parts) && json_array_size(file_parts) == 1) {
        nbytes_to_load = json_integer_value(json_array_get(file_parts, 0));
    } else { // TODO, figure out why it is missing sometimes when num of consumer > 1
        fprintf(
            stderr, "[transcoder][image][preload] line:%d, job_id:%s, file_parts:%p \n", __LINE__,
            processor->data.rpc_receipt->job_id.bytes, file_parts
        );
        return ASTORAGE_RESULT_ARG_ERROR;
    }
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    size_t             dst_max_nbytes = asa_src->op.read.dst_max_nbytes;
    asa_src->op.read.dst_sz = (nbytes_to_load > dst_max_nbytes ? dst_max_nbytes : nbytes_to_load);
    asa_src->op.read.offset = 0; // point back to beginning of file, then read the first few byte
    asa_src->op.read.cb = atfp_img__preload_read_done_cb;
    imgproc->internal.src.preload.nbytes_copied = 0;
    imgproc->internal.src.preload.nbytes_required = nbytes_to_load;
    imgproc->internal.src.preload.done_cb = cb;
    return asa_src->storage->ops.fn_read(asa_src);
} // end of  atfp__image_src_preload_start

static void atfp_img__open_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp__open_local_seg__cb(asaobj, result);
}

static void atfp_img__open_dst_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_img_t *igproc = (atfp_img_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    igproc->super.transfer.transcoded_dst.flags.version_created = result == ASTORAGE_RESULT_COMPLETE;
    atfp__open_dst_seg__cb(&igproc->internal.dst.asa_local.super, &igproc->internal.dst.seginfo, result);
}

static void atfp_img__close_dst_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_img_t *igproc = (atfp_img_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &igproc->super;
    processor->transfer.transcoded_dst.flags.asaremote_open = 0;
    if (result != ASTORAGE_RESULT_COMPLETE)
        json_object_set_new(
            processor->data.error, "storage",
            json_string("[img] failed to close playlist on destination side")
        );
    igproc->internal.dst._has_done_processing = 1;
    processor->data.callback(processor);
}

static void atfp_img__unlink_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp__unlink_local_seg__cb(asaobj, result);
}

static void atfp_img__close_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_img_t *igproc = (atfp_img_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp__close_local_seg__cb(asaobj, &igproc->internal.dst.seginfo, result);
}

static void atfp_img__read_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nread) {
    atfp_img_t *igproc = (atfp_img_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp__read_local_seg__cb(asaobj, &igproc->internal.dst.seginfo, result, nread);
}

static void atfp_img__write_dst_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nwrite) {
    atfp_img_t *igproc = (atfp_img_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp__write_dst_seg__cb(
        &igproc->internal.dst.asa_local.super, &igproc->internal.dst.seginfo, result, nwrite
    );
}

ASA_RES_CODE atfp__image_dst__save_to_storage(atfp_img_t *imgproc) {
    atfp_t               *processor = &imgproc->super;
    asa_op_base_cfg_t    *asa_dst = processor->data.storage.handle;
    asa_op_localfs_cfg_t *asa_local = &imgproc->internal.dst.asa_local;
    atfp_segment_t       *_seg_info = &imgproc->internal.dst.seginfo;
    const char           *filename_local = _seg_info->filename.prefix.data;
    const char           *filename_dst = processor->data.version;
    asa_dst->op.open.cb = atfp_img__open_dst_seg__cb;
    asa_dst->op.close.cb = atfp_img__close_dst_seg__cb;
    asa_dst->op.write.cb = atfp_img__write_dst_seg__cb;
    asa_local->super.op.open.cb = atfp_img__open_local_seg__cb;
    asa_local->super.op.close.cb = atfp_img__close_local_seg__cb;
    asa_local->super.op.read.cb = atfp_img__read_local_seg__cb;
    asa_local->super.op.unlink.cb = atfp_img__unlink_local_seg__cb;
    ASA_RES_CODE result =
        atfp__file_start_transfer(asa_dst, asa_local, _seg_info, filename_local, filename_dst);
    if (result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(
            processor->data.error, "transcoder",
            json_string("[img][storage] error on transferring output file")
        );
    return result;
} // end of  atfp__image_dst__save_to_storage

static void _atfp_remove_version_unlinkfile_done(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    processor->data.callback(processor);
}

void atfp_storage_image_remove_version(atfp_t *processor, const char *status) {
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    json_t            *err_info = processor->data.error;
    uint32_t           _usr_id = processor->data.usr_id;
    uint32_t           _upld_req_id = processor->data.upld_req_id;
    const char        *version = processor->data.version;
    assert(_usr_id);
    assert(_upld_req_id);
    assert(version);
    assert(asa_dst->op.unlink.path == NULL);
    size_t fullpath_sz = strlen(asa_dst->storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
                         UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1 + strlen(status) + 1 + strlen(version) + 1;
    char   fullpath[fullpath_sz];
    size_t nwrite = snprintf(
        &fullpath[0], fullpath_sz, "%s/%d/%08x/%s/%s", asa_dst->storage->base_path, _usr_id, _upld_req_id,
        status, version
    );
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    asa_dst->op.unlink.path = &fullpath[0];
    asa_dst->op.unlink.cb = _atfp_remove_version_unlinkfile_done;
    ASA_RES_CODE result = asa_dst->storage->ops.fn_unlink(asa_dst);
    asa_dst->op.unlink.path = NULL;
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            err_info, "transcode",
            json_string("[image][storage] failed to issue unlink operation for removing files")
        );
        fprintf(
            stderr, "[transcoder][image][storage] error, line:%d, version:%s, result:%d \n", __LINE__,
            processor->data.version, result
        );
        processor->data.callback(processor);
    }
} // end of  atfp_storage_image_remove_version

static void _atfp_img_dst_remove_transcoding_version_done(atfp_t *processor) {
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    json_t     *err_info = processor->data.error;
    if (json_object_size(err_info) > 0) {
        fprintf(
            stderr,
            "[transcoder][img][common][deinit] line:%d, error on"
            " transcoding version folder\n",
            __LINE__
        );
    }
    if (processor->data.error) {
        json_decref(processor->data.error);
        processor->data.error = NULL;
    }
    imgproc->internal.dst.deinit_final_cb(imgproc);
}

static void _atfp_img_dst_remove_discarded_version_done(atfp_t *processor) {
    json_t *err_info = processor->data.error;
    if (json_object_size(err_info) > 0) {
        fprintf(
            stderr,
            "[transcoder][img][common][deinit] line:%d, error on "
            "discarding version folder\n",
            __LINE__
        );
        json_object_clear(err_info);
    }
    if (processor->transfer.transcoded_dst.flags.version_created) {
        processor->data.callback = _atfp_img_dst_remove_transcoding_version_done;
        processor->transfer.transcoded_dst.remove_file(processor, ATFP__TEMP_TRANSCODING_FOLDER_NAME);
    } else {
        _atfp_img_dst_remove_transcoding_version_done(processor);
    }
}

static void _atfp_img_dst__asaremote_closef_cb(asa_op_base_cfg_t *asaremote, ASA_RES_CODE result) {
    atfp_t *processor = asaremote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (processor->transfer.transcoded_dst.flags.version_exists) {
        processor->data.error = json_object();
        processor->data.callback = _atfp_img_dst_remove_discarded_version_done;
        processor->transfer.transcoded_dst.remove_file(processor, ATFP__DISCARDING_FOLDER_NAME);
    } else {
        _atfp_img_dst_remove_discarded_version_done(processor);
    }
}

static void _atfp_img_dst__asalocal_unlinkf_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_t            *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asaremote = processor->data.storage.handle;
    uint8_t            asa_remote_open = processor->transfer.transcoded_dst.flags.asaremote_open != 0;
    asaobj->op.unlink.path = NULL;
    if (asa_remote_open) {
        asaremote->op.close.cb = _atfp_img_dst__asaremote_closef_cb;
        result = asaremote->storage->ops.fn_close(asaremote);
        if (result != ASTORAGE_RESULT_ACCEPT)
            _atfp_img_dst__asaremote_closef_cb(asaremote, ASTORAGE_RESULT_COMPLETE);
    } else {
        _atfp_img_dst__asaremote_closef_cb(asaremote, ASTORAGE_RESULT_COMPLETE);
    }
}

static void _atfp_img_dst__asalocal_closef_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    asaobj->op.unlink.path = asaobj->op.open.dst_path;
    asaobj->op.unlink.cb = _atfp_img_dst__asalocal_unlinkf_cb;
    result = asaobj->storage->ops.fn_unlink(asaobj);
    if (result != ASTORAGE_RESULT_ACCEPT) // TODO, logging
        _atfp_img_dst__asalocal_unlinkf_cb(asaobj, ASTORAGE_RESULT_COMPLETE);
}

uint8_t atfp_img_dst_common_deinit(atfp_img_t *imgproc, void (*cb)(atfp_img_t *)) {
    ASA_RES_CODE       result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    atfp_t            *processor = &imgproc->super;
    asa_op_base_cfg_t *asalocal_dst = &imgproc->internal.dst.asa_local.super;
    imgproc->internal.dst.deinit_final_cb = cb;
    uint8_t asa_local_open = processor->transfer.transcoded_dst.flags.asalocal_open != 0;
    uint8_t asa_remote_open = processor->transfer.transcoded_dst.flags.asaremote_open != 0;
    if (asa_local_open) {
        asalocal_dst->op.close.cb = _atfp_img_dst__asalocal_closef_cb;
        result = asalocal_dst->storage->ops.fn_close(asalocal_dst);
        if (result != ASTORAGE_RESULT_ACCEPT) {
            asa_local_open = 0;
            _atfp_img_dst__asalocal_closef_cb(asalocal_dst, ASTORAGE_RESULT_COMPLETE);
        }
    } else {
        _atfp_img_dst__asalocal_closef_cb(asalocal_dst, ASTORAGE_RESULT_COMPLETE);
    }
    return asa_remote_open || asa_local_open;
}
