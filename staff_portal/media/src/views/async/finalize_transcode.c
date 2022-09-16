#include <jansson.h>

#include "rpc/core.h"
#include "transcoder/rpc.h"


static void api_rpc_transcoding__storage_deinit(asa_op_base_cfg_t *asaobj) {
    atfp_t  *processor = asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    // each file-processor is responsible to de-init asa object, due to the reason
    // file processor may require information provided in external asa object during de-init
    processor->data.error = NULL;
    processor->data.spec = NULL;
    processor->data.callback = NULL;
    processor->ops->deinit(processor);
}

void api_rpc_transcoding__storagemap_deinit(atfp_asa_map_t *_map) {
    if(!_map) { return; }
    // NOTE :
    // * each source file processor is responsible to de-initialize asa_src, asa_local, as well as
    //   the map object, since it could takes several event-loop cycles to complete de-initialization
    //   (for example, unlink temp files for its internal use)
    // * all the asa objects MUST NOT read values from `api_req` and `err_info` during the
    //   de-initialization 
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(_map);
    asa_op_base_cfg_t *asa_dst = NULL;
    atfp_asa_map_reset_dst_iteration(_map);
    while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        atfp_asa_map_remove_destination(_map, asa_dst);
        api_rpc_transcoding__storage_deinit(asa_dst);
    }
    if (asa_src) { // other objects shared between `asa_op_base_cfg_t` objects
        json_t *api_req  = asa_src->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        json_t *err_info = asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
        json_decref(api_req); // de-init in destinations may require to read values from here
        json_decref(err_info);
        asa_src->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST] = NULL;
        asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO] = NULL;
        api_rpc_transcoding__storage_deinit(asa_src);
    }
} // end of api_rpc_transcoding__storagemap_deinit
   

static void api_rpc_transcode__update_metadata_done(struct atfp_s *processor)
{
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_asa_map_t   *map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    atfp_asa_map_dst_stop_working(map, asa_dst);
    if(!atfp_asa_map_all_dst_stopped(map)) {
        return;
    }
    asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(map);
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
} // end of api_rpc_transcode__update_metadata_done

void  api_rpc_transcode__finalize (atfp_asa_map_t *map)
{
    uint8_t has_err = 0;
    asa_op_base_cfg_t *asa_dst = NULL, *asa_src = atfp_asa_map_get_source(map);
    arpc_receipt_t  *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    json_t         *err_info = asa_src->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    atfp_asa_map_reset_dst_iteration(map);
    while(!has_err && (asa_dst = atfp_asa_map_iterate_destination(map))) {
        atfp_t *fp_dst = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
        fp_dst->data.callback = api_rpc_transcode__update_metadata_done;
        fp_dst->transfer.dst.update_metadata(fp_dst, receipt->loop);
        has_err = json_object_size(err_info) > 0;
        if(!has_err)
            atfp_asa_map_dst_start_working(map, asa_dst);
    } // TODO, solve potential n + 1 problems
    if(atfp_asa_map_all_dst_stopped(map) && has_err) {
        app_rpc_task_send_reply(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(map);
    }
} // end of api_rpc_transcode__finalize
