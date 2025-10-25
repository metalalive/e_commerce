#include <jansson.h>
#include <magic.h>

#include "app_cfg.h"
#include "api/setup.h"
#include "rpc/core.h"
#include "transcoder/rpc.h"
#include "storage/cfg_parser.h"

#define SRC_FILECHUNK_BEGINNING_READ_SZ 0x40

static void _api_rpc__atfp_dst_process_done_cb(atfp_asa_map_t *, json_t *, arpc_receipt_t *);

// switch to the destination file-processors which haven't done yet
static void _rpc_atfp__do_dst_processing(atfp_asa_map_t *_map, json_t *err_info, arpc_receipt_t *receipt) {
    uint8_t            has_err = 0, _num_dst_done = 0;
    asa_op_base_cfg_t *_asa_dst = NULL;
    atfp_t            *_processor = NULL;
    json_incref(err_info);
    atfp_asa_map_reset_dst_iteration(_map);
    while (!has_err && (_asa_dst = atfp_asa_map_iterate_destination(_map))) {
        _processor = _asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
        uint8_t done_dst = _processor->ops->has_done_processing(_processor);
        if (done_dst) {
            _num_dst_done += 1;
        } else {
            _processor->ops->processing(_processor);
            has_err = json_object_size(err_info) > 0;
            if (!has_err && _processor->op_async_done.processing)
                atfp_asa_map_dst_start_working(_map, _asa_dst);
        }
    } // end of loop
    json_decref(err_info);
    if (atfp_asa_map_all_dst_stopped(_map)) {
        if (json_object_size(err_info) == 0) {
            if (_num_dst_done == _map->dst.size) {
                api_rpc_transcode__finalize(_map);
            } else {
                _api_rpc__atfp_dst_process_done_cb(_map, err_info, receipt);
            }
        } else if (err_info->refcount == 1) {
            API_RPC__SEND_ERROR_REPLY(receipt, err_info);
            api_rpc_transcoding__storagemap_deinit(_map);
        } // avoid redundant de-init in potential recursive calls
    }
} // end of  _rpc_atfp__do_dst_processing

static void
_api_rpc__atfp_dst_process_done_cb(atfp_asa_map_t *_map, json_t *err_info, arpc_receipt_t *receipt) {
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(_map);
    atfp_t            *processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    uint8_t            done_src = processor->ops->has_done_processing(processor);
    if (!done_src) { // switch to source file processor
        json_incref(err_info);
        processor->ops->processing(processor);
        json_decref(err_info);
        if (json_object_size(err_info) > 0 && err_info->refcount == 1) {
            API_RPC__SEND_ERROR_REPLY(receipt, err_info);
            api_rpc_transcoding__storagemap_deinit(_map);
        } // avoid redundant de-init in potenial recursive calls
    } else {
        // Beware of duplicate reply messages are sent due to (1) recursive calls to processing
        // function of multiple destinations, and (2) error returned multiple times by these
        // processing function
        _rpc_atfp__do_dst_processing(_map, err_info, receipt);
    }
} // end of _api_rpc__atfp_dst_process_done_cb

