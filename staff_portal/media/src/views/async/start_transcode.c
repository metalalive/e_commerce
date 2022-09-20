#include <jansson.h>
#include <magic.h>

#include "app_cfg.h"
#include "views.h"
#include "rpc/core.h"
#include "transcoder/rpc.h"
#include "storage/cfg_parser.h"

#define   SRC_FILECHUNK_BEGINNING_READ_SZ  0x40

static void  api_rpc_transcode__atfp_src_processing_cb (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_op_base_cfg_t *asa_dst = NULL;
    arpc_receipt_t  *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    atfp_asa_map_t   *_map = asa_src->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    uint8_t has_err = json_object_size(err_info) > 0;
    if(has_err) {
        app_rpc_task_send_reply(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    } else {
        atfp_asa_map_reset_dst_iteration(_map);
        while(!has_err && (asa_dst = atfp_asa_map_iterate_destination(_map))) {
            processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            uint8_t done_dst = processor->ops->has_done_processing(processor);
            if(done_dst) { continue; }
            processor->ops->processing(processor);
            has_err = json_object_size(err_info) > 0;
            if(!has_err)
                atfp_asa_map_dst_start_working(_map, asa_dst);
        }
        if(atfp_asa_map_all_dst_stopped(_map)) {
            if(has_err) {
                app_rpc_task_send_reply(receipt, err_info);
                api_rpc_transcoding__storagemap_deinit(_map);
            } else {
                api_rpc_transcode__finalize(_map);
            }
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
    asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
    if(!has_err) {
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
            if(atfp_asa_map_all_dst_stopped(_map)) { // send return message to rpc-reply queue
                if(!has_err)
                    api_rpc_transcode__finalize(_map);
            } else {
                if(has_err)
                    has_err = 0; // postpone error handling in later event-loop cycles
            }
        }
    }
    if(has_err) {
         arpc_receipt_t *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
         app_rpc_task_send_reply(receipt, err_info);
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
             app_rpc_task_send_reply(receipt, err_info);
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
         app_rpc_task_send_reply(receipt, err_info);
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
        uint32_t _usr_id = (uint32_t) json_integer_value(json_object_get(spec, "usr_id"));
        uint32_t _upld_req_id = (uint32_t) json_integer_value(json_object_get(spec, "last_upld_req"));
        asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP] = processor;
        processor->data = (atfp_data_t) {
            .error=err_info, .spec=spec, .callback=callback, .usr_id=_usr_id, .upld_req_id=_upld_req_id,
            .version=asaobj->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL],
            .storage={ .basepath=asaobj->op.mkdir.path.origin, .handle=asaobj},
        };
    }
    return processor;
} // end of api_rpc_transcode__init_file_processor


