#include <stdlib.h>
#include <string.h>
#include <h2o/memory.h>
#include "storage/localfs.h"

static void _app_storage_localfs_open_cb(uv_fs_t *req) {
    const char *filepath = req->path;
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    if(req->result >= 0) { // return valid file descriptor, zero is possible if stdin is turned off
        app_result = ASTORAGE_RESULT_COMPLETE;
        req->file = req->result;
    } else {
        app_result = ASTORAGE_RESULT_OS_ERROR;
    }
    cfg->op.open.cb(cfg, app_result);
    if(filepath) {
        free((void *)filepath);
    } // libuv internally allocates extra space for file path
} // end of _app_storage_localfs_open_cb

ASA_RES_CODE app_storage_localfs_open (asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.open.cb || !cfg->op.open.dst_path)
        return ASTORAGE_RESULT_ARG_ERROR;
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    _cfg->file.file = -1; // TODO, may risk unclosed file-descriptor if application developers forgot 
    int err = uv_fs_open( _cfg->loop, &_cfg->file, cfg->op.open.dst_path,
            cfg->op.open.flags, cfg->op.open.mode, _app_storage_localfs_open_cb
        );
    if(err != 0) 
        result = ASTORAGE_RESULT_OS_ERROR;
    return result;
} // end of app_storage_localfs_open


static void _app_storage_localfs_close_cb(uv_fs_t *req) {
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result == 0)? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    req->file = -1;
    cfg->op.close.cb(cfg, app_result);
} // end of _app_storage_localfs_close_cb

ASA_RES_CODE app_storage_localfs_close(asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!cfg || !_cfg->loop || !cfg->op.close.cb) {
        return ASTORAGE_RESULT_ARG_ERROR;
    } else if (_cfg->file.file <= 0) {
        return ASTORAGE_RESULT_OS_ERROR;
    }
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    int err = uv_fs_close(_cfg->loop, &_cfg->file, _cfg->file.file, _app_storage_localfs_close_cb);
    if(err != 0) {
        result = ASTORAGE_RESULT_OS_ERROR;
    }
    return result;
} // end of app_storage_localfs_close


static ASA_RES_CODE  _app_storage_mkdir_nxt_parent(asa_op_base_cfg_t *cfg)
{
    char *tok = NULL;
    char *origin = NULL;
    uint8_t init_round = cfg->op.mkdir.path.curr_parent[0] == 0x0;
    if(init_round) {
        cfg->op.mkdir.path.tok_saveptr = NULL;
        origin = cfg->op.mkdir.path.origin;
        char *prefix = cfg->op.mkdir.path.prefix;
        if(prefix && prefix[0] != 0) {
            strcat(cfg->op.mkdir.path.curr_parent, prefix);
            strcat(cfg->op.mkdir.path.curr_parent, "/");
        }
    } else {
        if(!cfg->op.mkdir.path.tok_saveptr)
            return ASTORAGE_RESULT_ARG_ERROR;
        origin = NULL;
    }
    tok = strtok_r(origin, "/", &cfg->op.mkdir.path.tok_saveptr);
    if(!tok || strcmp(tok, ".") == 0 || strcmp(tok, "..") == 0)
        return ASTORAGE_RESULT_ARG_ERROR;
    if(!init_round)
        strcat(cfg->op.mkdir.path.curr_parent, "/");
    strcat(cfg->op.mkdir.path.curr_parent, tok);
    return ASTORAGE_RESULT_ACCEPT;
} // end of _app_storage_mkdir_nxt_parent


static void _app_storage_localfs_mkdir_cb(uv_fs_t *req) {
    const char *curr_path = req->path;
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    uint8_t  allow_exists = cfg->op.mkdir._allow_exists;
    uint8_t  op_ok = (req->result == 0) || ((req->result == UV_EEXIST) && (allow_exists));
    uint8_t  final_round = (cfg->op.mkdir.path.tok_saveptr[0] == 0x0) ||  (!op_ok);
    ASA_RES_CODE  app_result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    if(final_round) {
        size_t cpy_sz = strlen(cfg->op.mkdir.path.curr_parent); // recover destination path
        char *cpy_pos = cfg->op.mkdir.path.curr_parent;
        char *prefix = cfg->op.mkdir.path.prefix;
        if(prefix && prefix[0] != 0) {
            size_t prefix_sz = strlen(prefix) + 1; // one extra slash character
            cpy_pos += prefix_sz;
            cpy_sz  -= prefix_sz;
        }
        memcpy(cfg->op.mkdir.path.origin, cpy_pos, cpy_sz);
        cfg->op.mkdir.path.origin[cpy_sz] = 0;
    }
    if(op_ok) { // acceptable if parent folder already exists
        if(final_round) { // all essential parent folders are created
            app_result   = ASTORAGE_RESULT_COMPLETE;
            cfg->op.mkdir.cb(cfg, app_result);
        } else { // recursively create new subfolder
            app_result = app_storage_localfs_mkdir(cfg, allow_exists);
            if(app_result != ASTORAGE_RESULT_ACCEPT)
                cfg->op.mkdir.cb(cfg, app_result);
        }
    } else {
        app_result = ASTORAGE_RESULT_OS_ERROR;
        cfg->op.mkdir.cb(cfg, app_result);
    }
    if(curr_path) {
        free((void *)curr_path);
    } // libuv internally allocates extra space for file path
} // end of _app_storage_localfs_mkdir_cb