static void api_rpc_atfp__dst_async_process_done_cb(atfp_t *processor) {
    if (!processor->op_async_done.processing)
        return;
    json_t            *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_asa_map_t    *_map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    arpc_receipt_t    *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    atfp_asa_map_dst_stop_working(_map, asa_dst);
    if (!atfp_asa_map_all_dst_stopped(_map))
        return; // only the last destination storage handle can proceed
    if (json_object_size(err_info) == 0) {
        _api_rpc__atfp_dst_process_done_cb(_map, err_info, receipt);
    } else {
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of  api_rpc_atfp__dst_async_process_done_cb

static void api_rpc_atfp__src_processing_cb(atfp_t *processor) {
    json_t            *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    atfp_asa_map_t    *_map = asa_src->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    arpc_receipt_t    *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    if (json_object_size(err_info) == 0) {
        _rpc_atfp__do_dst_processing(_map, err_info, receipt);
    } else if (processor->op_async_done.processing) {
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    } // TODO, avoid double-deinit error
} // end of api_rpc_atfp__src_processing_cb

static void _api_rpc__atfp_dst_init_done_cb(atfp_asa_map_t *_map, json_t *err_info, arpc_receipt_t *receipt) {
#if 1 // switch to source file processor
    atfp_t            *processor = NULL;
    asa_op_base_cfg_t *asa_dst = NULL;
    atfp_asa_map_reset_dst_iteration(_map);
    while ((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
        processor->data.callback = api_rpc_atfp__dst_async_process_done_cb;
    }
    json_incref(err_info);
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(_map);
    processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    processor->data.callback = api_rpc_atfp__src_processing_cb;
    processor->ops->processing(processor);
    json_decref(err_info);
    if (json_object_size(err_info) > 0 && err_info->refcount == 1) {
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    } // avoid redundant de-init in potential recursive calls
#else
    json_object_set_new(
        err_info, "__dev__", json_string("assertion for memory chekcing, after init atfp dst")
    );
#endif
} // end of _api_rpc__atfp_dst_init_done_cb

static void api_rpc_atfp__dst_init_async_done_cb(atfp_t *processor) {
    if (!processor->op_async_done.init)
        return;
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    atfp_asa_map_t    *_map = asa_dst->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    arpc_receipt_t    *receipt = asa_dst->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
    json_t            *err_info = processor->data.error;
    atfp_asa_map_dst_stop_working(_map, asa_dst);
    if (!atfp_asa_map_all_dst_stopped(_map))
        return; // only the last destination storage handle can proceed
    if (json_object_size(err_info) == 0) {
        _api_rpc__atfp_dst_init_done_cb(_map, err_info, receipt);
    } else {
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
}

static void api_rpc_transcode__atfp_src_init_finish_cb(atfp_t *processor) {
    json_t            *err_info = processor->data.error;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    asa_op_base_cfg_t *asa_dst = NULL;
    atfp_asa_map_t    *_map = asa_src->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    arpc_receipt_t    *receipt = asa_src->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
#if 0
    json_object_set_new(err_info, "__dev__", json_string("assertion for memory chekcing, after init atfp src"));
#endif
    uint8_t has_err = json_object_size(err_info) > 0;
    if (!has_err) {
        atfp_asa_map_reset_dst_iteration(_map);
        while (!has_err && (asa_dst = atfp_asa_map_iterate_destination(_map))) {
            processor = asa_dst->cb_args.entries[ASA_USRARG_INDEX__AFTP];
            processor->ops->init(processor); // internally it may add error message to err_info
            has_err = json_object_size(err_info) > 0;
            if (!has_err && processor->op_async_done.init)
                atfp_asa_map_dst_start_working(_map, asa_dst);
        }
        if (atfp_asa_map_all_dst_stopped(_map)) {
            if (has_err) {
                API_RPC__SEND_ERROR_REPLY(receipt, err_info);
                api_rpc_transcoding__storagemap_deinit(_map);
            } else {
                _api_rpc__atfp_dst_init_done_cb(_map, err_info, receipt);
            } // all dst processors completed init in one event-loop cycle
        }
    } else { // de-init at here, regardless src processor has done it sync or async
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__atfp_src_init_finish_cb

static atfp_t *api_rpc_transcode__init_file_processor(
    asa_op_base_cfg_t *asaobj, const char *label, void (*callback)(struct atfp_s *)
) {
    atfp_t *processor = app_transcoder_file_processor(label);
    if (processor) {
        json_t  *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
        json_t  *spec = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
        uint32_t _usr_id = (uint32_t)json_integer_value(json_object_get(spec, "usr_id"));
        uint32_t _upld_req_id = (uint32_t)json_integer_value(json_object_get(spec, "last_upld_req"));
        asaobj->cb_args.entries[ASA_USRARG_INDEX__AFTP] = processor;
        processor->data = (atfp_data_t){
            .error = err_info,
            .spec = spec,
            .callback = callback,
            .usr_id = _usr_id,
            .upld_req_id = _upld_req_id,
            .version = asaobj->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL],
            .rpc_receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT],
            .storage = {.basepath = asaobj->op.mkdir.path.origin, .handle = asaobj},
        };
    }
    return processor;
} // end of api_rpc_transcode__init_file_processor

static void api_rpc_transcode__try_init_file_processors(asa_op_base_cfg_t *asaobj) {
    json_t            *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    json_t            *spec = asaobj->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST];
    atfp_asa_map_t    *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    atfp_t            *processor = NULL;
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(_map);
    asa_op_base_cfg_t *asa_dst = NULL;
    // TODO, upgrade libmagic to latest version, to prevent memory leak (currently 5.14)
    // https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=840754
    magic_t m = magic_open(MAGIC_MIME_TYPE); // check magic bytes of the file
    if (magic_load(m, "/usr/share/misc/magic") == 0) {
        const char *mimetype =
            magic_buffer(m, (const void *)asa_src->op.read.dst, SRC_FILECHUNK_BEGINNING_READ_SZ);
        processor = api_rpc_transcode__init_file_processor(
            asa_src, mimetype, api_rpc_transcode__atfp_src_init_finish_cb
        );
        if (processor == NULL) {
            json_object_set_new(err_info, "transcoder", json_string("unsupported source file format"));
            json_object_set_new(err_info, "mimetype", json_string(mimetype));
            goto done;
        }
    } else {
        json_object_set_new(err_info, "transcoder", json_string("failed to load MIME-type database"));
        goto done;
    }
    json_t *outputs = json_object_get(spec, "outputs");
    atfp_asa_map_reset_dst_iteration(_map);
    while ((asa_dst = atfp_asa_map_iterate_destination(_map))) {
        const char *version = asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL];
        json_t     *output = json_object_get(outputs, version);
        const char *ofmt_label = json_string_value(json_object_get(output, "container"));
        if (!ofmt_label) {
            json_t *_out_item_internal = json_object_get(output, "__internal__");
            ofmt_label = json_string_value(json_object_get(_out_item_internal, "container"));
            if (!ofmt_label)
                fprintf(stderr, "[rpc][transcoder] line:%d, ofmt_label is null \n", __LINE__);
        }
        processor =
            api_rpc_transcode__init_file_processor(asa_dst, ofmt_label, api_rpc_atfp__dst_init_async_done_cb);
        if (processor == NULL) {
            json_object_set_new(err_info, "transcoder", json_string("unsupported destination file format"));
            goto done;
        }
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
        uint8_t version_exist = (uint8_t)asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_EXIST_FLAG];
#pragma GCC diagnostic pop
        processor->transfer.transcoded_dst.flags.version_exists = version_exist;
    } // end of loop
    processor = asa_src->cb_args.entries[ASA_USRARG_INDEX__AFTP];
    processor->ops->init(processor); // internally it may add error message to err_info
done:
    magic_close(m);
} // end of api_rpc_transcode__try_init_file_processors

static void
api_rpc_transcode__src_first_chunk_read_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result, size_t nread) {
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    json_t         *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    _map->app_sync_cnt -= 1;
    if (json_object_size(err_info) > 0) {
        // pass
    } else if (app_result == ASTORAGE_RESULT_COMPLETE && nread == SRC_FILECHUNK_BEGINNING_READ_SZ) {
        if (_map->app_sync_cnt == 0)
            api_rpc_transcode__try_init_file_processors(asaobj);
    } else {
        json_object_set_new(
            err_info, "storage", json_string("failed to read begining portion of the first file chunk")
        );
    }
    if (_map->app_sync_cnt == 0 && json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__src_first_chunk_read_cb

static void api_rpc_transcode__open_src_first_chunk_cb(
    asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result
) { // read first few bytes,
    json_t *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    if (json_object_size(err_info) > 0) {
        // pass
    } else if (app_result == ASTORAGE_RESULT_COMPLETE) {
        asaobj->op.read.cb = api_rpc_transcode__src_first_chunk_read_cb;
        asaobj->op.read.dst_sz = SRC_FILECHUNK_BEGINNING_READ_SZ;
        app_result = asaobj->storage->ops.fn_read(asaobj);
        if (app_result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "storage", json_string("failed to issue read-file operation"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open original file chunk"));
    }
    if (json_object_size(err_info) > 0) {
        atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
        if (--_map->app_sync_cnt == 0) {
            arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
            API_RPC__SEND_ERROR_REPLY(receipt, err_info);
            api_rpc_transcoding__storagemap_deinit(_map);
        }
    } // TODO, figure out how to solve the problem if error happens to both event callbacks.
} // end of api_rpc_transcode__open_src_first_chunk_cb

static void api_rpc_transcode__create_folder_common_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE app_result) {
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP];
    json_t         *err_info = asaobj->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO];
    _map->app_sync_cnt -= 1;
    if (json_object_size(err_info) > 0) {
        // pass
    } else if (app_result == ASTORAGE_RESULT_COMPLETE) {
        if (_map->app_sync_cnt == 0)
            api_rpc_transcode__try_init_file_processors(asaobj);
    } else {
        json_object_set_new(
            err_info, "storage", json_string("failed to create work folder for transcoded file")
        );
    }
    if (_map->app_sync_cnt == 0 && json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = asaobj->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT];
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        api_rpc_transcoding__storagemap_deinit(_map);
    }
} // end of api_rpc_transcode__create_folder_common_cb

