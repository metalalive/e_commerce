#include <unistd.h>
#include <string.h>
#include "transcoder/video/hls.h"

// NOTE: unlinking local file has to be done before closing file in destination storage.
// if you do both concurrently in one event-loop cycle, that will cause segmentation fault
// at next event loop which is extremely difficult to debug. 
static  void atfp_hls__close_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_local = asaobj;
        asa_local->op.unlink.path = asa_local-> op.open.dst_path ;
        result =  app_storage_localfs_unlink(asa_local);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to close transferred segment file on local side"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__close_local_seg__cb

static  void atfp_hls__unlink_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
        asa_cfg_t  *storage = processor->data.storage.config;
        result =  storage->ops.fn_close(asa_dst);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to unlink transferred segment file on local side"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__unlink_local_seg__cb

static  void atfp_hls__close_dst_playlist__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if(result != ASTORAGE_RESULT_COMPLETE)
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to close playlist on destination side"));
    processor -> data.callback(processor);
}

static  void atfp_hls__close_dst_initmap__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_t    *seg_cfg = &hlsproc->internal.segment;
        asa_op_base_cfg_t *asa_dst = asaobj;
        asa_op_localfs_cfg_t  *asa_local = &hlsproc->asa_local;
        result = atfp__file_start_transfer(asa_dst, asa_local, seg_cfg, HLS_PLAYLIST_FILENAME);
        asa_dst->op.close.cb = atfp_hls__close_dst_playlist__cb;
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to close init-map file on destination side"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__close_dst_initmap__cb

static  void atfp_hls__close_dst_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_t    *seg_cfg = &hlsproc->internal.segment;
        asa_op_base_cfg_t *asa_dst = asaobj;
        asa_op_localfs_cfg_t  *asa_local = &hlsproc->asa_local;
        int nxt_seg_idx = seg_cfg->transfer.curr_idx + 1;
        //uv_fs_req_cleanup(&asa_local->file); // libuv cleaned up fs request at the end of previous event-loop cycle
        result = atfp__segment_start_transfer(asa_dst, asa_local, seg_cfg, nxt_seg_idx);
        if(result == ASTORAGE_RESULT_COMPLETE) {
            uint8_t process_done = processor->ops->has_done_processing(processor);
            if(process_done) {
                result = atfp__file_start_transfer(asa_dst, asa_local, seg_cfg, HLS_FMP4_FILENAME);
                // change file-close callback for initial map file and playlist
                asa_dst->op.close.cb = atfp_hls__close_dst_initmap__cb;
                err = result != ASTORAGE_RESULT_ACCEPT;
            } else {
                err = 0; // all available segments are transferred to destination storage
                processor -> data.callback(processor);
            }
        } else {
            err = result != ASTORAGE_RESULT_ACCEPT;
        }
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to close segment file on destination side"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__close_dst_seg__cb


static  void atfp_hls__open_local_seg__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
        asa_cfg_t  *storage = processor->data.storage.config;
        result =  storage->ops.fn_open(asa_dst);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to open local segment file for transfer"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__open_local_seg__cb

static  void atfp_hls__open_dst_seg__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
        seg_cfg->transfer.eof_reached = 0;
        result = app_storage_localfs_read(&hlsproc->asa_local.super);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to open segment file on destination side for transfer"));
        processor -> data.callback(processor);
    }
} // end of atfp_hls__open_dst_seg__cb

static void atfp_hls__read_local_seg__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nread)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
        seg_cfg->transfer.eof_reached = asaobj->op.read.dst_sz > nread;
        if(nread == 0) {
            result = app_storage_localfs_close(asaobj);
        } else {
            asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
            asa_cfg_t  *storage = processor->data.storage.config;
            asa_dst->op.write.src_sz = nread;
            result =  storage->ops.fn_write(asa_dst);
        }
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to read from local segment file for transfer"));
        processor -> data.callback(processor);
    } // TODO, close file-descriptors on both ends
} // end of atfp_hls__read_local_seg__cb

static void atfp_hls__write_dst_seg__cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result, size_t nwrite)
{
    int err = 1;
    atfp_hls_t *hlsproc = (atfp_hls_t *) asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
        if(seg_cfg->transfer.eof_reached) { // switch to next segment (if exists)
            result = app_storage_localfs_close(&hlsproc->asa_local.super);
        } else {
            result = app_storage_localfs_read(&hlsproc->asa_local.super);
        }
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to transfer to destination segment file"));
        processor -> data.callback(processor);
    } // TODO, close file-descriptors on both ends
} // end of atfp_hls__write_dst_seg__cb