ASA_RES_CODE  app_storage_localfs_mkdir (asa_op_base_cfg_t *cfg, uint8_t  allow_exists)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.mkdir.cb || !cfg->op.mkdir.path.origin
            || !cfg->op.mkdir.path.curr_parent)
    { return ASTORAGE_RESULT_ARG_ERROR; }
    cfg->op.mkdir._allow_exists = allow_exists;
    ASA_RES_CODE result = _app_storage_mkdir_nxt_parent(cfg);
    if(result == ASTORAGE_RESULT_ACCEPT) {
        int err = uv_fs_mkdir(_cfg->loop, &_cfg->file, cfg->op.mkdir.path.curr_parent,
                cfg->op.mkdir.mode, _app_storage_localfs_mkdir_cb);
        if(err != 0) {
            result = ASTORAGE_RESULT_OS_ERROR;
        }
    }
    return result;
} // end of app_storage_localfs_mkdir


static void _app_storage_localfs_rmdir_cb(uv_fs_t *req) {
    const char *curr_path = req->path;
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result == 0)? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    cfg->op.rmdir.cb(cfg, app_result);
    if(curr_path) {
        free((void *)curr_path);
    } // libuv internally allocates extra space for file path
} // end of _app_storage_localfs_rmdir_cb

ASA_RES_CODE app_storage_localfs_rmdir (asa_op_base_cfg_t *cfg)
{ // TODO, recursively remove sub folders
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.rmdir.cb || !cfg->op.rmdir.path)
        return ASTORAGE_RESULT_ARG_ERROR;
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    int err = uv_fs_rmdir(_cfg->loop, &_cfg->file, cfg->op.rmdir.path
            , _app_storage_localfs_rmdir_cb);
    if(err != 0) {
        result = ASTORAGE_RESULT_OS_ERROR;
    }
    return result;
} // end of app_storage_localfs_rmdir


static void  _app_storage_localfs__scandir_cb(uv_fs_t *req)
{
    const char *path = req->path; // `new_path` and `path` fields were allocated together
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result >= 0) ? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    if(req->result >= 0) {
        cfg->op.scandir.fileinfo.size = req->result;
        cfg->op.scandir.fileinfo.rd_idx = 0;
    } // let app developers decide when to alloc/free memory for the result
    cfg->op.scandir.cb(cfg, app_result);
    if(path)
        free((void *)path);
}

ASA_RES_CODE  app_storage_localfs_scandir (asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.scandir.cb || !cfg->op.scandir.path)
        return ASTORAGE_RESULT_ARG_ERROR;
    if(cfg->op.scandir.fileinfo.data || cfg->op.scandir.fileinfo.size > 0)
        return ASTORAGE_RESULT_ARG_ERROR; // previous scan data should be cleaned
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    int err = uv_fs_scandir(_cfg->loop, &_cfg->file, cfg->op.scandir.path,
            0, _app_storage_localfs__scandir_cb);
    if(err != 0)
        result = ASTORAGE_RESULT_OS_ERROR;
    return result;
}

ASA_RES_CODE  app_storage_localfs_scandir_next (asa_op_base_cfg_t *cfg, asa_dirent_t *e)
{ 
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !e)
        return ASTORAGE_RESULT_ARG_ERROR;
    ASA_RES_CODE result = ASTORAGE_RESULT_COMPLETE;
    uv_dirent_t  ent = {0};
    int err = uv_fs_scandir_next(&_cfg->file, &ent);
    if(err == UV_EOF) {
        *e = (asa_dirent_t) {0};
        result = ASTORAGE_RESULT_EOF_SCAN;
    } else if(err == 0) {
        asa_dirent_type_t ftyp;
        switch(ent.type) {
            case UV_DIRENT_FILE:
                ftyp = ASA_DIRENT_FILE;
                break;
            case UV_DIRENT_DIR:
                ftyp = ASA_DIRENT_DIR;
                break;
            case UV_DIRENT_LINK:
                ftyp = ASA_DIRENT_LINK;
                break;
            default:
                ftyp = ASA_DIRENT_UNKNOWN;
                break;
        }
        *e = (asa_dirent_t) {.type=ftyp, .name=ent.name};
    } else {
        uv_fs_req_cleanup(&_cfg->file);
        result = ASTORAGE_RESULT_OS_ERROR;
    }
    return result;
} // end of app_storage_localfs_scandir_next

static void  _app_storage_localfs__rename_cb(uv_fs_t *req)
{
    const char *path = req->path; // `new_path` and `path` fields were allocated together
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result == 0)? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    cfg->op.rename.cb(cfg, app_result);
    if(path)
        free((void *)path);
}

