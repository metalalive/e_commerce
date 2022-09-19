#include <fcntl.h>
#include <string.h>

#include "transcoder/file_processor.h"

#define  PRINT_DST_PATH_STRING(out, prefix, status_f, version) \
{ \
    (out)[0] = 0; \
    strcat(out, prefix); \
    strcat(out, "/"); \
    strcat(out, status_f); \
    strcat(out, "/"); \
    strcat(out, version); \
    size_t  sz = strlen(out); \
    (out)[sz] = 0; \
}

// NOTE: each destination file-processor is responsible to remove the version folder
//  under discarding folder, this project do not provide one-size-fit-all solution.
static void  _atfp__move_transcoding_to_committed_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ //  move transcoded files to committed folder
    atfp_t  *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t  *err_info  = processor->data.error;
    if (result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(err_info, "transcode", json_string(
                    "[storage] failed to move from transcoding to committed folder"));
    }
    processor->data.callback(processor);
} // end of _atfp__move_transcoding_to_committed_cb


static void  _atfp__move_committed_to_discarding_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_t  *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t  *err_info  = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t path_sz  = strlen(asaobj->op.mkdir.path.prefix) + 2 + ATFP__MAXSZ_STATUS_FOLDER_NAME;
        char  old_path[path_sz],  new_path[path_sz];
        PRINT_DST_PATH_STRING( &new_path[0], asaobj->op.mkdir.path.prefix,
              ATFP__COMMITTED_FOLDER_NAME,  processor->data.version );
        PRINT_DST_PATH_STRING( &old_path[0], asaobj->op.mkdir.path.prefix,
             ATFP__TEMP_TRANSCODING_FOLDER_NAME,  processor->data.version );
        asaobj->op.rename.cb = _atfp__move_transcoding_to_committed_cb;
        asaobj->op.rename.path._new = &new_path[0];
        asaobj->op.rename.path._old = &old_path[0];
        result = asaobj->storage->ops.fn_rename(asaobj);
        if (result != ASTORAGE_RESULT_ACCEPT) 
            json_object_set_new(err_info, "transcode", json_string(
                 "[storage] failed to issue move operation for transcoded files"));
    } else {
        json_object_set_new(err_info, "transcode", json_string(
            "[storage] failed to move from committed to discarding folder"));
    }
    if(json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of _atfp__move_committed_to_discarding_cb


static void  _atfp__ensure_dst_committed_basepath_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // move committed files (if exists) to discarding folder
    atfp_t  *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t  *err_info  = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint8_t  _is_update = processor->transfer.dst.flags.version_exists;
        size_t path_sz  = strlen(asaobj->op.mkdir.path.prefix) + 2 + ATFP__MAXSZ_STATUS_FOLDER_NAME;
        char  old_path[path_sz],  new_path[path_sz];
        PRINT_DST_PATH_STRING( &new_path[0], asaobj->op.mkdir.path.prefix,
             ((_is_update)?ATFP__DISCARDING_FOLDER_NAME:ATFP__COMMITTED_FOLDER_NAME) ,
             processor->data.version );
        PRINT_DST_PATH_STRING( &old_path[0], asaobj->op.mkdir.path.prefix,
             ((_is_update)?ATFP__COMMITTED_FOLDER_NAME:ATFP__TEMP_TRANSCODING_FOLDER_NAME) ,
             processor->data.version );
        if(_is_update) {
            asaobj->op.rename.cb = _atfp__move_committed_to_discarding_cb;
        } else {
            asaobj->op.rename.cb = _atfp__move_transcoding_to_committed_cb;
        }
        asaobj->op.rename.path._new = &new_path[0];
        asaobj->op.rename.path._old = &old_path[0];
        result = asaobj->storage->ops.fn_rename(asaobj);
        if (result != ASTORAGE_RESULT_ACCEPT) 
            json_object_set_new(err_info, "transcode", json_string(
                 "[storage] failed to issue move operation for transcoded files"));
    } else {
        json_object_set_new(err_info, "transcode", json_string(
                    "[storage] failed to create folder for committed files"));
    }
    if(json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of _atfp__ensure_dst_committed_basepath_cb


static void _atfp__ensure_dst_discarding_basepath_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // ensure committed folder
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_dst = asaobj;
        size_t nwrite = sprintf(asa_dst->op.mkdir.path.origin, "%s", ATFP__COMMITTED_FOLDER_NAME);
        asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
        asa_dst->op.mkdir.path.curr_parent[0] = 0x0; // reset for mkdir
        asa_dst->op.mkdir.cb = _atfp__ensure_dst_committed_basepath_cb;
        result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
        if (result != ASTORAGE_RESULT_ACCEPT) 
            json_object_set_new(processor->data.error, "transcode", json_string(
                        "[storage] failed to create folder for discarding files"));
    } else {
        json_object_set_new(processor->data.error, "transcode", json_string(
             "[storage] failed to issue mkdir operation for committed files"));
    }
    if(json_object_size(processor->data.error) > 0) 
        processor->data.callback(processor);
} // end of _atfp__ensure_dst_discarding_basepath_cb


void  atfp_storage__commit_new_version(atfp_t *processor)
{ // ensure discarding folder
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    size_t nwrite = 0;
    assert(processor->data.error);
    assert(asa_dst->op.mkdir.path.prefix);
    assert(asa_dst->op.mkdir.path.origin);
    nwrite = sprintf(asa_dst->op.mkdir.path.prefix, "%s/%d/%08x", asa_dst->storage->base_path,
            processor->data.usr_id, processor->data.upld_req_id);
    asa_dst->op.mkdir.path.prefix[nwrite++] = 0x0; // NULL-terminated
    nwrite = sprintf(asa_dst->op.mkdir.path.origin, "%s", ATFP__DISCARDING_FOLDER_NAME);
    asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
    asa_dst->op.mkdir.path.curr_parent[0] = 0x0; // reset for mkdir
    asa_dst->op.mkdir.mode = S_IRWXU;
    asa_dst->op.mkdir.cb = _atfp__ensure_dst_discarding_basepath_cb;
    ASA_RES_CODE  result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "transcode", json_string(
                    "[storage] failed to issue mkdir operation for discarding files"));
        processor->data.callback(processor);
    }
} // end of atfp_storage__commit_new_version