int atfp__collect_output_segment_num (uv_fs_t* req, int *out, size_t o_sz,
          const char *seg_prefix, size_t seg_prefix_sz, uint8_t final)
{
    int err = 0, idx = 0, num_segs_rdy = 0, curr_seg_max_idx = -1,
        curr_seg_max_num = -1;
    uv_dirent_t  curr_entry = {0};
    for(idx = 0; idx < o_sz; idx++) {
        err = uv_fs_scandir_next(req, &curr_entry);
        if((err == UV_EOF) || (err < 0)) 
            break;
        if(curr_entry.type != UV_DIRENT_FILE)
            continue;
        const char *filename = curr_entry.name;
        int not_match = strncmp(filename, seg_prefix, seg_prefix_sz);
        if(not_match)
            continue;
        int seg_num = (int) strtol(&filename[seg_prefix_sz], NULL, 10);
        if(seg_num > curr_seg_max_num) {
            curr_seg_max_num = seg_num;
            curr_seg_max_idx = num_segs_rdy;
        }
        out[num_segs_rdy++] = seg_num;
    } // end of loop
    // the segment with currently largest number might NOT be ready, skip it for current flush operation
    // (and will be transferring next time)
    if(!final && num_segs_rdy > 0)
        out[curr_seg_max_idx] = out[--num_segs_rdy];
    return num_segs_rdy;
} // end of  atfp__collect_output_segment_num


static void  atfp_hls__scan_local_tmpbuf_cb(uv_fs_t* req)
{
    atfp_hls_t *hlsproc = req->data;
    atfp_t *processor = &hlsproc->super;
    atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
    int end = 1, nitems = req->result, num_segs_created = 0;
    free((char *)req->path);
    if(nitems > 0) {
        int avail_seg_numbers[nitems];
        uint8_t process_done = processor->ops->has_done_processing(processor);
        num_segs_created = atfp__collect_output_segment_num( req, &avail_seg_numbers[0], nitems,
               seg_cfg->filename.prefix.data,  seg_cfg->filename.prefix.sz, process_done );
        if(num_segs_created > 0) {
            h2o_vector_reserve(NULL, &seg_cfg->rdy_list, num_segs_created);
            seg_cfg-> rdy_list.size = num_segs_created;
            memcpy(seg_cfg-> rdy_list.entries, &avail_seg_numbers[0], sizeof(int) * num_segs_created);
            // ---- 
            asa_op_base_cfg_t     *asa_dst = processor->data.storage.handle;
            asa_op_localfs_cfg_t  *asa_local = &hlsproc->asa_local;
            asa_dst->op.open.cb  = atfp_hls__open_dst_seg__cb;
            asa_dst->op.close.cb = atfp_hls__close_dst_seg__cb;
            asa_dst->op.write.cb = atfp_hls__write_dst_seg__cb;
            asa_local->super.op.open.cb  = atfp_hls__open_local_seg__cb;
            asa_local->super.op.close.cb = atfp_hls__close_local_seg__cb;
            asa_local->super.op.read.cb  = atfp_hls__read_local_seg__cb;
            asa_local->super.op.unlink.cb = atfp_hls__unlink_local_seg__cb;
            ASA_RES_CODE  result = atfp__segment_start_transfer(asa_dst, asa_local, seg_cfg, 0);
            end = result != ASTORAGE_RESULT_ACCEPT;
            if(end)
                json_object_set_new(processor->data.error, "storage",
                   json_string("[hls] failed to start transfer at first ready segment"));
        }
    } else {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] nitems should not be negative in local temp buf"));
    }
    if(end)
        processor -> data.callback(processor);
} // end of atfp_hls__scan_local_tmpbuf_cb


// All functions in this file is supposed to work with ffmpeg, since ffmpeg does not
// support non-locking API functions, application may never know when a segment file
// is ready to transfer.
// The function below scans the folder of local temp buffer to list all existing
// segment files (they are either processed or being processing), try to determine
// which segment(s) can be transferring and which are not ready yet....
ASA_RES_CODE  atfp_hls__try_flush_to_storage(atfp_hls_t *hlsproc)
{ // TODO, reduce number of flushing operations
    asa_op_localfs_cfg_t  *asa_local_dst = &hlsproc->asa_local;
    const char *basepath = asa_local_dst->super.op.mkdir.path.origin;
    asa_local_dst->file.data = hlsproc;
    int err = uv_fs_scandir(asa_local_dst->loop,  &asa_local_dst->file, basepath,
                0,  atfp_hls__scan_local_tmpbuf_cb );
    return err ? ASTORAGE_RESULT_OS_ERROR: ASTORAGE_RESULT_ACCEPT;
} // end of atfp_hls__try_flush_to_storage