static asa_op_base_cfg_t *api_rpc_transcode__init_asa_obj(
    arpc_receipt_t *receipt, json_t *api_req, json_t *err_info, asa_cfg_t *storage, uint8_t num_cb_args,
    uint32_t rd_buf_bytes, uint32_t wr_buf_bytes
) {
    asa_op_base_cfg_t *out =
        app_storage__init_asaobj_helper(storage, num_cb_args, rd_buf_bytes, wr_buf_bytes);
    // each storage handle connects to its own file processor, it is one-to-one relationship
    out->cb_args.entries[ASA_USRARG_INDEX__AFTP] = NULL;
    // all storage handles share the same following objects
    out->cb_args.entries[ASA_USRARG_INDEX__ASAOBJ_MAP] = NULL;
    out->cb_args.entries[ASA_USRARG_INDEX__RPC_RECEIPT] = (void *)receipt;
    out->cb_args.entries[ASA_USRARG_INDEX__API_REQUEST] = (void *)api_req;
    out->cb_args.entries[ASA_USRARG_INDEX__ERROR_INFO] = (void *)err_info;
    return out;
} // end of api_rpc_transcode__init_asa_obj

static __attribute__((optimize("O0"))) void api_rpc_task_handler__start_transcode(arpc_receipt_t *receipt) {
    json_error_t          jerror = {0};
    asa_op_base_cfg_t    *asa_src = NULL, *asa_dst = NULL;
    asa_op_localfs_cfg_t *asa_local_tmpbuf = NULL;
    ASA_RES_CODE          asa_result = ASTORAGE_RESULT_ACCEPT;
    json_t               *err_info = json_object();
    json_t               *api_req =
        json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    atfp_asa_map_t *asaobj_map = NULL;
    int             _app_sync_cnt = 0;
    if (jerror.line >= 0 || jerror.column >= 0) {
        json_t *item = json_object();
        json_object_set_new(item, "message", json_string("invalid JSON format found in request"));
        json_object_set_new(item, "line", json_integer(jerror.line));
        json_object_set_new(item, "column", json_integer(jerror.column));
        json_object_set_new(err_info, "non-field", item);
        goto error;
    }
    const char *_metadata_db = json_string_value(json_object_get(api_req, "metadata_db"));
    const char *src_storage_alias = json_string_value(json_object_get(api_req, "storage_alias"));
    uint32_t    _usr_id = (uint32_t)json_integer_value(json_object_get(api_req, "usr_id"));
    uint32_t    _upld_req_id = (uint32_t)json_integer_value(json_object_get(api_req, "last_upld_req"));
    json_t     *outputs = json_object_get(api_req, "outputs");
    uint32_t    num_destinations = outputs ? json_object_size(outputs) : 0;
    if (_upld_req_id == 0)
        json_object_set_new(err_info, "upld_req", json_string("has to be non-zero unsigned integer"));
    if (_usr_id == 0)
        json_object_set_new(err_info, "usr_id", json_string("has to be non-zero unsigned integer"));
    if (!_metadata_db)
        json_object_set_new(err_info, "metadata_db", json_string("required"));
    if (!src_storage_alias)
        json_object_set_new(err_info, "storage_alias", json_string("required"));
    if (!outputs || num_destinations == 0)
        json_object_set_new(err_info, "outputs", json_string("required"));
    if (json_object_size(err_info) > 0)
        goto error;
    // storage applied to both file processors is local filesystem in this app
    asa_cfg_t *src_storage = app_storage_cfg_lookup(src_storage_alias);
    asaobj_map = atfp_asa_map_init(num_destinations);
    { // instantiate asa objects
        asa_src = api_rpc_transcode__init_asa_obj(
            receipt, api_req, err_info, src_storage, (uint8_t)NUM_USRARGS_ASA_SRC,
            (uint32_t)APP_ENCODED_RD_BUF_SZ, (uint32_t)0
        );
        asa_local_tmpbuf = (asa_op_localfs_cfg_t *)api_rpc_transcode__init_asa_obj(
            receipt, api_req, err_info, app_storage_cfg_lookup("localfs"), (uint8_t)NUM_USRARGS_ASA_LOCALTMP,
            (uint32_t)0, (uint32_t)0
        );
        // set event loop to each file processor. TODO: event loop field should be moved to parent
        // type
        if (!strcmp(src_storage->alias, "localfs"))
            ((asa_op_localfs_cfg_t *)asa_src)->loop = receipt->loop; // TODO
        asa_local_tmpbuf->loop = receipt->loop;
        atfp_asa_map_set_source(asaobj_map, asa_src);
        atfp_asa_map_set_localtmp(asaobj_map, asa_local_tmpbuf);
        asa_src->deinit = api_rpc_transcode__asa_src_deinit;
        asa_local_tmpbuf->super.deinit = api_rpc_transcode__asa_localtmp_deinit;
        const char *version = NULL;
        json_t     *output = NULL;
        json_object_foreach(outputs, version, output) {
            const char *dst_storage_alias = json_string_value(json_object_get(output, "storage_alias"));
            asa_cfg_t  *dst_storage = app_storage_cfg_lookup(dst_storage_alias);
            if (!dst_storage) {
                json_object_set_new(err_info, "dst_storage_alias", json_string("invalid"));
                goto error;
            }
            uint8_t version_exist = (uint8_t
            )json_boolean_value(json_object_get(json_object_get(output, "__internal__"), "is_update"));
            asa_dst = api_rpc_transcode__init_asa_obj(
                receipt, api_req, err_info, dst_storage, (uint8_t)NUM_USRARGS_ASA_DST, (uint32_t)0,
                (uint32_t)APP_ENCODED_WR_BUF_SZ
            );
            if (!strcmp(dst_storage->alias, "localfs"))
                ((asa_op_localfs_cfg_t *)asa_dst)->loop = receipt->loop; // TODO
            char *version_cpy = calloc(4, sizeof(char)); // used as key in json object, to make valgrind happy
            strncpy(version_cpy, version, APP_TRANSCODED_VERSION_SIZE);
            asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL] = (void *)version_cpy;
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
            asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_EXIST_FLAG] = (void *)version_exist;
#pragma GCC diagnostic pop
            atfp_asa_map_add_destination(asaobj_map, asa_dst);
            asa_dst->deinit = api_rpc_transcode__asa_dst_deinit;
        } // end of iteration
    }
    { // create work folder for local temp buffer
        app_cfg_t *app_cfg = app_get_global_cfg();
        size_t     path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1 +
                         UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1; // include NULL-terminated byte
        char   basepath[path_sz];
        size_t nwrite =
            snprintf(&basepath[0], path_sz, "%s/%d/%08x", app_cfg->tmp_buf.path, _usr_id, _upld_req_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_local_tmpbuf->file.file = -1;
        asa_local_tmpbuf->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local_tmpbuf->super.op.mkdir.cb = api_rpc_transcode__create_folder_common_cb;
        asa_local_tmpbuf->super.op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_local_tmpbuf->super.op.mkdir.path.curr_parent = (void *)calloc(nwrite, sizeof(char));
        asa_result = asa_local_tmpbuf->super.storage->ops.fn_mkdir(&asa_local_tmpbuf->super, 1);
        if (asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(
                err_info, "storage", json_string("failed to issue create-folder operation for tmp buf")
            );
            goto error;
        }
    }
    { // open source file then read first portion, assume NULL-terminated string
        size_t path_sz = USR_ID_STR_SIZE + 1 + UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1;
        char   basepath[path_sz];
        size_t nwrite = snprintf(&basepath[0], path_sz, "%d/%08x", _usr_id, _upld_req_id);
        basepath[nwrite++] = 0x0; // NULL-terminated
        asa_src->op.mkdir.path.origin = (void *)strndup(&basepath[0], nwrite);
        asa_result = atfp_open_srcfile_chunk(
            asa_src, asa_src->op.mkdir.path.origin, 1, api_rpc_transcode__open_src_first_chunk_cb
        );
        if (asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(err_info, "storage", json_string("failed to issue open-file operation"));
            goto error;
        }
    }
    atfp_asa_map_reset_dst_iteration(asaobj_map);
    size_t transcoding_fullpath_sz = 0, version_fullpath_sz = 0;
    while ((asa_dst = atfp_asa_map_iterate_destination(asaobj_map))) {
        const char *version = asa_dst->cb_args.entries[ASA_USRARG_INDEX__VERSION_LABEL];
        transcoding_fullpath_sz =
            USR_ID_STR_SIZE + 1 + UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1 + ATFP__MAXSZ_STATUS_FOLDER_NAME + 1;
        version_fullpath_sz = transcoding_fullpath_sz + strlen(version) + 1;
        asa_dst->op.mkdir.path.prefix = (void *)calloc(transcoding_fullpath_sz, sizeof(char));
        asa_dst->op.mkdir.path.origin = (void *)calloc(version_fullpath_sz, sizeof(char));
        asa_dst->op.mkdir.path.curr_parent = (void *)calloc(version_fullpath_sz, sizeof(char));
        // will be used later when application moves transcoded file from temporary buffer (locally
        // stored in transcoding server) to destination storage (may be remotely stored, e.g. in
        // cloud platform)
        size_t nwrite = snprintf(
            asa_dst->op.mkdir.path.origin, transcoding_fullpath_sz, "%d/%08x/%s", _usr_id, _upld_req_id,
            ATFP__TEMP_TRANSCODING_FOLDER_NAME
        );
        asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
        assert(nwrite <= transcoding_fullpath_sz);
    }
    { // ensure transcoding folder by the first asa_dst object
        atfp_asa_map_reset_dst_iteration(asaobj_map);
        asa_dst = atfp_asa_map_iterate_destination(asaobj_map);
        asa_dst->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_dst->op.mkdir.cb = api_rpc_transcode__create_folder_common_cb;
        asa_result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
        if (asa_result == ASTORAGE_RESULT_ACCEPT) {
            _app_sync_cnt += 1;
        } else {
            json_object_set_new(
                err_info, "storage", json_string("failed to issue mkdir operation to storage")
            );
            goto error;
        }
    } // create folder for saving transcoded files in destinations
    asaobj_map->app_sync_cnt = _app_sync_cnt;
    return;
error:
    if (_app_sync_cnt > 0) { // will send error message to reply queue in next event-loop cycle
        asaobj_map->app_sync_cnt = _app_sync_cnt;
    } else {
        API_RPC__SEND_ERROR_REPLY(receipt, err_info);
        if (asaobj_map) {
            api_rpc_transcoding__storagemap_deinit(asaobj_map);
        } else {
            if (api_req) {
                json_decref(api_req);
            }
            if (err_info) {
                json_decref(err_info);
            }
        }
    }
} // end of api_rpc_task_handler__start_transcode
