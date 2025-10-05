#include <fcntl.h>
#include <string.h>
#include <uuid/uuid.h>

#include "transcoder/file_processor.h"

#define PRINT_DST_PATH_STRING(out, o_sz, prefix, status_f, version) \
    { \
        size_t min_req_sz = strlen(prefix) + 1 + strlen(status_f) + 1 + strlen(version) + 1; \
        assert(min_req_sz <= o_sz); \
        memset(out, 0x0, o_sz); \
        strcat(out, prefix); \
        strcat(out, "/"); \
        strcat(out, status_f); \
        strcat(out, "/"); \
        strcat(out, version); \
        assert((out)[min_req_sz - 1] == 0x0); \
        assert((out)[o_sz - 1] == 0x0); \
    }

// NOTE: each destination file-processor is responsible to remove the version folder
//  under discarding folder, this project do not provide one-size-fit-all solution.
static void _atfp__move_transcoding_to_committed_cb(
    asa_op_base_cfg_t *asaobj, ASA_RES_CODE result
) { //  move transcoded files to committed folder
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(
            err_info, "transcode",
            json_string("[storage] failed to move from transcoding to committed folder")
        );
    }
    processor->data.callback(processor);
} // end of _atfp__move_transcoding_to_committed_cb

static void _atfp__move_committed_to_discarding_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t path_sz = strlen(asaobj->op.mkdir.path.prefix) + 3 + ATFP__MAXSZ_STATUS_FOLDER_NAME +
                         strlen(processor->data.version);
        char old_path[path_sz], new_path[path_sz];
        PRINT_DST_PATH_STRING(
            &new_path[0], path_sz, asaobj->op.mkdir.path.prefix, ATFP__COMMITTED_FOLDER_NAME,
            processor->data.version
        );
        PRINT_DST_PATH_STRING(
            &old_path[0], path_sz, asaobj->op.mkdir.path.prefix, ATFP__TEMP_TRANSCODING_FOLDER_NAME,
            processor->data.version
        );
        asaobj->op.rename.cb = _atfp__move_transcoding_to_committed_cb;
        asaobj->op.rename.path._new = &new_path[0];
        asaobj->op.rename.path._old = &old_path[0];
        result = asaobj->storage->ops.fn_rename(asaobj);
        if (result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(
                err_info, "transcode",
                json_string("[storage] failed to issue move operation for transcoded files")
            );
    } else {
        json_object_set_new(
            err_info, "transcode", json_string("[storage] failed to move from committed to discarding folder")
        );
    }
    if (json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of _atfp__move_committed_to_discarding_cb

static void _atfp__ensure_dst_committed_basepath_cb(
    asa_op_base_cfg_t *asaobj, ASA_RES_CODE result
) { // move committed files (if exists) to discarding folder
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint8_t _is_update = processor->transfer.transcoded_dst.flags.version_exists;
        size_t  path_sz = strlen(asaobj->op.mkdir.path.prefix) + 3 + ATFP__MAXSZ_STATUS_FOLDER_NAME +
                         strlen(processor->data.version);
        char old_path[path_sz], new_path[path_sz];
        PRINT_DST_PATH_STRING(
            &new_path[0], path_sz, asaobj->op.mkdir.path.prefix,
            ((_is_update) ? ATFP__DISCARDING_FOLDER_NAME : ATFP__COMMITTED_FOLDER_NAME),
            processor->data.version
        );
        PRINT_DST_PATH_STRING(
            &old_path[0], path_sz, asaobj->op.mkdir.path.prefix,
            ((_is_update) ? ATFP__COMMITTED_FOLDER_NAME : ATFP__TEMP_TRANSCODING_FOLDER_NAME),
            processor->data.version
        );
        if (_is_update) {
            asaobj->op.rename.cb = _atfp__move_committed_to_discarding_cb;
        } else {
            asaobj->op.rename.cb = _atfp__move_transcoding_to_committed_cb;
        }
        asaobj->op.rename.path._new = &new_path[0];
        asaobj->op.rename.path._old = &old_path[0];
        result = asaobj->storage->ops.fn_rename(asaobj);
        if (result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(
                err_info, "transcode",
                json_string("[storage] failed to issue move operation for transcoded files")
            );
    } else {
        json_object_set_new(
            err_info, "transcode", json_string("[storage] failed to create folder for committed files")
        );
    }
    if (json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of _atfp__ensure_dst_committed_basepath_cb

static void _atfp__ensure_dst_discarding_basepath_cb(
    asa_op_base_cfg_t *asaobj, ASA_RES_CODE result
) { // ensure committed folder
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_dst = asaobj;
        size_t             nwrite = sprintf(asa_dst->op.mkdir.path.origin, "%s", ATFP__COMMITTED_FOLDER_NAME);
        asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
        asa_dst->op.mkdir.path.curr_parent[0] = 0x0;   // reset for mkdir
        asa_dst->op.mkdir.cb = _atfp__ensure_dst_committed_basepath_cb;
        result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
        if (result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(
                processor->data.error, "transcode",
                json_string("[storage] failed to create folder for discarding files")
            );
    } else {
        json_object_set_new(
            processor->data.error, "transcode",
            json_string("[storage] failed to issue mkdir operation for committed files")
        );
    }
    if (json_object_size(processor->data.error) > 0)
        processor->data.callback(processor);
} // end of _atfp__ensure_dst_discarding_basepath_cb

void atfp_storage__commit_new_version(atfp_t *processor) { // ensure discarding folder
    asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
    size_t             nwrite = 0;
    assert(processor->data.error);
    assert(asa_dst->op.mkdir.path.prefix);
    assert(asa_dst->op.mkdir.path.origin);
    nwrite = sprintf(
        asa_dst->op.mkdir.path.prefix, "%s/%d/%08x", asa_dst->storage->base_path, processor->data.usr_id,
        processor->data.upld_req_id
    );
    asa_dst->op.mkdir.path.prefix[nwrite++] = 0x0; // NULL-terminated
    nwrite = sprintf(asa_dst->op.mkdir.path.origin, "%s", ATFP__DISCARDING_FOLDER_NAME);
    asa_dst->op.mkdir.path.origin[nwrite++] = 0x0; // NULL-terminated
    asa_dst->op.mkdir.path.curr_parent[0] = 0x0;   // reset for mkdir
    asa_dst->op.mkdir.mode = S_IRWXU;
    asa_dst->op.mkdir.cb = _atfp__ensure_dst_discarding_basepath_cb;
    ASA_RES_CODE result = asa_dst->storage->ops.fn_mkdir(asa_dst, 1);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            processor->data.error, "transcode",
            json_string("[storage] failed to issue mkdir operation for discarding files")
        );
        processor->data.callback(processor);
    }
} // end of atfp_storage__commit_new_version

ASA_RES_CODE
atfp_open_srcfile_chunk(asa_op_base_cfg_t *cfg, const char *basepath, int chunk_seq, asa_open_cb_t cb) {
#define MAX_INT32_DIGITS 10
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    { // update file path for each media segment, open the first file chunk
        size_t filepath_sz = strlen(basepath) + 1 + MAX_INT32_DIGITS + 1; // assume NULL-terminated string
        char   filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, "%s/%d", basepath, chunk_seq);
        filepath[nwrite++] = 0x0;
        if (cfg->op.open.dst_path)
            free(cfg->op.open.dst_path);
        cfg->op.open.dst_path = strndup(&filepath[0], nwrite);
    }
    cfg->op.open.cb = cb;
    cfg->op.open.mode = S_IRUSR;
    cfg->op.open.flags = O_RDONLY;
    result = cfg->storage->ops.fn_open(cfg);
    return result;
#undef MAX_INT32_DIGITS
} // end of  atfp_open_srcfile_chunk

