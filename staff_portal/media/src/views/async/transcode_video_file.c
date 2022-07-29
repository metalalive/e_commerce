#include <jansson.h>
#include <magic.h>

#include "app_cfg.h"
#include "views.h"
#include "rpc/core.h"
#include "storage/localfs.h"
#include "transcoder/file_processor.h"

#define   APP_ENCODED_RD_BUF_SZ       2048
#define   SRC_FILECHUNK_BEGINNING_READ_SZ  0x40

#define   ASA_USRARG_INDEX__AFTP         ATFP_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__ASAOBJ_MAP   ASAMAP_INDEX__IN_ASA_USRARG
// for all file processors of each API request
#define   ASA_USRARG_INDEX__RPC_RECEIPT  (ASA_USRARG_INDEX__ASAOBJ_MAP + 1)
#define   ASA_USRARG_INDEX__API_REQUEST  (ASA_USRARG_INDEX__ASAOBJ_MAP + 2)
#define   ASA_USRARG_INDEX__ERROR_INFO      (ASA_USRARG_INDEX__ASAOBJ_MAP + 3)
#define   ASA_USRARG_INDEX__STORAGE_CONFIG  (ASA_USRARG_INDEX__ASAOBJ_MAP + 4)

#define   NUM_USRARGS_ASA_LOCALTMP  (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)
#define   NUM_USRARGS_ASA_SRC       (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)
#define   NUM_USRARGS_ASA_DST       (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)


static  __attribute__((optimize("O0"))) void _rpc_task_send_reply (arpc_receipt_t *receipt, json_t *res_body)
{
#define  MAX_BYTES_RESP_BODY  256
    size_t nwrite = 0;
    size_t nrequired = MAX_BYTES_RESP_BODY;
    char *body_raw = malloc(sizeof(char) * nrequired);
    while(1) { // optionally extend serialized json once the required size exceeds the one declared
        nwrite = json_dumpb((const json_t *)res_body, body_raw, nrequired, JSON_COMPACT);
        assert(nwrite > 0);
        if(nrequired == nwrite) {
            nrequired += MAX_BYTES_RESP_BODY;
            body_raw = realloc(body_raw, sizeof(char) * nrequired);
        } else { break; }
    }
    receipt->return_fn(receipt, body_raw, nwrite);
    free(body_raw);
#undef   MAX_BYTES_RESP_BODY
} // end of _rpc_task_send_reply


static void api_rpc_transcoding__storage_deinit(asa_op_base_cfg_t *asaobj) {
    atfp_t  *processor = asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    if(processor) {
        processor->ops->deinit(processor);
    }
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
    free(asaobj);
}

static void api_rpc_transcoding__storagemap_deinit(atfp_asa_map_t *_map) {
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
        api_rpc_transcoding__storage_deinit(&asa_local->super);
    }
    atfp_asa_map_reset_dst_iteration(_map);
    while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        atfp_asa_map_remove_destination(_map, asa_dst);
        api_rpc_transcoding__storage_deinit(asa_dst);
    }
    atfp_asa_map_deinit(_map);
} // end of api_rpc_transcoding__storagemap_deinit


static  __attribute__((optimize("O0"))) void  api_rpc_transcode__finalize (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *cfg = processor->data.storage.handle;
    arpc_receipt_t  *receipt = cfg->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    atfp_asa_map_t  *_map = cfg->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    if (json_object_size(err_info) == 0) {
        json_t *api_req = cfg->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        json_t *resource_id_item = json_object_get(api_req, "resource_id");
        json_t *version_item = json_object_get(api_req, "version");
        json_t *usr_id_item  = json_object_get(api_req, "usr_id");
        json_t *upld_req_item = json_object_get(api_req, "last_upld_req");
        json_object_set(err_info, "resource_id", resource_id_item);
        json_object_set(err_info, "version", version_item);
        json_object_set(err_info, "usr_id", usr_id_item);
        json_object_set(err_info, "last_upld_req", upld_req_item);
        json_object_set(err_info, "info", processor->transcoded_info); // e.g. size and checksum of each file ...etc.
    } // transcoded successfully
    _rpc_task_send_reply(receipt, err_info);
    api_rpc_transcoding__storagemap_deinit(_map);
} // end of api_rpc_transcode__finalize


