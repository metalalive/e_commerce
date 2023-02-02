#include "datatypes.h"
#include "transcoder/video/common.h"

void  atfp_storage_video_remove_version(atfp_t *processor, const char *status)
{
    asa_op_base_cfg_t *asa_dst = processor ->data.storage.handle;
    uint32_t _usr_id = processor ->data.usr_id;
    uint32_t _upld_req_id = processor ->data.upld_req_id;
    const char *version = processor->data.version;
    assert(_usr_id);
    assert(_upld_req_id);
    assert(version);
    size_t  fullpath_sz = strlen(asa_dst->storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
            UPLOAD_INT2HEX_SIZE(_upld_req_id) + 1 + strlen(status) + 1 + strlen(version) + 1 ;
    char fullpath[fullpath_sz];
    size_t nwrite = snprintf(&fullpath[0], fullpath_sz, "%s/%d/%08x/%s/%s",
            asa_dst->storage->base_path, _usr_id, _upld_req_id,  status, version);
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    atfp_remote_rmdir_generic (processor, &fullpath[0]);
} // end of atfp_storage_video_remove_version


void  atfp_storage_video_create_version(atfp_t *processor, asa_mkdir_cb_t cb)
{
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    size_t nwrite = sprintf(asa_dst->op.mkdir.path.prefix, "%s/%d/%08x/%s", asa_dst->storage->base_path,
            processor->data.usr_id, processor->data.upld_req_id, ATFP__TEMP_TRANSCODING_FOLDER_NAME);
    asa_dst->op.mkdir.path.prefix[nwrite++] = 0x0; // NULL-terminated
    nwrite = sprintf(asa_dst->op.mkdir.path.origin, "%s", processor->data.version);
    asa_dst->op.mkdir.path.origin[nwrite++] = 0;
    asa_dst->op.mkdir.path.curr_parent[0] = 0x0; // reset for mkdir
    asa_dst->op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
    asa_dst->op.mkdir.cb =  cb;
    // clear allow_exist flag, to make use of OS lock, and consider EEXISTS as error after mkdir()
    ASA_RES_CODE result = asa_dst->storage->ops.fn_mkdir(asa_dst, 0);
    if (result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(processor->data.error, "storage",
                json_string("failed to issue mkdir operation to storage"));
} // end of  atfp_storage_video_create_version

