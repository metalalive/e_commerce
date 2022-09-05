#include <jansson.h>

#include "rpc/core.h"
#include "transcoder/rpc.h"


static void api_rpc_transcoding__storage_deinit(asa_op_base_cfg_t *asaobj) {
    atfp_t  *processor = asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    // each file-processor is responsible to de-init asa object, due to the reason
    // file processor may require information provided in external asa object during de-init
    processor->ops->deinit(processor);
}

static void  api_rpc__asalocal_closefile_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{   // TODO, clean up all files in created local storage since they are for temporary use
    if(asaobj->op.mkdir.path.origin) {
        free(asaobj->op.mkdir.path.origin);
        asaobj->op.mkdir.path.origin = NULL;
    }
    if(asaobj->op.mkdir.path.curr_parent) {
        free(asaobj->op.mkdir.path.curr_parent);
        asaobj->op.mkdir.path.curr_parent = NULL;
    }
    if(asaobj->op.open.dst_path) {
        free(asaobj->op.open.dst_path);
        asaobj->op.open.dst_path = NULL;
    }
}

void api_rpc_transcoding__storagemap_deinit(atfp_asa_map_t *_map) {
    if(!_map) { return; }
    // TODO, this function has to be idempotent, to make sure all connected storage
    //  handles are de-initialized before de-initializing the map 
    asa_op_localfs_cfg_t *asa_local =  atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(_map);
    asa_op_base_cfg_t *asa_dst = NULL;
    if (asa_src) { // other objects shared between `asa_op_base_cfg_t` objects
        json_t *api_req  = asa_src->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        json_t *err_info = asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
        json_decref(api_req);
        json_decref(err_info);
        asa_src->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST] = NULL;
        asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO] = NULL;
        atfp_asa_map_set_source(_map, NULL);
        api_rpc_transcoding__storage_deinit(asa_src);
    }
    if(asa_local) {
        atfp_asa_map_set_localtmp(_map, NULL);
        if(asa_local->file.file >= 0) {
            asa_local->super.op.close.cb =  api_rpc__asalocal_closefile_cb;
            app_storage_localfs_close(&asa_local->super);
        } else {
            api_rpc__asalocal_closefile_cb(&asa_local->super, ASTORAGE_RESULT_COMPLETE);
        }
    }
    atfp_asa_map_reset_dst_iteration(_map);
    while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        atfp_asa_map_remove_destination(_map, asa_dst);
        api_rpc_transcoding__storage_deinit(asa_dst);
    }
    atfp_asa_map_deinit(_map);
} // end of api_rpc_transcoding__storagemap_deinit


void  api_rpc_transcode__finalize (atfp_asa_map_t *map)
{ // TODO, write metadata of transcoded files to database, swtich folders in storage
    asa_op_base_cfg_t *asa_dst = NULL, *asa_src = atfp_asa_map_get_source(map);
    arpc_receipt_t  *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    json_t *err_info = asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if (json_object_size(err_info) == 0) { // reuse error info object, to construct reply message
        json_t *api_req = asa_src->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        json_t *resource_id_item = json_object_get(api_req, "resource_id");
        json_t *usr_id_item      = json_object_get(api_req, "usr_id");
        json_t *upld_req_item    = json_object_get(api_req, "last_upld_req");
        json_object_set(err_info, "resource_id", resource_id_item);
        json_object_set(err_info, "usr_id", usr_id_item);
        json_object_set(err_info, "last_upld_req", upld_req_item);
        json_t *transcoded_info_list = json_object();
        json_object_set_new(err_info, "info", transcoded_info_list);
        atfp_asa_map_reset_dst_iteration(map);
        while((asa_dst = atfp_asa_map_iterate_destination(map))) {
            atfp_t *fp_dst = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            json_object_set(transcoded_info_list, fp_dst->data.version, fp_dst->transfer.dst.info);
        }  // e.g. size and checksum of each file ...etc.
    } // transcoded successfully
    app_rpc_task_send_reply(receipt, err_info);
    api_rpc_transcoding__storagemap_deinit(map);
} // end of api_rpc_transcode__finalize