static void
atfp__close_curr_srcfchunk_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) { // only for source filechunk
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    uint8_t err = result != ASTORAGE_RESULT_COMPLETE;
    if (!err) {
        int next_chunk_seq = (int)processor->filechunk_seq.next + 1;
        result = atfp_open_srcfile_chunk(
            asaobj, processor->data.storage.basepath, next_chunk_seq, asaobj->op.open.cb
        );
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if (err) {
        processor->filechunk_seq.usr_cb(asaobj, result);
    }
}

static void
atfp__open_next_srcfchunk_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) { // only for source filechunk
    atfp_t *processor = (atfp_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        processor->filechunk_seq.curr = processor->filechunk_seq.next;
        processor->filechunk_seq.eof_reached = 0;
    }
    processor->filechunk_seq.usr_cb(asaobj, result);
}

ASA_RES_CODE atfp_switch_to_srcfile_chunk(
    atfp_t *processor, int chunk_seq, asa_open_cb_t cb
) { // close current filechunk then optionally open the next one if exists.
    ASA_RES_CODE result;
    json_t      *filechunks_size = json_object_get(processor->data.spec, "parts_size");
    uint32_t     final_filechunk_id = json_array_size(filechunks_size) - 1;
    uint32_t     next_filechunk_id = (chunk_seq < 0) ? (processor->filechunk_seq.curr + 1) : chunk_seq;
    if (final_filechunk_id >= next_filechunk_id) {
        asa_op_base_cfg_t *cfg = processor->data.storage.handle;
        cfg->op.close.cb = atfp__close_curr_srcfchunk_cb;
        cfg->op.open.cb = atfp__open_next_srcfchunk_cb;
        processor->filechunk_seq.next = next_filechunk_id;
        processor->filechunk_seq.usr_cb = cb;
        result = cfg->storage->ops.fn_close(cfg);
    } else {
        result = ASTORAGE_RESULT_DATA_ERROR;
    }
    return result;
} // end of atfp_switch_to_srcfile_chunk

