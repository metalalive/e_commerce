#include <assert.h>
#include <string.h>

#include "views.h"
#include "transcoder/video/hls.h"

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static  void  _atfp_hls__final_dealloc(atfp_t *processor)
{
    if(!processor)
        return;
    void (*cb)(atfp_t *) = processor->data.callback;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) > 0)
        fprintf(stderr, "[transcoder][hls][deinit] line:%d, version:%s, error on removing"
                " transcoding version folder\n",  __LINE__, processor->data.version );
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    asaremote->deinit(asaremote);
    processor->data.version = NULL; // app caller should dealloc it
    DEINIT_IF_EXISTS(processor->data.error, json_decref);
    DEINIT_IF_EXISTS(processor, free);
    if(cb)
        cb(NULL);
} // end of _atfp_hls__final_dealloc

static  void  _atfp_hls__remove_discarded_version_done(atfp_t *processor)
{
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) > 0) {
        fprintf(stderr, "[transcoder][hls][deinit] line:%d, error on discarding "
                "version folder\n", __LINE__ );
        json_object_clear(err_info);
    }  // TODO, ensure no extra callback from rpc consumer
    if(processor->transfer.transcoded_dst.flags.version_created) {
        processor->data.callback = _atfp_hls__final_dealloc;
        processor->transfer.transcoded_dst.remove_file(processor, ATFP__TEMP_TRANSCODING_FOLDER_NAME);
    } else {
        _atfp_hls__final_dealloc(processor);
    }
} // end of  _atfp_hls__remove_discarded_version_done

static  void  atfp_hls__asaremote_closefile_cb(asa_op_base_cfg_t *asaremote, ASA_RES_CODE result)
{
    atfp_t *processor = asaremote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if(processor->transfer.transcoded_dst.flags.version_exists) {
        processor->data.error = json_object();
        processor->data.callback = _atfp_hls__remove_discarded_version_done;
        processor->transfer.transcoded_dst.remove_file(processor, ATFP__DISCARDING_FOLDER_NAME);
    } else {
        _atfp_hls__remove_discarded_version_done(processor);
    }
}

static  void  atfp_hls__asalocal_closefile_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    asa_op_base_cfg_t *asaremote = processor ->data.storage.handle;
    uint8_t  asa_remote_open = processor->transfer.transcoded_dst.flags.asaremote_open != 0;
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

uint8_t  atfp__video_hls__deinit_transcode(atfp_t *processor)
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
    DEINIT_IF_EXISTS(processor->transfer.transcoded_dst.info, json_decref);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.cb_args.entries, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.prefix, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.origin, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.mkdir.path.curr_parent, free);
    DEINIT_IF_EXISTS(asa_local_dstdata->super.op.open.dst_path, free);
    // ensure file-descriptors in asa-local and asa-dst are closed
    ASA_RES_CODE  asa_result;
    uint8_t  asa_local_open  = processor->transfer.transcoded_dst.flags.asalocal_open  != 0;
    uint8_t  asa_remote_open = processor->transfer.transcoded_dst.flags.asaremote_open != 0;
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
} // end of atfp__video_hls__deinit_transcode
#undef  DEINIT_IF_EXISTS

