#include <assert.h>
#include <string.h>

#include "views.h"
#include "transcoder/video/hls.h"


#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static  void _atfp_removefile__dealloc(asa_op_base_cfg_t *asa_dst)
{
    asa_dst->op.rmdir.path = NULL;
    DEINIT_IF_EXISTS(asa_dst->op.scandir.path, free);
    if(asa_dst->op.scandir.fileinfo.data) {
        for(int idx = 0; idx < asa_dst->op.scandir.fileinfo.size; idx++) {
            asa_dirent_t  *e = &asa_dst->op.scandir.fileinfo.data[idx];
            DEINIT_IF_EXISTS(e->name, free);
        }
    }
    DEINIT_IF_EXISTS(asa_dst->op.scandir.fileinfo.data, free);
}

static  void  _atfp__hls_version_removal_done_cb(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result != ASTORAGE_RESULT_COMPLETE)
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to remove folder for discarding old version"));
    _atfp_removefile__dealloc(asa_dst);
    processor->data.callback(processor);
} // end of _atfp__hls_version_removal_done_cb


static  void  _atfp_hls_rmdir(asa_op_base_cfg_t *asa_dst, json_t *err_info)
{
    asa_dst->op.rmdir.path = asa_dst->op.scandir.path;
    asa_dst->op.rmdir.cb   = _atfp__hls_version_removal_done_cb;
    ASA_RES_CODE result =  asa_dst->storage->ops.fn_rmdir(asa_dst);
    if(result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(err_info, "transcode", json_string(
           "[storage] failed to issue rmdir operation for discarding old version"));
}

static  void  _atfp_removefile__unlink_start (asa_op_base_cfg_t *asa_dst, json_t *err_info, asa_unlink_cb_t cb)
{
    uint32_t  max_num_files = asa_dst->op.scandir.fileinfo.size;
    uint32_t  curr_rd_idx   = asa_dst->op.scandir.fileinfo.rd_idx ++;
    assert(max_num_files > curr_rd_idx);
    asa_dirent_t  *e = &asa_dst->op.scandir.fileinfo.data[ curr_rd_idx ];
    size_t  fullpath_sz = strlen(asa_dst->op.scandir.path) + 1 + strlen(e->name) + 1 ;
    char  fullpath[fullpath_sz];
    size_t nwrite = snprintf(&fullpath[0], fullpath_sz, "%s/%s", asa_dst->op.scandir.path, e->name);
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    asa_dst->op.unlink.path = &fullpath[0];
    asa_dst->op.unlink.cb   =  cb;
    ASA_RES_CODE result =  asa_dst->storage->ops.fn_unlink(asa_dst);
    if(result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(err_info, "transcode", json_string(
           "[storage] failed to issue unlink operation for removing files"));
} // end of  _atfp_removefile__unlink_start


static  void  _atfp_removefile__unlink_done_cb(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint32_t  max_num_files = asa_dst->op.scandir.fileinfo.size;
        uint32_t  curr_rd_idx   = asa_dst->op.scandir.fileinfo.rd_idx;
        if(curr_rd_idx < max_num_files) {
            _atfp_removefile__unlink_start (asa_dst, err_info, _atfp_removefile__unlink_done_cb);
        } else {
            _atfp_hls_rmdir(asa_dst, err_info);
        }
    } else {
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to scan folder path for removing files"));
    }
    if(json_object_size(err_info) > 0) {
        // TODO, cleanup internal alloc memory in asa_dst
        _atfp_removefile__dealloc(asa_dst);
        processor->data.callback(processor);
    }
} // end of _atfp_removefile__unlink_done_cb


static  void  _atfp_removefile__scandir_done_cb(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t num_files = asa_dst->op.scandir.fileinfo.size;
        if(num_files > 0) {
            int err = atfp_scandir_load_fileinfo (asa_dst, err_info);
            if(!err)
                _atfp_removefile__unlink_start (asa_dst, err_info, _atfp_removefile__unlink_done_cb);
        } else {
            _atfp_hls_rmdir(asa_dst, err_info);
        }
    } else {
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to scan folder path for removing files"));
    }
    if(json_object_size(err_info) > 0) {
        _atfp_removefile__dealloc(asa_dst);
        processor->data.callback(processor);
    }
} // end of _atfp_removefile__scandir_done_cb


void  atfp_hls__remove_file(atfp_t *processor, const char *status)
{
    asa_op_base_cfg_t *asa_dst = processor ->data.storage.handle;
    json_t *err_info = processor->data.error;
    uint32_t _usr_id = processor ->data.usr_id;
    uint32_t _upld_req_id = processor ->data.upld_req_id;
    const char *version = processor->data.version;
    size_t  fullpath_sz = strlen(asa_dst->storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1 + strlen(status) + 1 + strlen(version) + 1 ;
    char fullpath[fullpath_sz];
    size_t nwrite = snprintf(&fullpath[0], fullpath_sz, "%s/%d/%08x/%s/%s",
            asa_dst->storage->base_path, _usr_id, _upld_req_id,  status, version);
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    asa_dst->op.scandir.path = strdup(&fullpath[0]);
    asa_dst->op.scandir.cb   = _atfp_removefile__scandir_done_cb;
    ASA_RES_CODE  result =  asa_dst->storage->ops.fn_scandir(asa_dst);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string(
                "[storage] failed to issue scandir operation for removing files"));
        _atfp_removefile__dealloc(asa_dst);
        processor->data.callback(processor);
    }
} // end of atfp_hls__remove_file