int atfp_estimate_src_filechunk_idx(json_t *spec, int chunk_idx_start, size_t *pos) {
    json_t *fchunks_sz = json_object_get(spec, "parts_size");
    size_t  max_num_fchunks = (size_t)json_array_size(fchunks_sz);
    int     chunk_idx_dst = chunk_idx_start;
    size_t  fread_offset = *pos;
    for (; chunk_idx_dst < max_num_fchunks; chunk_idx_dst++) {
        size_t chunk_sz = (size_t)json_integer_value(json_array_get(fchunks_sz, chunk_idx_dst));
        if (fread_offset > chunk_sz) {
            fread_offset -= chunk_sz;
        } else {
            break;
        }
    }
    if (chunk_idx_dst < max_num_fchunks) {
        *pos = fread_offset;
    } else { // destination file chunk NOT found
        chunk_idx_dst = -1;
    }
    return chunk_idx_dst;
} // end of atfp_estimate_src_filechunk_idx

ASA_RES_CODE atfp_src__open_localbuf(asa_op_base_cfg_t *asa_src, asa_open_cb_t cb) {
    atfp_asa_map_t       *map = asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(map);
    const char           *local_tmpbuf_basepath = asa_local->super.op.mkdir.path.origin;
#define LOCAL_BUFFER_FILENAME "local_buffer"
#define UUID_STR_SZ           36
#define PATH_PATTERN          "%s/%s-%s"
    { // in case frontend client sent 2 requests which indicate the same source file
        char   _uid_postfix[UUID_STR_SZ + 1] = {0};
        uuid_t _uuid_obj;
        uuid_generate_random(_uuid_obj);
        uuid_unparse(_uuid_obj, &_uid_postfix[0]);
        size_t tmpbuf_basepath_sz = strlen(local_tmpbuf_basepath);
        size_t tmpbuf_filename_sz = strlen(LOCAL_BUFFER_FILENAME);
        size_t tmpbuf_fullpath_sz =
            sizeof(PATH_PATTERN) + tmpbuf_basepath_sz + tmpbuf_filename_sz + UUID_STR_SZ;
        char *ptr = calloc(tmpbuf_fullpath_sz, sizeof(char));
        asa_local->super.op.open.dst_path = ptr;
        size_t nwrite = snprintf(
            ptr, tmpbuf_fullpath_sz, PATH_PATTERN, local_tmpbuf_basepath, LOCAL_BUFFER_FILENAME,
            &_uid_postfix[0]
        );
        assert(nwrite < tmpbuf_fullpath_sz);
    }
#undef UUID_STR_SZ
#undef PATH_PATTERN
#undef LOCAL_BUFFER_FILENAME
    asa_local->super.op.open.cb = cb;
    asa_local->super.op.open.mode = S_IRUSR | S_IWUSR;
    asa_local->super.op.open.flags = O_RDWR | O_CREAT;
    return asa_local->super.storage->ops.fn_open(&asa_local->super);
} // end of  atfp_src__open_localbuf

