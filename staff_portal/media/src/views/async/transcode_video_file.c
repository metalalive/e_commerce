#include <jansson.h>
#include <magic.h>

#include "app_cfg.h"
#include "views.h"
#include "rpc/core.h"
#include "storage/localfs.h"
#include "transcoder/file_processor.h"

#define   TEMP_TRANSCODING_FOLDER_NAME  "transcoding"
#define   APP_ENCODED_RD_BUF_SZ       2048
#define   ASA_USRARG_RPC_RECEIPT_INDEX  6
#define   ASA_USRARG_ATFP_INDEX         ATFP_INDEX_IN_ASA_OP_USRARG


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


static void _storage_error_handler(asa_op_base_cfg_t *cfg) {
    if(!cfg) { return; }
    if(cfg->op.open.dst_path) {
        free(cfg->op.open.dst_path);
        cfg->op.open.dst_path = NULL;
    }
    if(cfg->cb_args.entries[1]) { // request from app server
        json_t *api_req = cfg->cb_args.entries[1];
        json_decref(api_req);
        cfg->cb_args.entries[1] = NULL;
    }
    if(cfg->cb_args.entries[2]) { // base path in storage function
        free(cfg->cb_args.entries[2]);
        cfg->cb_args.entries[2] = NULL;
    }
    if(cfg->cb_args.entries[4]) { // base path in local temp buffer space
        free(cfg->cb_args.entries[4]);
        cfg->cb_args.entries[4] = NULL;
    }
    if(cfg->cb_args.entries[5]) {
        json_t *err_info = cfg->cb_args.entries[5];
        json_decref(err_info);
        cfg->cb_args.entries[5] = NULL;
    }
    free(cfg);
} // end of _storage_error_handler


static  __attribute__((optimize("O0"))) void  _transcoding__processing_finish_cb (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    asa_op_base_cfg_t *cfg = processor->data.src.storage.handle;
    arpc_receipt_t *receipt = cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX];
    if (json_object_size(err_info) == 0) {
        json_t *api_req = cfg->cb_args.entries[1];
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
    _storage_error_handler(cfg);
    processor->ops->deinit(processor);
} // end of _transcoding__processing_finish_cb


static  void _transcoding__io_init_finish_cb (atfp_t  *processor)
{
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) == 0) {
        processor->data.callback = _transcoding__processing_finish_cb;
        processor->ops->processing(processor);
    }
    if (json_object_size(err_info) > 0) {
        asa_op_base_cfg_t *cfg = processor->data.src.storage.handle;
        arpc_receipt_t *receipt = cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX];
        _rpc_task_send_reply(receipt, err_info);
        _storage_error_handler(cfg);
         processor->ops->deinit(processor);
    }
} // end of _transcoding__io_init_finish_cb


static __attribute__((optimize("O0"))) void _transcoding__first_filechunk__read_first_portion_cb (
        asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result, size_t nread)
{
    json_t *err_info = cfg->cb_args.entries[5];
    atfp_t *processor = NULL;
    if(app_result != ASTORAGE_RESULT_COMPLETE || nread == 0) {
        json_object_set_new(err_info, "storage", json_string("failed to read begining portion of the first file chunk"));
        goto done;
    }
    magic_t  m = magic_open(MAGIC_MIME_TYPE); // check magic bytes of the file
    if(magic_load(m, NULL) != 0) {
        json_object_set_new(err_info, "transcoder", json_string("failed to load MIME-type database"));
        goto done;
    }
    const char *mimetype = magic_buffer(m, (const void *)cfg->op.read.dst, nread);
    processor = app_transcoder_file_processor(mimetype);
    if(processor == NULL) {
        json_object_set_new(err_info, "transcoder", json_string("unsupported input resource"));
        goto done;
    }
    cfg->cb_args.entries[ASA_USRARG_ATFP_INDEX] = processor;
    processor->data = (atfp_data_t) {
        .error=err_info, .callback=_transcoding__io_init_finish_cb,
        .spec=cfg->cb_args.entries[1], // request from app server
        .loop=((asa_op_localfs_cfg_t *)cfg)->loop,
        .local_tmpbuf_basepath=cfg->cb_args.entries[4],
        .src={ .basepath=cfg->cb_args.entries[2], .storage={ .handle=cfg,
            .config=(asa_cfg_t *)cfg->cb_args.entries[3] } },
        .dst={ .basepath=cfg->op.mkdir.path.origin, .storage={
            .config=(asa_cfg_t *)cfg->cb_args.entries[3] } }
    };
    processor->ops->init(processor); // internally it may add error message to err_info
done:
    if(m)
        magic_close(m);
    if(json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX];
        _rpc_task_send_reply(receipt, err_info);
        _storage_error_handler(cfg);
        if(processor) {
            processor->ops->deinit(processor);
        }
    }
} // end of _transcoding__first_filechunk__read_first_portion_cb