static  void  api_rpc_transcode__try_init_file_processors(asa_op_base_cfg_t *asaobj)
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
    json_t *outputs = json_object_get(spec, "outputs");
    atfp_asa_map_reset_dst_iteration(_map);
    while((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        const char *version = asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL];
        json_t *output = json_object_get(outputs, version);
        const char *ofmt_label = json_string_value(json_object_get(output, "container"));
        processor = api_rpc_transcode__init_file_processor(asa_dst, ofmt_label, api_rpc_transcode__atfp_dst_init_finish_cb);
        if(processor == NULL) {
            json_object_set_new(err_info, "transcoder", json_string("unsupported destination file format"));
            goto done;
        }
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        uint8_t version_exist = (uint8_t) asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_EXIST_FLAG];
#pragma GCC diagnostic pop
        processor->transfer.dst.flags.version_exists = version_exist;
        if(asa_dst->op.mkdir.path.prefix) {
            char *basepath = asa_dst->op.mkdir.path.origin;
            basepath[0] = 0;
            strcat(basepath, asa_dst->op.mkdir.path.prefix);
            strcat(basepath, "/");
            strcat(basepath, version);
            size_t nwrite = strlen(basepath) + 1;
            basepath[nwrite++] = 0;
            asa_dst->op.mkdir.path.prefix[0] = 0; // let next mkdir() call ignore the prefix
        } // move prefix path to origin field, append version value to it
    } // end of loop
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
        app_rpc_task_send_reply(receipt, err_info);
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
        app_result = asaobj->storage->ops.fn_read(asaobj);
        if(app_result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("failed to issue read-file operation"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open original file chunk"));
    }
    if(json_object_size(err_info) > 0) {
        atfp_asa_map_t *_map    = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
        if(--_map->app_sync_cnt == 0) {
            arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
            app_rpc_task_send_reply(receipt, err_info);
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
        app_rpc_task_send_reply(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__create_folder_common_cb


static void api_rpc_transcode__create_dst_basepath_cb (asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_asa_map_t *_map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    json_t     *err_info = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if(json_object_size(err_info) > 0) {
        // pass
    } else if(result == ASTORAGE_RESULT_COMPLETE) {
        const char *version = asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL];
        char *basepath = asa_dst->op.mkdir.path.origin;
        strcpy(asa_dst->op.mkdir.path.prefix, basepath);
        size_t nwrite = sprintf(basepath, "%s", version);
        basepath[nwrite++] = 0;
        asa_dst->op.mkdir.path.curr_parent[0] = 0x0; // reset for mkdir
        asa_dst->op.mkdir.cb =  api_rpc_transcode__create_folder_common_cb;
        // clear allow_exist flag, to make use of OS lock, and consider EEXISTS as error after mkdir()
        result = asa_dst->storage->ops.fn_mkdir(asa_dst, 0);
        if (result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("failed to issue mkdir operation to storage"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to create work folder for transcoded file"));
    }
    if(json_object_size(err_info) > 0) {
        if(--_map->app_sync_cnt == 0) {
            arpc_receipt_t *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
            app_rpc_task_send_reply(receipt, err_info);
            api_rpc_transcoding__storagemap_deinit(_map);
        } // positive `app_sync_cnt` means more running tasks haven't been completed 
    }
} // end of api_rpc_transcode__create_dst_basepath_cb


static asa_op_base_cfg_t * api_rpc_transcode__init_asa_obj (arpc_receipt_t *receipt,
        json_t *api_req, json_t *err_info, asa_cfg_t *storage, 
        uint8_t num_cb_args, uint32_t rd_buf_bytes, uint32_t wr_buf_bytes )
{
    asa_op_base_cfg_t *out = NULL;
    size_t   cb_args_tot_sz = sizeof(void *) * num_cb_args;
    size_t   asaobj_sz = storage->ops.fn_typesize();
    size_t   asaobj_tot_sz = asaobj_sz + cb_args_tot_sz + rd_buf_bytes + wr_buf_bytes;
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
    out->storage = storage;
    ptr += cb_args_tot_sz;
    out->op.read.offset = 0;
    out->op.read.dst_max_nbytes = rd_buf_bytes;
    out->op.read.dst_sz = 0;
    if(rd_buf_bytes > 0)
        out->op.read.dst = (char *)ptr;
    ptr += rd_buf_bytes;
    out->op.write.offset = 0;
    out->op.write.src_max_nbytes = wr_buf_bytes;
    out->op.write.src_sz = 0;
    if(wr_buf_bytes > 0)
        out->op.write.src = (char *)ptr;
    ptr += wr_buf_bytes;
    assert((size_t)(ptr - (char *)out) == asaobj_tot_sz);
    return out;
}  // end of api_rpc_transcode__init_asa_obj


static  __attribute__((optimize("O0"))) void  api_rpc_task_handler__start_transcode (arpc_receipt_t *receipt)
{
    json_error_t jerror = {0};
    asa_op_base_cfg_t  *asa_src = NULL, *asa_dst = NULL;
    asa_op_localfs_cfg_t  *asa_local_tmpbuf = NULL;
    ASA_RES_CODE  asa_result; 
    json_t *err_info = json_object();
    json_t *api_req = json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    atfp_asa_map_t  *asaobj_map = NULL;
    int _app_sync_cnt = 0;
    if(jerror.line >= 0 || jerror.column >= 0) {
        json_t *item = json_object();
        json_object_set_new(item, "message", json_string("invalid JSON format found in request"));
        json_object_set_new(item, "line", json_integer(jerror.line));
        json_object_set_new(item, "column", json_integer(jerror.column));
        json_object_set_new(err_info, "non-field", item);
        goto error;
    }
    const char *_metadata_db   = json_string_value(json_object_get(api_req, "metadata_db"));
    const char *src_storage_alias = json_string_value(json_object_get(api_req, "storage_alias"));
    uint32_t _usr_id = (uint32_t) json_integer_value(json_object_get(api_req, "usr_id"));
    uint32_t _upld_req_id = (uint32_t) json_integer_value(json_object_get(api_req, "last_upld_req"));
    json_t *outputs = json_object_get(api_req, "outputs");
    uint32_t num_destinations = outputs ? json_object_size(outputs): 0;
    if(_upld_req_id == 0)
        json_object_set_new(err_info, "upld_req", json_string("has to be non-zero unsigned integer"));
    if(_usr_id == 0) 
        json_object_set_new(err_info, "usr_id", json_string("has to be non-zero unsigned integer"));
    if(!_metadata_db)
        json_object_set_new(err_info, "metadata_db", json_string("required"));
    if(!src_storage_alias)
        json_object_set_new(err_info, "storage_alias", json_string("required"));
    if(!outputs || num_destinations == 0)
        json_object_set_new(err_info, "outputs", json_string("required"));
    if(json_object_size(err_info) > 0)
        goto error;
    // storage applied to both file processors is local filesystem in this app
    asa_cfg_t *src_storage = app_storage_cfg_lookup(src_storage_alias);
    asaobj_map = atfp_asa_map_init(num_destinations);
    { // instantiate asa objects
        asa_src = api_rpc_transcode__init_asa_obj (receipt, api_req, err_info, src_storage,
                 (uint8_t)NUM_USRARGS_ASA_SRC, (uint32_t)APP_ENCODED_RD_BUF_SZ, (uint32_t)0);
        asa_local_tmpbuf = (asa_op_localfs_cfg_t  *) api_rpc_transcode__init_asa_obj (receipt, api_req, err_info,
                 app_storage_cfg_lookup("localfs"), (uint8_t)NUM_USRARGS_ASA_LOCALTMP, (uint32_t)0, (uint32_t)0);
        // set event loop to each file processor. TODO: event loop field should be moved to parent type
        if(!strcmp(src_storage->alias, "localfs"))
            ((asa_op_localfs_cfg_t *)asa_src)->loop = receipt->loop; // TODO
        asa_local_tmpbuf->loop = receipt->loop;
        atfp_asa_map_set_source(asaobj_map, asa_src);
        atfp_asa_map_set_localtmp(asaobj_map, asa_local_tmpbuf);
        const char *version = NULL;
        json_t *output = NULL;
        json_object_foreach(outputs, version, output) {
            const char *dst_storage_alias = json_string_value(json_object_get(output, "storage_alias"));
            asa_cfg_t *dst_storage = app_storage_cfg_lookup(dst_storage_alias);
            if(!dst_storage) {
                 json_object_set_new(err_info, "dst_storage_alias", json_string("invalid"));
                 goto error;
            }
            uint8_t version_exist = (uint8_t) json_boolean_value(json_object_get(json_object_get(
                            output, "__internal__"), "is_update"));
            asa_dst = api_rpc_transcode__init_asa_obj (receipt, api_req, err_info, dst_storage,
                   (uint8_t)NUM_USRARGS_ASA_DST, (uint32_t)0, (uint32_t)APP_ENCODED_WR_BUF_SZ);
            if(!strcmp(dst_storage->alias, "localfs"))
                ((asa_op_localfs_cfg_t *)asa_dst)->loop = receipt->loop; // TODO
            asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL] = (void *) strdup(version);
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
            asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_EXIST_FLAG] = (void *) version_exist;
#pragma GCC diagnostic pop
            atfp_asa_map_add_destination(asaobj_map, asa_dst);
        } // end of iteration
    } { // create work folder for local temp buffer
        app_cfg_t *app_cfg = app_get_global_cfg();
        size_t path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1; // include NULL-terminated byte
        char basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x", app_cfg->tmp_buf.path,
                _usr_id, _upld_req_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_local_tmpbuf->file.file = -1;
        asa_local_tmpbuf->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local_tmpbuf->super.op.mkdir.cb = api_rpc_transcode__create_folder_common_cb;
        asa_local_tmpbuf->super.op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_local_tmpbuf->super.op.mkdir.path.curr_parent = (void *)calloc(nwrite, sizeof(char));
        asa_result = asa_local_tmpbuf->super.storage->ops.fn_mkdir(&asa_local_tmpbuf->super, 1);
        if(asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(err_info, "storage", json_string("failed to issue create-folder operation for tmp buf"));
            goto error;
        }
    } { // open source file then read first portion
        size_t path_sz = strlen(asa_src->storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
                   UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1; // assume NULL-terminated string
        char basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x", asa_src->storage->base_path,
                _usr_id, _upld_req_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_src->op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_result = atfp_open_srcfile_chunk(asa_src, asa_src->op.mkdir.path.origin, 1,
                api_rpc_transcode__open_src_first_chunk_cb );
        if(asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(err_info, "storage", json_string("failed to issue open-file operation"));
            goto error;
        }
    }
    atfp_asa_map_reset_dst_iteration(asaobj_map);
    while((asa_dst = atfp_asa_map_iterate_destination(asaobj_map))) {
        const char *version = asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL];
        size_t transcoding_fullpath_sz = strlen(asa_dst->storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1 + ATFP__MAXSZ_STATUS_FOLDER_NAME + 1 ;
        size_t version_fullpath_sz = transcoding_fullpath_sz + strlen(version) + 1;
        asa_dst->op.mkdir.path.prefix = (void *)calloc(transcoding_fullpath_sz, sizeof(char));
        asa_dst->op.mkdir.path.origin = (void *)calloc(version_fullpath_sz, sizeof(char));
        asa_dst->op.mkdir.path.curr_parent = (void *)calloc(version_fullpath_sz, sizeof(char));
        // will be used later when application moves transcoded file from temporary buffer (locally
        // stored in transcoding server) to destination storage (may be remotely stored, e.g. in cloud platform)
        size_t nwrite = snprintf(asa_dst->op.mkdir.path.origin, version_fullpath_sz, "%s/%d/%08x/%s",
                asa_dst->storage->base_path, _usr_id, _upld_req_id,  ATFP__TEMP_TRANSCODING_FOLDER_NAME);
        asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
        assert(nwrite <= transcoding_fullpath_sz);
        asa_dst->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_dst->op.mkdir.cb = api_rpc_transcode__create_dst_basepath_cb;
        asa_result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
        if (asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(err_info, "storage", json_string("failed to issue mkdir operation to storage"));
            goto error;
        }
    } // create folder for saving transcoded files in destinations
    asaobj_map->app_sync_cnt = _app_sync_cnt;
    return;
error:
    if(_app_sync_cnt > 0) { // will send error message to reply queue in next event-loop cycle
        asaobj_map->app_sync_cnt = _app_sync_cnt;
    } else {
        app_rpc_task_send_reply(receipt, err_info);
        if(asaobj_map) {
            api_rpc_transcoding__storagemap_deinit(asaobj_map);
        } else {
            if(api_req) { json_decref(api_req); }
            if(err_info) { json_decref(err_info); }
        }
    }
} // end of api_rpc_task_handler__start_transcode