int atfp_src__rd4localbuf_done_cb(
    asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread, asa_write_cb_t write_cb
) {
    atfp_t *processor = (atfp_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_asa_map_t       *_map = (atfp_asa_map_t *)asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(_map);
        asa_local->super.op.write.src = asa_src->op.read.dst;
        asa_local->super.op.write.src_sz = nread;
        asa_local->super.op.write.src_max_nbytes = nread;
        asa_local->super.op.write.offset = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
        asa_local->super.op.write.cb = write_cb;
        processor->filechunk_seq.eof_reached = asa_src->op.read.dst_sz > nread;
        result = asa_local->super.storage->ops.fn_write(&asa_local->super);
        if (result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(
                err_info, "storage", json_string("failed to issue write operation for atom body")
            );
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to read atom body from mp4 input"));
    }
    return json_object_size(err_info) > 0;
}

#define DEALLOC_IF_EXISTS(var, fn_name) \
    if (var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static void _atfp_remote_rmdir_g_finalize(atfp_t *processor) {
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    asa_remote->op.unlink.path = NULL;
    asa_remote->op.rmdir.path = NULL;
    DEALLOC_IF_EXISTS(asa_remote->op.scandir.path, free);
    if (asa_remote->op.scandir.fileinfo.data) {
        for (int idx = 0; idx < asa_remote->op.scandir.fileinfo.size; idx++) {
            asa_dirent_t *e = &asa_remote->op.scandir.fileinfo.data[idx];
            DEALLOC_IF_EXISTS(e->name, free);
        }
    }
    DEALLOC_IF_EXISTS(asa_remote->op.scandir.fileinfo.data, free);
    asa_remote->op.scandir.fileinfo.size = 0;
    processor->data.callback(processor);
}

static void _atfp_remote_rmdir_g_rmdir_done(asa_op_base_cfg_t *asa_remote, ASA_RES_CODE result) {
    atfp_t *processor = asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result != ASTORAGE_RESULT_COMPLETE) {
        json_object_set_new(err_info, "transcode", json_string("[storage] failed to remove folder"));
        fprintf(
            stderr, "[transcoder][storage] error, line:%d, rmdir path:%s, result:%d \n", __LINE__,
            asa_remote->op.scandir.path, result
        );
    }
    _atfp_remote_rmdir_g_finalize(processor);
}

static void _atfp_remote_rmdir_g_rmdir_start(asa_op_base_cfg_t *asa_remote, json_t *err_info) {
    asa_remote->op.rmdir.path = asa_remote->op.scandir.path;
    asa_remote->op.rmdir.cb = _atfp_remote_rmdir_g_rmdir_done;
    ASA_RES_CODE result = asa_remote->storage->ops.fn_rmdir(asa_remote);
    asa_remote->op.rmdir.path = NULL;
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(err_info, "transcode", json_string("[storage] failed to issue rmdir operation"));
        fprintf(
            stderr, "[transcoder][storage] line:%d, rmdir path:%s, result:%d \n", __LINE__,
            asa_remote->op.scandir.path, result
        );
    }
}

static void
_atfp_remote_rmdir_g_unlinkfile_start(asa_op_base_cfg_t *asa_remote, json_t *err_info, asa_unlink_cb_t cb) {
    uint32_t max_num_files = asa_remote->op.scandir.fileinfo.size;
    uint32_t curr_rd_idx = asa_remote->op.scandir.fileinfo.rd_idx++;
    assert(max_num_files > curr_rd_idx);
    asa_dirent_t *e = &asa_remote->op.scandir.fileinfo.data[curr_rd_idx];
    size_t        fullpath_sz = strlen(asa_remote->op.scandir.path) + 1 + strlen(e->name) + 1;
    char          fullpath[fullpath_sz];
    size_t        nwrite = snprintf(&fullpath[0], fullpath_sz, "%s/%s", asa_remote->op.scandir.path, e->name);
    fullpath[nwrite++] = 0x0; // NULL-terminated
    assert(nwrite <= fullpath_sz);
    asa_remote->op.unlink.path = &fullpath[0];
    asa_remote->op.unlink.cb = cb;
    ASA_RES_CODE result = asa_remote->storage->ops.fn_unlink(asa_remote);
    asa_remote->op.unlink.path = NULL;
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            err_info, "transcode",
            json_string("[storage] failed to issue unlink operation for removing files")
        );
        fprintf(
            stderr, "[transcoder][storage] error, line:%d, result:%d, unlink path:%s, type:%d \n", __LINE__,
            result, &fullpath[0], e->type
        );
    }
} // end of  _atfp_remote_rmdir_g_unlinkfile_start

