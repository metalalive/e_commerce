#include "datatypes.h"
#include "transcoder/file_processor.h"

#define  STATUS_PATH_PATTERN  "%s/%d/%08x/%s"

#define  COMMON_CODE__GEN_STATUS_PATH(o_path, _status) \
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle; \
    uint32_t _usr_id = processor->data.usr_id; \
    uint32_t _upld_req_id = processor->data.upld_req_id; \
    assert(_usr_id != 0 && _upld_req_id != 0); \
    size_t  opath_sz = sizeof(STATUS_PATH_PATTERN) + strlen(asa_remote->storage->base_path) + \
        USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(_upld_req_id) + strlen(_status); \
    char o_path[opath_sz]; \
    size_t nwrite = snprintf(&o_path[0], opath_sz, STATUS_PATH_PATTERN, \
            asa_remote->storage->base_path, _usr_id, _upld_req_id,  _status); \
    o_path[nwrite++] = 0x0; \
    assert(nwrite <= opath_sz);

static void  _atfp_discard__remove_version_start(atfp_t *);
static void  _atfp_discard__status_scan_start (atfp_t *, const char *status);

static void  _atfp_discard__finalize(atfp_t *processor)
{
    json_t *spec = processor->data.spec;
    json_object_del(spec, "_atfp_rm_status_list");
    json_object_del(spec, "_atfp_versions_under_status");
    processor->transfer.discard.usr_cb(processor);
}

static void  _atfp_discard__go_next_status (atfp_t *processor)
{
    json_t *spec = processor->data.spec, *status_list = json_object_get(spec, "_atfp_rm_status_list");
    json_array_remove(status_list, 0);
    if(json_array_size(status_list) > 0) {
        const char *curr_status = json_string_value(json_array_get(status_list, 0));
        _atfp_discard__status_scan_start(processor, curr_status);
    } else {
        _atfp_discard__finalize(processor);
    } // all status folders are removed
}

static void _atfp_discard__status_rmdir_done (asa_op_base_cfg_t *asa_remote, ASA_RES_CODE result)
{
    atfp_t *processor = asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    _atfp_discard__go_next_status (processor);
}

static void _atfp_discard__remove_status_folder(atfp_t *processor)
{
    json_t *err_info = processor->data.error, *spec = processor->data.spec,
           *status_list = json_object_get(spec, "_atfp_rm_status_list");
    const char *curr_status = json_string_value(json_array_get(status_list, 0));
    COMMON_CODE__GEN_STATUS_PATH(fullpath, curr_status);
    asa_remote->op.rmdir.path = &fullpath[0];
    asa_remote->op.rmdir.cb   = _atfp_discard__status_rmdir_done;
    ASA_RES_CODE  result =  asa_remote->storage->ops.fn_rmdir(asa_remote);
    asa_remote->op.rmdir.path = NULL;
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string("[storage] failed to"
                " issue rmdir operation"));
        fprintf(stderr, "[atfp][removal] line:%d, result:%d, path:%s \n", __LINE__, result, &fullpath[0]);
        _atfp_discard__finalize(processor);
    }
}

static void _atfp_discard__remove_version_done(atfp_t *processor)
{
    json_t *err_info = processor->data.error, *spec = processor->data.spec,
           *version_list = json_object_get(spec, "_atfp_versions_under_status");
    if(json_object_size(err_info) == 0) {
        json_array_remove(version_list, 0);
        if(json_array_size(version_list) > 0) {
            _atfp_discard__remove_version_start(processor);
        } else {
            _atfp_discard__remove_status_folder(processor);
        }
    } else {
        json_t *status_list  = json_object_get(spec, "_atfp_rm_status_list"),
               *curr_ver_item = json_array_get(version_list, 0);
        const char *_curr_status = json_string_value(json_array_get(status_list, 0));
        const char *ent_name = json_string_value(json_object_get(curr_ver_item, "name"));
        asa_dirent_type_t ent_type = json_integer_value(json_object_get(curr_ver_item, "type"));
        fprintf(stderr, "[atfp][removal] line:%d, failed to remove version in remote storage, status:%s"
             ", version-name:%s, version-type:%d \n", __LINE__, _curr_status, ent_name, ent_type );
        _atfp_discard__finalize(processor);
    }
} // end of _atfp_discard__remove_version_done

static void _atfp_discard__remove_version_start(atfp_t *processor)
{
    json_t *err_info = processor->data.error, *spec = processor->data.spec,
           *status_list  = json_object_get(spec, "_atfp_rm_status_list"),
           *version_list = json_object_get(spec, "_atfp_versions_under_status"),
           *curr_version = json_array_get(version_list, 0);
    const char *curr_status = json_string_value(json_array_get(status_list, 0));
    processor->data.callback = _atfp_discard__remove_version_done;
    processor->data.version = json_string_value(json_object_get(curr_version, "name"));
    processor->transfer.discard.remove_ver_storage(processor, curr_status);
    if(json_object_size(err_info) > 0)
        _atfp_discard__finalize(processor);
} // end of  _atfp_discard__remove_version_start