static void _transcoding__open_first_filechunk_evt_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result)
{ // read first few bytes,
    json_t *err_info = cfg->cb_args.entries[5];
    if(app_result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(err_info, "storage", json_string("failed to open original file chunk"));
        goto done;
    }
    size_t expect_nread = 0x40;
    cfg->op.read.cb = _transcoding__first_filechunk__read_first_portion_cb;
    cfg->op.read.dst_sz = expect_nread;
    asa_cfg_t *storage = cfg->cb_args.entries[3];
    app_result = storage->ops.fn_read(cfg);
    if(app_result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(err_info, "storage", json_string("failed to issue read-file operation"));
done:
    if(json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX];
        _rpc_task_send_reply(receipt, err_info);
        _storage_error_handler(cfg);
    }
} // end of _transcoding__open_first_filechunk_evt_cb


static void _transcoding__create_work_folder_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result)
{
    json_t *err_info = cfg->cb_args.entries[5];
    if(app_result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(err_info, "storage", json_string("failed to create work folder for transcoded file"));
        goto done;
    }
    size_t dirpath_sz = strlen(cfg->op.mkdir.path.curr_parent); // recover destination path
    memcpy(cfg->op.mkdir.path.origin, cfg->op.mkdir.path.curr_parent, dirpath_sz);
    app_result = atfp_open_srcfile_chunk( cfg, cfg->cb_args.entries[3], cfg->cb_args.entries[2],
            1, _transcoding__open_first_filechunk_evt_cb );
    if(app_result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "storage", json_string("failed to issue open-file operation"));
    }
done:
    if(json_object_size(err_info) > 0) {
        arpc_receipt_t *receipt = cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX];
        _rpc_task_send_reply(receipt, err_info);
        _storage_error_handler(cfg);
    }
} // end of _transcoding__create_work_folder_cb