static void _atfp_remote_rmdir_g_unlinkfile_done(asa_op_base_cfg_t *asa_remote, ASA_RES_CODE result) {
    atfp_t *processor = asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        uint32_t max_num_files = asa_remote->op.scandir.fileinfo.size;
        uint32_t curr_rd_idx = asa_remote->op.scandir.fileinfo.rd_idx;
        if (curr_rd_idx < max_num_files) {
            _atfp_remote_rmdir_g_unlinkfile_start(asa_remote, err_info, _atfp_remote_rmdir_g_unlinkfile_done);
        } else {
            _atfp_remote_rmdir_g_rmdir_start(asa_remote, err_info);
        }
    } else {
        uint32_t      curr_rd_idx = asa_remote->op.scandir.fileinfo.rd_idx - 1;
        asa_dirent_t *e = &asa_remote->op.scandir.fileinfo.data[curr_rd_idx];
        json_object_set_new(
            err_info, "transcode",
            json_string("[storage] failed to"
                        " unlink file for removing folder")
        );
        fprintf(
            stderr, "[transcoder][storage] error, line:%d, result:%d, unlink path:%s/%s, type:%d \n",
            __LINE__, result, asa_remote->op.scandir.path, e->name, e->type
        );
    }
    if (err_info && json_object_size(err_info) > 0)
        _atfp_remote_rmdir_g_finalize(processor);
} // end of _atfp_remote_rmdir_g_unlinkfile_done

static void _atfp_remote_rmdir_g_prescan_done(asa_op_base_cfg_t *asa_remote, ASA_RES_CODE result) {
    atfp_t *processor = asa_remote->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        size_t num_files = asa_remote->op.scandir.fileinfo.size;
        if (num_files > 0) {
            int err = atfp_scandir_load_fileinfo(asa_remote, err_info);
            if (!err)
                _atfp_remote_rmdir_g_unlinkfile_start(
                    asa_remote, err_info, _atfp_remote_rmdir_g_unlinkfile_done
                );
        } else {
            _atfp_remote_rmdir_g_rmdir_start(asa_remote, err_info);
        }
    } else {
        json_object_set_new(
            err_info, "transcode", json_string("[storage] failed to scan folder path for removing files")
        );
        fprintf(
            stderr, "[transcoder][storage] error, line:%d, result:%d, scan path:%s \n", __LINE__, result,
            asa_remote->op.scandir.path
        );
    }
    if (err_info && json_object_size(err_info) > 0)
        _atfp_remote_rmdir_g_finalize(processor);
} // end of _atfp_remote_rmdir_g_prescan_done

void atfp_remote_rmdir_generic(atfp_t *processor, const char *fullpath) {
    asa_op_base_cfg_t *asa_remote = processor->data.storage.handle;
    assert(asa_remote->op.scandir.path == NULL);
    assert(asa_remote->op.rmdir.path == NULL);
    assert(asa_remote->op.unlink.path == NULL);
    asa_remote->op.scandir.path = strdup(fullpath);
    asa_remote->op.scandir.cb = _atfp_remote_rmdir_g_prescan_done;
    ASA_RES_CODE result = asa_remote->storage->ops.fn_scandir(asa_remote);
    if (result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(
            processor->data.error, "transcode",
            json_string("[storage] failed to "
                        "issue scandir operation for removing folder")
        );
        fprintf(
            stderr, "[transcoder][storage] error, line:%d, result:%d, scan path:%s \n", __LINE__, result,
            fullpath
        );
        _atfp_remote_rmdir_g_finalize(processor);
    }
}