static void  api_rpc_transcode__atfp_src_processing_cb (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_op_base_cfg_t *asa_dst = NULL;
    atfp_asa_map_t   *_map = asa_src->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    uint8_t has_err = json_object_size(err_info) > 0;
    if(has_err) {
        api_rpc_transcode__finalize(processor);
    } else {
        atfp_asa_map_reset_dst_iteration(_map);
        while(!has_err && (asa_dst = atfp_asa_map_iterate_destination(_map))) {
            processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            processor->ops->processing(processor);
            has_err = json_object_size(err_info) > 0;
            if(!has_err) {
                atfp_asa_map_dst_start_working(_map, asa_dst);
            }
        }
        if(has_err && atfp_asa_map_all_dst_stopped(_map)) {
             arpc_receipt_t *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
             _rpc_task_send_reply(receipt, err_info);
             api_rpc_transcoding__storagemap_deinit(_map);
        }
    }
} // end of api_rpc_transcode__atfp_src_processing_cb


static void  api_rpc_transcode__atfp_dst_processing_cb (atfp_t  *processor)
{
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_asa_map_t   *_map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    atfp_asa_map_dst_stop_working(_map, asa_dst);
    if(!atfp_asa_map_all_dst_stopped(_map)) {
        return;
    } // only the last destination storage handle can proceed
    json_t *err_info = processor->data.error;
    uint8_t has_err = json_object_size(err_info) > 0;
    if(!has_err) {
        asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
        processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
        uint8_t done_src = processor->ops->has_done_processing(processor);
        if(!done_src) { // switch to source file processor
            processor->ops->processing(processor);
            has_err = json_object_size(err_info) > 0;
        } else { // switch to the destination file-processors which haven't done yet
            atfp_asa_map_reset_dst_iteration(_map);
            while(!has_err && (asa_dst = atfp_asa_map_iterate_destination(_map))) {
                processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
                uint8_t done_dst = processor->ops->has_done_processing(processor);
                if(!done_dst) {
                    processor->ops->processing(processor);
                    has_err = json_object_size(err_info) > 0;
                    if(!has_err)
                        atfp_asa_map_dst_start_working(_map, asa_dst);
                }
            } // end of while loop
            if(has_err && !atfp_asa_map_all_dst_stopped(_map))
                has_err = 0; // will report error in next event-loop iteration
        }
    }
    if(has_err) {
         arpc_receipt_t *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
         _rpc_task_send_reply(receipt, err_info);
         api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__atfp_dst_processing_cb


static  void api_rpc_transcode__atfp_dst_init_finish_cb (atfp_t  *processor)
{
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_asa_map_t   *_map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    atfp_asa_map_dst_stop_working(_map, asa_dst);
    if(atfp_asa_map_all_dst_stopped(_map)) {
        json_t *err_info = processor->data.error;
        atfp_asa_map_reset_dst_iteration(_map);
        while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
            processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            processor->data.callback = api_rpc_transcode__atfp_dst_processing_cb;
        }
        if(json_object_size(err_info) == 0) { // switch to source file processor
            asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
            processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            processor->data.callback = api_rpc_transcode__atfp_src_processing_cb;
            processor->ops->processing(processor);
        }
        if (json_object_size(err_info) > 0) {
             arpc_receipt_t *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
             _rpc_task_send_reply(receipt, err_info);
             api_rpc_transcoding__storagemap_deinit(_map);
        }
    } // only the last destination storage handle can proceed
} // end of api_rpc_transcode__atfp_dst_init_finish_cb


static  void api_rpc_transcode__atfp_src_init_finish_cb (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src = processor->data .storage.handle;
    asa_op_base_cfg_t *asa_dst = NULL;
    atfp_asa_map_t *_map    = asa_src->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    uint8_t has_err = json_object_size(err_info) > 0;
    atfp_asa_map_reset_dst_iteration(_map);
    while(!has_err && (asa_dst = atfp_asa_map_iterate_destination(_map))) {
        processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
        processor->ops->init(processor); // internally it may add error message to err_info
        has_err = json_object_size(err_info) > 0;
        if(!has_err) {
            atfp_asa_map_dst_start_working(_map, asa_dst);
        }
    }
    if(has_err && atfp_asa_map_all_dst_stopped(_map)) {
         arpc_receipt_t *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
         _rpc_task_send_reply(receipt, err_info);
         api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__atfp_src_init_finish_cb


static atfp_t * api_rpc_transcode__init_file_processor (
        asa_op_base_cfg_t *asaobj, const char *label, void (*callback)(struct atfp_s *)  )
{
    atfp_t *processor = app_transcoder_file_processor(label);
    if(processor) {
        json_t  *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
        json_t  *spec     = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP] = processor;
        processor->data = (atfp_data_t) {
            .error=err_info, .spec=spec, .callback=callback,
            .storage={ .basepath=asaobj->op.mkdir.path.origin, .handle=asaobj,
                .config=asaobj->cb_args.entries[ASA_USRARG_INDEX__STORAGE_CONFIG] },
        };
    }
    return processor;
} // end of api_rpc_transcode__init_file_processor


static __attribute__((optimize("O0"))) void  api_rpc_transcode__try_init_file_processors(asa_op_base_cfg_t *asaobj)
{
    json_t  *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    json_t  *spec     = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    atfp_t *processor = NULL;
    asa_op_base_cfg_t   *asa_src = atfp_asa_map_get_source(_map);
    asa_op_base_cfg_t   *asa_dst = NULL;
    magic_t  m = magic_open(MAGIC_MIME_TYPE); // check magic bytes of the file
    if(magic_load(m, NULL) == 0) {
        const char *mimetype = magic_buffer(m, (const void *)asa_src->op.read.dst, SRC_FILECHUNK_BEGINNING_READ_SZ);
        processor = api_rpc_transcode__init_file_processor(asa_src, mimetype, api_rpc_transcode__atfp_src_init_finish_cb);
        if(processor == NULL) {
            json_object_set_new(err_info, "transcoder", json_string("unsupported source file format"));
            goto done;
        }
    } else {
        json_object_set_new(err_info, "transcoder", json_string("failed to load MIME-type database"));
        goto done;
    }
    const char *ofmt_label = json_string_value(json_object_get(spec, "container"));
    atfp_asa_map_reset_dst_iteration(_map);
    while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        processor = api_rpc_transcode__init_file_processor(asa_dst, ofmt_label, api_rpc_transcode__atfp_dst_init_finish_cb);
        if(processor == NULL) {
            json_object_set_new(err_info, "transcoder", json_string("unsupported destination file format"));
            goto done;
        }
    }
    processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    processor->ops->init(processor); // internally it may add error message to err_info
done:
    if(m)
        magic_close(m);
} // end of api_rpc_transcode__try_init_file_processors


static void api_rpc_transcode__src_first_chunk_read_cb (
        asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result, size_t nread)
{
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    json_t     *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    _map->app_sync_cnt -= 1;
    if(json_object_size(err_info) > 0) {
        // pass
    } else if(app_result == ASTORAGE_RESULT_COMPLETE && nread == SRC_FILECHUNK_BEGINNING_READ_SZ) {
        if(_map->app_sync_cnt == 0)
            api_rpc_transcode__try_init_file_processors(asaobj);
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to read begining portion of the first file chunk"));
    }
    if(_map->app_sync_cnt == 0 && json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
        _rpc_task_send_reply(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__src_first_chunk_read_cb


static void api_rpc_transcode__open_src_first_chunk_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result)
{ // read first few bytes,
    json_t  *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if(json_object_size(err_info) > 0) {
        // pass
    } else if(app_result == ASTORAGE_RESULT_COMPLETE) {
        asaobj->op.read.cb = api_rpc_transcode__src_first_chunk_read_cb;
        asaobj->op.read.dst_sz = SRC_FILECHUNK_BEGINNING_READ_SZ;
        asa_cfg_t *storage = asaobj->cb_args.entries[ASA_USRARG_INDEX__STORAGE_CONFIG];
        app_result = storage->ops.fn_read(asaobj);
        if(app_result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("failed to issue read-file operation"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open original file chunk"));
    }
    if(json_object_size(err_info) > 0) {
        atfp_asa_map_t *_map    = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
        if(--_map->app_sync_cnt == 0) {
            arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
            _rpc_task_send_reply(receipt, err_info);
            api_rpc_transcoding__storagemap_deinit(_map);
        }
    } // TODO, figure out how to solve the problem if error happens to both event callbacks.
} // end of api_rpc_transcode__open_src_first_chunk_cb


static void api_rpc_transcode__create_folder_common_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result)
{
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    json_t     *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    _map->app_sync_cnt -= 1;
    if(json_object_size(err_info) > 0) {
        // pass
    } else if(app_result == ASTORAGE_RESULT_COMPLETE) {
        if(_map->app_sync_cnt == 0)
            api_rpc_transcode__try_init_file_processors(asaobj);
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to create work folder for transcoded file"));
    }
    if(_map->app_sync_cnt == 0 && json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
        _rpc_task_send_reply(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__create_folder_common_cb


static asa_op_base_cfg_t * api_rpc_transcode__init_asa_obj (arpc_receipt_t *receipt,
        json_t *api_req, json_t *err_info, asa_cfg_t *storage, size_t asaobj_sz, 
        uint8_t num_cb_args, uint32_t rd_buf_bytes )
{
    asa_op_base_cfg_t *out = NULL;
    size_t   cb_args_tot_sz = sizeof(void *) * num_cb_args;
    size_t   asaobj_tot_sz = asaobj_sz + cb_args_tot_sz + rd_buf_bytes;
    out = calloc(1, asaobj_tot_sz);
    char *ptr = (char *)out + asaobj_sz;
    out->cb_args.size = num_cb_args;
    out->cb_args.entries = (void **) ptr;
    // each storage handle connects to its own file processor, it is one-to-one relationship
    out->cb_args.entries[ASA_USRARG_INDEX__AFTP] = NULL;
    // all storage handles share the same following objects
    out->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP] = NULL;
    out->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT] = (void *)receipt;
    out->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST] = (void *)api_req;
    out->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO] = (void *)err_info;
    out->cb_args.entries[ASA_USRARG_INDEX__STORAGE_CONFIG] = (void *)storage; // global config object for  storage
    ptr += cb_args_tot_sz;
    out->op.read.offset = 0;
    out->op.read.dst_max_nbytes = rd_buf_bytes;
    out->op.read.dst_sz = 0;
    if(rd_buf_bytes > 0) {
        out->op.read.dst = (char *)ptr;
    }
    ptr += rd_buf_bytes;
    assert((size_t)(ptr - (char *)out) == asaobj_tot_sz);
    return out;
}  // end of api_rpc_transcode__init_asa_obj


static  __attribute__((optimize("O0"))) void  api_transcode_video_file__rpc_task_handler (arpc_receipt_t *receipt)
{
    json_error_t jerror = {0};
    asa_op_base_cfg_t  *asa_src = NULL, *asa_dst = NULL;
    asa_op_localfs_cfg_t  *asa_local_tmpbuf = NULL;
    ASA_RES_CODE  asa_result; 
    json_t *err_info = json_object();
    json_t *api_req = json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    atfp_asa_map_t  *asaobj_map = NULL;
    if(jerror.line >= 0 || jerror.column >= 0) {
        json_t *item = json_object();
        json_object_set_new(item, "message", json_string("invalid JSON format found in request"));
        json_object_set_new(item, "line", json_integer(jerror.line));
        json_object_set_new(item, "column", json_integer(jerror.column));
        json_object_set_new(err_info, "non-field", item);
        goto error;
    }
    const char *version = json_string_value(json_object_get(api_req, "version"));
    uint32_t usr_id = (uint32_t) json_integer_value(json_object_get(api_req, "usr_id"));
    uint32_t last_upld_req = (uint32_t) json_integer_value(json_object_get(api_req, "last_upld_req"));
    if(last_upld_req == 0)
        json_object_set_new(err_info, "upld_req", json_string("has to be non-zero unsigned integer"));
    if(usr_id == 0) 
        json_object_set_new(err_info, "usr_id", json_string("has to be non-zero unsigned integer"));
    if(!version)
        json_object_set_new(err_info, "upld_req", json_string("required"));
    if(json_object_size(err_info) > 0)
        goto error;
    // storage applied to both file processors is local filesystem in this app
    app_cfg_t *app_cfg = app_get_global_cfg();
    asa_cfg_t *storage = &app_cfg->storages.entries[0];
#define  NUM_DESTINATIONS  1
    asaobj_map = atfp_asa_map_init(NUM_DESTINATIONS); // TODO, will eexpand number of destination if required
#undef   NUM_DESTINATIONS
    asa_src = api_rpc_transcode__init_asa_obj (receipt, api_req, err_info, storage,
            sizeof(asa_op_localfs_cfg_t), (uint8_t)NUM_USRARGS_ASA_SRC, (uint32_t)APP_ENCODED_RD_BUF_SZ);
    asa_dst = api_rpc_transcode__init_asa_obj (receipt, api_req, err_info, storage,
            sizeof(asa_op_localfs_cfg_t), (uint8_t)NUM_USRARGS_ASA_DST, (uint32_t)0);
    asa_local_tmpbuf = (asa_op_localfs_cfg_t  *) api_rpc_transcode__init_asa_obj (receipt, api_req,
            err_info, storage,  sizeof(asa_op_localfs_cfg_t), (uint8_t)NUM_USRARGS_ASA_LOCALTMP, (uint32_t)0);
    {
        ((asa_op_localfs_cfg_t *)asa_src)->loop = receipt->loop;
        ((asa_op_localfs_cfg_t *)asa_dst)->loop = receipt->loop;
        asa_local_tmpbuf->loop = receipt->loop;
        atfp_asa_map_set_source(asaobj_map, asa_src);
        atfp_asa_map_set_localtmp(asaobj_map, asa_local_tmpbuf);
        atfp_asa_map_add_destination(asaobj_map, asa_dst);
    } { // create work folder for local temp buffer
        size_t path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(last_upld_req) + 1; // include NULL-terminated byte
        char basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x", app_cfg->tmp_buf.path,
                usr_id, last_upld_req);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_local_tmpbuf->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local_tmpbuf->super.op.mkdir.cb = api_rpc_transcode__create_folder_common_cb;
        asa_local_tmpbuf->super.op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_local_tmpbuf->super.op.mkdir.path.curr_parent = (void *)calloc(nwrite, sizeof(char));
        asa_result = app_storage_localfs_mkdir(&asa_local_tmpbuf->super);
        if(asa_result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("failed to issue create-folder operation for tmp buf"));
            goto error;
        }
    } { // open source file then read first portion
        size_t path_sz = strlen(storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
                   UPLOAD_INT2HEX_SIZE(last_upld_req) + 1; // assume NULL-terminated string
        char basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x", storage->base_path,
                usr_id, last_upld_req);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_src->op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_result = atfp_open_srcfile_chunk( asa_src, storage, asa_src->op.mkdir.path.origin,
            1, api_rpc_transcode__open_src_first_chunk_cb );
        if(asa_result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("failed to issue open-file operation"));
            goto error;
        }
    } { // create folder for saving transcoded files in destination
        size_t path_sz = strlen(storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
                   UPLOAD_INT2HEX_SIZE(last_upld_req) + 1 + strlen(ATFP_TEMP_TRANSCODING_FOLDER_NAME)
                   + 1 + strlen(version) + 1;
        // will be used later when application moves transcoded file from temporary buffer (locally
        // stored in transcoding server) to destination storage (may be remotely stored, e.g. in cloud platform)
        char basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x/%s/%s", storage->base_path,
                     usr_id, last_upld_req,  ATFP_TEMP_TRANSCODING_FOLDER_NAME, version);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_dst->op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_dst->op.mkdir.path.curr_parent = (void *)calloc(nwrite, sizeof(char));
        asa_dst->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_dst->op.mkdir.cb = api_rpc_transcode__create_folder_common_cb;
        asa_result = storage->ops.fn_mkdir(asa_dst);
        if (asa_result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(err_info, "storage", json_string("failed to issue mkdir operation to storage"));
            goto error;
        }
    } // end of storage object setup
    asaobj_map->app_sync_cnt = 3;
    return;
error:
    _rpc_task_send_reply(receipt, err_info);
    if(asaobj_map) {
        api_rpc_transcoding__storagemap_deinit(asaobj_map);
    } else {
        if(api_req) { json_decref(api_req); }
        if(err_info) { json_decref(err_info); }
    }
} // end of api_transcode_video_file__rpc_task_handler

