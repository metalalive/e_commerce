#include "transcoder/image/common.h"

static void  atfp_img__preload_read_done_cb (asa_op_base_cfg_t *, ASA_RES_CODE, size_t nread);

static void  atfp_img__preload_write_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nwrite)
{
    atfp_asa_map_t  *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
    atfp_t *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_img_t *imgproc = (atfp_img_t *)processor;
    json_t    *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        uint32_t  nbytes_copied = imgproc->internal.src.preload.nbytes_copied;
        uint32_t  nbytes_total = imgproc->internal.src.preload.nbytes_required;
        nbytes_copied += nwrite;
        imgproc->internal.src.preload.nbytes_copied = nbytes_copied;
        if(nbytes_total <= nbytes_copied) {
            if(nbytes_total < nbytes_copied) {
                json_object_set_new(err_info, "storage", json_string("[img] unknown corruption when loading file"));
                fprintf(stderr, "[transcoder][image][preload] line:%d, job_id:%s, nbytes_total:%u, nbytes_copied:%u"
                        "\n", __LINE__, processor->data.rpc_receipt->job_id.bytes, nbytes_total, nbytes_copied );
            }
            imgproc->internal.src.preload.done_cb(imgproc);
        } else {
            if(processor->filechunk_seq.eof_reached) {
                json_object_set_new(err_info, "storage", json_string("[img] unknown corruption when loading file"));
                fprintf(stderr, "[transcoder][image][preload] line:%d, job_id:%s, nbytes_total:%u, nbytes_copied:%u"
                        ", eof_reached:%d \n", __LINE__, processor->data.rpc_receipt->job_id.bytes,
                        nbytes_total, nbytes_copied, processor->filechunk_seq.eof_reached );
            } else {
                size_t nbytes_max_rdbuf = asa_src->op.read.dst_max_nbytes;
                size_t nbytes_unread = nbytes_total - nbytes_copied;
                size_t expect_nread = MIN(nbytes_max_rdbuf, nbytes_unread);
                asa_src->op.read.dst_sz = expect_nread;
                asa_src->op.read.offset = asa_src->op.seek.pos;
                assert(asa_src->op.read.cb ==  atfp_img__preload_read_done_cb);
                result = asa_src->storage->ops.fn_read(asa_src);
                if(result != ASTORAGE_RESULT_ACCEPT)
                    json_object_set_new(err_info, "storage", json_string("[img] failed to issue next read operation"));
            } // do not use APP_STORAGE_USE_CURRENT_FILE_OFFSET , it will start reading from last read pointer of the opened file
            if(json_object_size(err_info) > 0)
                imgproc->internal.src.preload.done_cb(imgproc);
        }
    } else {
        json_object_set_new(err_info, "storage", json_string("[img] failed to write to local temp buffer"));
        imgproc->internal.src.preload.done_cb(imgproc);
    }
} // end of  atfp_img__preload_write_done_cb


static void  atfp_img__preload_read_done_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread)
{
    atfp_img_t *imgproc = (atfp_img_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    int err = atfp_src__rd4localbuf_done_cb (asa_src, result, nread,
           atfp_img__preload_write_done_cb );
    if(err)
        imgproc->internal.src.preload.done_cb(imgproc) ;
}


ASA_RES_CODE  atfp__image_src_preload_start(atfp_img_t *imgproc, void (*cb)(atfp_img_t *))
{
    atfp_t *processor = & imgproc -> super;
    json_t *spec = processor ->data.spec, *file_parts = json_object_get(spec, "parts_size");
    size_t nbytes_to_load = 0;
    if(file_parts && json_is_array(file_parts) && json_array_size(file_parts) == 1) {
        nbytes_to_load = json_integer_value(json_array_get(file_parts, 0));
    } else {
        fprintf(stderr, "[transcoder][image][preload] line:%d, job_id:%s, file_parts:%p \n",
              __LINE__, processor->data.rpc_receipt->job_id.bytes, file_parts);
        return ASTORAGE_RESULT_ARG_ERROR;
    }
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    size_t dst_max_nbytes = asa_src->op.read.dst_max_nbytes;
    asa_src->op.read.dst_sz = (nbytes_to_load > dst_max_nbytes ? dst_max_nbytes: nbytes_to_load);
    asa_src->op.read.offset = 0; // point back to beginning of file, then read the first few byte
    asa_src->op.read.cb = atfp_img__preload_read_done_cb;
    imgproc->internal.src.preload.nbytes_copied = 0;
    imgproc->internal.src.preload.nbytes_required = nbytes_to_load;
    imgproc->internal.src.preload.done_cb = cb;
    return asa_src->storage->ops.fn_read(asa_src);
} // end of  atfp__image_src_preload_start