ASA_RES_CODE  app_storage_localfs_rename(asa_op_base_cfg_t *cfg)
{ 
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.rename.cb || !cfg->op.rename.path._new
             || !cfg->op.rename.path._old)
        return ASTORAGE_RESULT_ARG_ERROR;
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    int err = uv_fs_rename(_cfg->loop, &_cfg->file, cfg->op.rename.path._old,
            cfg->op.rename.path._new, _app_storage_localfs__rename_cb);
    if(err != 0)
        result = ASTORAGE_RESULT_OS_ERROR;
    return result;
}


static void _app_storage_localfs_unlink_cb(uv_fs_t *req) {
    const char *curr_path = req->path;
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result == 0)? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    if(cfg->op.unlink.cb)
        cfg->op.unlink.cb(cfg, app_result);
    if(curr_path) 
        free((void *)curr_path);
} // end of _app_storage_localfs_unlink_cb

ASA_RES_CODE app_storage_localfs_unlink (asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.unlink.path)
        return ASTORAGE_RESULT_ARG_ERROR;
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    int err = uv_fs_unlink(_cfg->loop, &_cfg->file, cfg->op.unlink.path,
                _app_storage_localfs_unlink_cb);
    if(err != 0)
        result = ASTORAGE_RESULT_OS_ERROR;
    return result;
} // end of app_storage_localfs_unlink


#define NUM_BUFS      1
static void _app_storage_localfs_read_cb(uv_fs_t *req) {
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result >= 0) ? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    size_t nread = (req->result >= 0) ? req->result : 0;
    if(app_result == ASTORAGE_RESULT_COMPLETE) {
        if(cfg->op.read.offset >= 0)
            cfg->op.seek.pos = cfg->op.read.offset;
        cfg->op.seek.pos += nread;
    }
    cfg->op.read.cb(cfg, app_result, nread);
} // end of _app_storage_localfs_read_cb

ASA_RES_CODE app_storage_localfs_read (asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.read.cb || !cfg->op.read.dst
            || cfg->op.read.dst_sz == 0 || cfg->op.read.dst_max_nbytes == 0
            || cfg->op.read.dst_sz > cfg->op.read.dst_max_nbytes) {
        return ASTORAGE_RESULT_ARG_ERROR;
    }
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    const uv_buf_t bufs[NUM_BUFS] = {{.base = cfg->op.read.dst, .len = cfg->op.read.dst_sz}};
    int err = uv_fs_read(_cfg->loop, &_cfg->file, _cfg->file.file, bufs, NUM_BUFS,
                 cfg->op.read.offset, _app_storage_localfs_read_cb );
    if(err != 0) {
        result = ASTORAGE_RESULT_OS_ERROR;
    }
    return result;
} // end of app_storage_localfs_read


static void _app_storage_localfs_write_cb(uv_fs_t *req) {
    asa_op_base_cfg_t *cfg = (asa_op_base_cfg_t *) H2O_STRUCT_FROM_MEMBER(asa_op_localfs_cfg_t, file, req);
    ASA_RES_CODE  app_result = (req->result > 0) ? ASTORAGE_RESULT_COMPLETE: ASTORAGE_RESULT_OS_ERROR;
    size_t nwrite = req->result;
    if(app_result == ASTORAGE_RESULT_COMPLETE) {
        if(cfg->op.write.offset >= 0)
            cfg->op.seek.pos = cfg->op.write.offset;
        cfg->op.seek.pos += cfg->op.write.src_sz;
    }
    cfg->op.write.cb(cfg, app_result, nwrite);
} // end of _app_storage_localfs_write_cb

ASA_RES_CODE app_storage_localfs_write(asa_op_base_cfg_t *cfg)
{
    asa_op_localfs_cfg_t *_cfg = (asa_op_localfs_cfg_t *) cfg;
    if(!_cfg || !_cfg->loop || !cfg->op.write.cb || !cfg->op.write.src
        || cfg->op.write.src_sz == 0 || cfg->op.write.src_max_nbytes == 0
        || cfg->op.write.src_sz > cfg->op.write.src_max_nbytes) {
        return ASTORAGE_RESULT_ARG_ERROR;
    }
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    const uv_buf_t bufs[NUM_BUFS] = {{.base = cfg->op.write.src, .len = cfg->op.write.src_sz}};
    int err = uv_fs_write(_cfg->loop, &_cfg->file, _cfg->file.file, bufs, NUM_BUFS,
                 cfg->op.write.offset, _app_storage_localfs_write_cb );
    if(err != 0) {
        result = ASTORAGE_RESULT_OS_ERROR;
    }
    return result;
} // end of app_storage_localfs_write
#undef NUM_BUFS

size_t   app_storage_localfs_typesize(void)
{ return sizeof(asa_op_localfs_cfg_t); }

ASA_RES_CODE app_storage_localfs_seek (asa_op_base_cfg_t *cfg)
{ //  TODO, remove, seek function doesn't seem appropriate for multi-threaded applications
  //  which requires to access the same file
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    return result;
} // end of app_storage_localfs_seek