static  __attribute__((optimize("O0"))) void  api_transcode_video_file__rpc_task_handler (arpc_receipt_t *receipt)
{
    json_error_t jerror = {0};
    json_t *err_info = json_object();
    json_t *api_req = json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    asa_op_base_cfg_t *asa_cfg = NULL; // storage config object for reading original resource
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
    app_cfg_t *app_cfg = app_get_global_cfg();
    asa_cfg_t *storage = &app_cfg->storages.entries[0]; // storage is local filesystem in this app
#define  NUM_CB_ARGS  7
    size_t dirpath_sz = strlen(storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(last_upld_req) + 1 + strlen(TEMP_TRANSCODING_FOLDER_NAME) + 1 +
            APP_TRANSCODED_VERSION_SIZE + 1; // assume NULL-terminated string
    size_t cb_args_tot_sz = sizeof(void *) * NUM_CB_ARGS; // for receipt, api_req, base path of the resource
    size_t rd_dst_buf_sz = APP_ENCODED_RD_BUF_SZ;
    size_t asa_cfg_sz = sizeof(asa_op_localfs_cfg_t) + (dirpath_sz << 1) + 
              cb_args_tot_sz + rd_dst_buf_sz;
    asa_cfg = malloc(asa_cfg_sz);
    memset(asa_cfg, 0x0, asa_cfg_sz);
    { // start of storage object setup
        char *ptr = (char *)asa_cfg + sizeof(asa_op_localfs_cfg_t);
        ((asa_op_localfs_cfg_t *)asa_cfg)->loop = receipt->loop;
        asa_cfg->cb_args.size = NUM_CB_ARGS;
        asa_cfg->cb_args.entries = (void **) ptr;
        asa_cfg->cb_args.entries[ASA_USRARG_RPC_RECEIPT_INDEX] = (void *)receipt;
        asa_cfg->cb_args.entries[1] = (void *)api_req;
        asa_cfg->cb_args.entries[3] = (void *)storage; // global config object for  storage
        asa_cfg->cb_args.entries[5] = (void *)err_info;
        asa_cfg->cb_args.entries[ASA_USRARG_ATFP_INDEX] = NULL; // reserved for later transcoding file-processor
        { // pre-calculated base path for files accessed by storage API
            char basepath[dirpath_sz];
            size_t nwrite = snprintf(&basepath[0], dirpath_sz, "%s/%d/%08x", storage->base_path,
                    usr_id, last_upld_req);
            basepath[nwrite++] = 0x0; // NULL-terminated
            asa_cfg->cb_args.entries[2] = (void *)strndup(&basepath[0], nwrite);
        } { // pre-calculated base path for files locally accessed in transcoding RPC consumer
            size_t path_sz = strlen(app_cfg->tmp_buf.path) + 1 + USR_ID_STR_SIZE + 1 +
                UPLOAD_INT2HEX_SIZE(last_upld_req) + 1 + strlen(TEMP_TRANSCODING_FOLDER_NAME) + 1 +
                APP_TRANSCODED_VERSION_SIZE + 1; // include NULL-terminated byte
            char basepath[path_sz];
            size_t nwrite = snprintf(&basepath[0], path_sz, "%s/%d/%08x/%s/%s", app_cfg->tmp_buf.path,
                    usr_id, last_upld_req, TEMP_TRANSCODING_FOLDER_NAME, version);
            basepath[nwrite++] = 0x0; // NULL-terminated
            asa_cfg->cb_args.entries[4] = (void *)strndup(&basepath[0], nwrite);
        }
        ptr += cb_args_tot_sz;
        asa_cfg->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_cfg->op.mkdir.cb = _transcoding__create_work_folder_cb;
        asa_cfg->op.mkdir.path.origin = ptr;
        ptr += dirpath_sz;
        asa_cfg->op.mkdir.path.curr_parent = ptr;
        ptr += dirpath_sz;
        {  // will be used later when application moves transcoded file from temporary buffer (locally
           // stored in transcoding server) to destination storage (may be remotely stored, e.g. in cloud platform)
            char dirpath[dirpath_sz];
            size_t nwrite = snprintf(&dirpath[0], dirpath_sz, "%s/%s/%s", (const char *)asa_cfg->cb_args.entries[2],
                     TEMP_TRANSCODING_FOLDER_NAME, version);
            dirpath[nwrite++] = 0x0; // NULL-terminated
            assert(nwrite <= dirpath_sz);
            memcpy(asa_cfg->op.mkdir.path.origin, dirpath, dirpath_sz);
        }
        asa_cfg->op.read.offset = 0;
        asa_cfg->op.read.dst_max_nbytes = rd_dst_buf_sz;
        asa_cfg->op.read.dst_sz = 0;
        asa_cfg->op.read.dst = (char *)ptr;
        ptr += rd_dst_buf_sz;
        assert((size_t)(ptr - (char *)asa_cfg) == asa_cfg_sz);
    } // end of storage object setup
    ASA_RES_CODE asa_result = storage->ops.fn_mkdir(asa_cfg);
    if (asa_result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "storage", json_string("failed to issue mkdir operation to storage"));
        goto error;
    }
    return;
error:
    _rpc_task_send_reply(receipt, err_info);
    if(asa_cfg) {
        _storage_error_handler(asa_cfg);
    } else {
        if(api_req) { json_decref(api_req); }
        if(err_info) { json_decref(err_info); }
    }
#undef  NUM_CB_ARGS
} // end of api_transcode_video_file__rpc_task_handler