static  void  _atfp_hls__final_dealloc(atfp_t *processor)
{
    if(!processor)
        return;
    void (*cb)(atfp_t *) = processor->data.callback;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) > 0) {} // TODO,log for error happened
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    asaremote->deinit(asaremote);
    processor->data.version = NULL;
    DEINIT_IF_EXISTS(processor->data.error, json_decref);
    DEINIT_IF_EXISTS(processor, free);
    if(cb)
        cb(NULL);
} // end of _atfp_hls__final_dealloc

static  void  atfp_hls__asaremote_closefile_cb(asa_op_base_cfg_t *asaremote, ASA_RES_CODE result)
{
    atfp_t *processor = asaremote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if(processor->transfer.dst.flags.version_exists) {
        processor->data.error = json_object();
        processor->data.callback = _atfp_hls__final_dealloc; // TODO, ensure no extra callback from rpc consumer
        processor->transfer.dst.remove_file(processor, ATFP__DISCARDING_FOLDER_NAME);
    } else {
        _atfp_hls__final_dealloc(processor);
    }
} // end of atfp_hls__asaremote_closefile_cb

static  void  atfp_hls__asalocal_closefile_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    uint8_t  asa_remote_open = processor->transfer.dst.flags.asaremote_open != 0;
    if (asa_remote_open) {
        asaremote->op.close.cb = atfp_hls__asaremote_closefile_cb;
        result =  asaremote->storage->ops.fn_close(asaremote);
        asa_remote_open = result == ASTORAGE_RESULT_ACCEPT;
        if(result != ASTORAGE_RESULT_ACCEPT)
            atfp_hls__asaremote_closefile_cb(asaremote, ASTORAGE_RESULT_COMPLETE);
    } else {
        atfp_hls__asaremote_closefile_cb(asaremote, ASTORAGE_RESULT_COMPLETE);
    }
}

uint8_t  atfp__video_hls__deinit(atfp_t *processor)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t     *asa_dst = processor -> data.storage.handle;
    asa_op_localfs_cfg_t  *asa_local_dstdata = &hlsproc->asa_local;
    atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
    hlsproc->internal.op.avctx_deinit(hlsproc);
    char *tmp = NULL;
    // Note in this file processor, dst_path points to reference of seg_cfg->fullpath._asa_dst.data
    tmp = asa_dst->op.open.dst_path;
    if(tmp == seg_cfg->fullpath._asa_dst.data)
        asa_dst->op.open.dst_path = NULL;
    tmp = asa_local_dstdata->super.op.open.dst_path;
    if(tmp == seg_cfg->fullpath._asa_local.data)
        asa_local_dstdata->super.op.open.dst_path = NULL;
    DEINIT_IF_EXISTS(seg_cfg->fullpath._asa_dst.data, free);
    DEINIT_IF_EXISTS(seg_cfg->fullpath._asa_local.data, free);
    DEINIT_IF_EXISTS(seg_cfg->filename.prefix.data, free);
    DEINIT_IF_EXISTS(seg_cfg->filename.pattern.data, free);
    DEINIT_IF_EXISTS(seg_cfg-> rdy_list.entries, free);
    DEINIT_IF_EXISTS(processor->transfer.dst.info, json_decref);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.cb_args.entries, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.prefix, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.origin, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.curr_parent, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.open.dst_path, free);
    // ensure file-descriptors in asa-local and asa-dst are closed
    ASA_RES_CODE  asa_result;
    uint8_t  asa_local_open  = processor->transfer.dst.flags.asalocal_open  != 0;
    uint8_t  asa_remote_open = processor->transfer.dst.flags.asaremote_open != 0;
    if (asa_local_open) {
        asa_local_dstdata->super.op.close.cb = atfp_hls__asalocal_closefile_cb;
        asa_result = app_storage_localfs_close(&asa_local_dstdata->super);
        if(asa_result != ASTORAGE_RESULT_ACCEPT) {
            asa_local_open = 0;
            atfp_hls__asalocal_closefile_cb(&asa_local_dstdata->super, ASTORAGE_RESULT_COMPLETE);
        }
    } else {
        atfp_hls__asalocal_closefile_cb(&asa_local_dstdata->super, ASTORAGE_RESULT_COMPLETE);
    }
    return  asa_remote_open || asa_local_open;
} // end of atfp__video_hls__deinit
#undef  DEINIT_IF_EXISTS

