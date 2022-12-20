#include "datatypes.h"
#include "transcoder/video/common.h"

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static  void _atfp_remove_version_dealloc(atfp_t *processor)
{
    asa_op_base_cfg_t *asa_dst = processor ->data.storage.handle;
    asa_dst->op.rmdir.path = NULL;
    DEINIT_IF_EXISTS(asa_dst->op.scandir.path, free);
    if(asa_dst->op.scandir.fileinfo.data) {
        for(int idx = 0; idx < asa_dst->op.scandir.fileinfo.size; idx++) {
            asa_dirent_t  *e = &asa_dst->op.scandir.fileinfo.data[idx];
            DEINIT_IF_EXISTS(e->name, free);
        }
    }
    DEINIT_IF_EXISTS(asa_dst->op.scandir.fileinfo.data, free);
    asa_dst->op.scandir.fileinfo.size = 0;
    processor->data.callback(processor);
}

static  void  _atfp_remove_version_rmdir_done(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to remove folder for discarding old version"));
        fprintf(stderr, "[transcoder][video][storage] error, line:%d, version:%s, result:%d \n",
                __LINE__, processor->data.version, result );
    }
    _atfp_remove_version_dealloc(processor);
}

static  void  _atfp_remove_version_rmdir_start(asa_op_base_cfg_t *asa_dst, json_t *err_info)
{
    asa_dst->op.rmdir.path = asa_dst->op.scandir.path;
    asa_dst->op.rmdir.cb   = _atfp_remove_version_rmdir_done;
    ASA_RES_CODE result =  asa_dst->storage->ops.fn_rmdir(asa_dst);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string(
           "[storage] failed to issue rmdir operation for discarding old version"));
        atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        fprintf(stderr, "[transcoder][video][storage] line:%d, version:%s, result:%d \n",
                __LINE__, processor->data.version, result );
    }
}

static  void  _atfp_remove_version_unlinkfile_start (asa_op_base_cfg_t *asa_dst, json_t *err_info, asa_unlink_cb_t cb)
{
    uint32_t  max_num_files = asa_dst->op.scandir.fileinfo.size;
    uint32_t  curr_rd_idx   = asa_dst->op.scandir.fileinfo.rd_idx ++;
    assert(max_num_files > curr_rd_idx);
    asa_dirent_t  *e = &asa_dst->op.scandir.fileinfo.data[ curr_rd_idx ];
    size_t  fullpath_sz = strlen(asa_dst->op.scandir.path) + 1 + strlen(e->name) + 1 ;
    char  fullpath[fullpath_sz];
    size_t nwrite = snprintf(&fullpath[0], fullpath_sz, "%s/%s", asa_dst->op.scandir.path, e->name);
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    asa_dst->op.unlink.path = &fullpath[0];
    asa_dst->op.unlink.cb   =  cb;
    ASA_RES_CODE result =  asa_dst->storage->ops.fn_unlink(asa_dst);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string(
           "[storage] failed to issue unlink operation for removing files"));
        atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        fprintf(stderr, "[transcoder][video][storage] error, line:%d, version:%s, result:%d \n",
                __LINE__, processor->data.version, result );
    }
} // end of  _atfp_remove_version_unlinkfile_start


static  void  _atfp_remove_version_unlinkfile_done(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint32_t  max_num_files = asa_dst->op.scandir.fileinfo.size;
        uint32_t  curr_rd_idx   = asa_dst->op.scandir.fileinfo.rd_idx;
        if(curr_rd_idx < max_num_files) {
            _atfp_remove_version_unlinkfile_start (asa_dst, err_info, _atfp_remove_version_unlinkfile_done);
        } else {
            _atfp_remove_version_rmdir_start(asa_dst, err_info);
        }
    } else {
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to scan folder path for removing files"));
        fprintf(stderr, "[transcoder][video][storage] error, line:%d, version:%s, result:%d \n",
                __LINE__, processor->data.version, result );
    } // TODO, cleanup internal alloc memory in asa_dst
    if(err_info && json_object_size(err_info) > 0)
        _atfp_remove_version_dealloc(processor);
} // end of _atfp_remove_version_unlinkfile_done


static  void  _atfp_remove_version_scandir_done(asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_t *processor = asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t num_files = asa_dst->op.scandir.fileinfo.size;
        if(num_files > 0) {
            int err = atfp_scandir_load_fileinfo (asa_dst, err_info);
            if(!err)
                _atfp_remove_version_unlinkfile_start (asa_dst, err_info, _atfp_remove_version_unlinkfile_done);
        } else {
            _atfp_remove_version_rmdir_start(asa_dst, err_info);
        }
    } else {
        json_object_set_new(err_info, "transcode", json_string(
               "[storage] failed to scan folder path for removing files"));
        fprintf(stderr, "[transcoder][video][storage] error, line:%d, version:%s, result:%d \n",
                __LINE__, processor->data.version, result );
    }
    if(err_info && json_object_size(err_info) > 0)
        _atfp_remove_version_dealloc(processor);
} // end of _atfp_remove_version_scandir_done


void  atfp_storage_video_remove_version(atfp_t *processor, const char *status)
{
    asa_op_base_cfg_t *asa_dst = processor ->data.storage.handle;
    json_t *err_info = processor->data.error;
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
    asa_dst->op.scandir.path = strdup(&fullpath[0]);
    asa_dst->op.scandir.cb   = _atfp_remove_version_scandir_done;
    ASA_RES_CODE  result =  asa_dst->storage->ops.fn_scandir(asa_dst);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string(
                "[storage] failed to issue scandir operation for removing files"));
        fprintf(stderr, "[transcoder][video][storage] error, line:%d, result:%d, scan path:%s \n",
                __LINE__, result, asa_dst->op.scandir.path );
        _atfp_remove_version_dealloc(processor);
    }
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