static void _atfp_discard__status_gather_versions (asa_op_base_cfg_t *asa_remote,
        json_t *err_info, json_t *spec)
{
    size_t idx = 0;
    size_t num_files = asa_remote->op.scandir.fileinfo.size;
    json_t *versions_item = json_object_get(spec, "_atfp_versions_under_status");
    if(versions_item) {
        json_array_clear(versions_item);
    } else {
        versions_item = json_array();
        json_object_set_new(spec, "_atfp_versions_under_status", versions_item);
    }
    for(idx = 0; idx < num_files; idx++) {
        asa_dirent_t  entry = {0};
        ASA_RES_CODE result = asa_remote->storage->ops.fn_scandir_next(asa_remote, &entry);
        if(result == ASTORAGE_RESULT_COMPLETE) {
            json_t *version_item = json_object();
            json_object_set_new(version_item, "name", json_string(entry.name));
            json_object_set_new(version_item, "type", json_integer(entry.type));
            json_array_append_new(versions_item, version_item);
        } else {
            json_t *status_list  = json_object_get(spec, "_atfp_rm_status_list");
            const char *_curr_status = json_string_value(json_array_get(status_list, 0));
            fprintf(stderr, "[atfp][removal] line:%d, result:%d, status:%s, version-name:%s"
                    ", version-type:%d \n", __LINE__, result, _curr_status, entry.name, entry.type );
            json_object_set_new(err_info, "transcode", json_string("[storage] failed"
                        " to retrieve next entry in scandir result"));
            break;
        }
    } // end of loop
    if(idx == num_files) {
        asa_dirent_t  e = {0};
        ASA_RES_CODE result =  asa_remote->storage->ops.fn_scandir_next(asa_remote, &e);
        if(result != ASTORAGE_RESULT_EOF_SCAN) {
            json_object_set_new(err_info, "transcode", json_string(
                "[storage] unexpected entry found in scandir result"));
        }
    }
    asa_remote->op.scandir.fileinfo.size = 0;
} // end of  _atfp_discard__status_gather_versions

static void _atfp_discard__status_scan_done (asa_op_base_cfg_t *asa_remote, ASA_RES_CODE result)
{
    atfp_t *processor = asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        json_t *err_info = processor->data.error, *spec = processor->data.spec;
        size_t num_files = asa_remote->op.scandir.fileinfo.size;
        if(num_files > 0) {
            _atfp_discard__status_gather_versions (asa_remote, err_info, spec);
            if(json_object_size(err_info) == 0) {
                _atfp_discard__remove_version_start(processor);
            } else {
                _atfp_discard__finalize(processor);
            }
        } else { // it is empty, directly remove status folder
            _atfp_discard__remove_status_folder(processor);
        }
    } else { // status folder might not exist
        _atfp_discard__go_next_status (processor);
    }
} // end of  _atfp_discard__status_scan_done

static void  _atfp_discard__status_scan_start (atfp_t *processor, const char *status)
{
    json_t *err_info = processor->data.error;
    COMMON_CODE__GEN_STATUS_PATH(fullpath, status);
    asa_remote->op.scandir.path = &fullpath[0];
    asa_remote->op.scandir.cb   = _atfp_discard__status_scan_done;
    ASA_RES_CODE  result =  asa_remote->storage->ops.fn_scandir(asa_remote);
    asa_remote->op.scandir.path = NULL;
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string("[storage] failed to"
                " issue scandir operation for removing files"));
        fprintf(stderr, "[atfp][removal] line:%d, result:%d, path:%s \n", __LINE__, result, &fullpath[0]);
        _atfp_discard__finalize(processor);
    }
} // end of  _atfp_discard__status_scan_start


void  atfp_discard_transcoded(atfp_t *processor, void (*rm_ver)(atfp_t *, const char *), void (*_usr_cb)(atfp_t *))
{
    assert(rm_ver && _usr_cb);
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    assert(asa_remote);
    assert(!asa_remote->op.scandir.path);
    json_t *spec = processor->data.spec,  *status_list = json_array();
    json_object_set_new(spec, "_atfp_rm_status_list", status_list);
    json_array_append_new(status_list, json_string(ATFP__TEMP_TRANSCODING_FOLDER_NAME));
    json_array_append_new(status_list, json_string(ATFP__DISCARDING_FOLDER_NAME));
    json_array_append_new(status_list, json_string(ATFP__COMMITTED_FOLDER_NAME));
    processor->transfer.discard.remove_ver_storage = rm_ver;
    processor->transfer.discard.usr_cb = _usr_cb;
    const char *curr_status = json_string_value(json_array_get(status_list, 0));
    _atfp_discard__status_scan_start(processor, curr_status);
}
