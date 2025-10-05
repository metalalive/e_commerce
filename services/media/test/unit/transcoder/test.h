#ifndef MEDIA__UTEST_TRANSCODER__H
#define MEDIA__UTEST_TRANSCODER__H
#ifdef __cplusplus
extern "C" {
#endif

#define UTEST_RUN_OPERATION_WITH_PATH(_basepath, _usr_id, _upld_req_id, _filename, _cmd) \
    { \
        size_t nwrite = 0, _fname_sz = 0; \
        char  *__filename = _filename; \
        if (__filename != NULL) \
            _fname_sz = strlen(__filename); \
        size_t path_sz = strlen(_basepath) + 1 + USR_ID_STR_SIZE + 1 + UPLOAD_INT2HEX_SIZE(_upld_req_id) + \
                         1 + _fname_sz + 1; \
        char path[path_sz]; \
        if (_usr_id != 0 && _upld_req_id != 0 && __filename) { \
            nwrite = \
                snprintf(&path[0], path_sz, "%s/%d/%x/%s", _basepath, _usr_id, _upld_req_id, __filename); \
        } else if (_usr_id != 0 && _upld_req_id != 0 && !__filename) { \
            nwrite = snprintf(&path[0], path_sz, "%s/%d/%x", _basepath, _usr_id, _upld_req_id); \
        } else if (_usr_id != 0 && _upld_req_id == 0 && !__filename) { \
            nwrite = snprintf(&path[0], path_sz, "%s/%d", _basepath, _usr_id); \
        } \
        if (nwrite != 0) { \
            assert(path_sz >= nwrite); \
            _cmd(&path[0], nwrite) \
        } \
    }

#define UTEST_OPS_UNLINK(_path, _path_sz) \
    { unlink(_path); }
#define UTEST_OPS_RMDIR(_path, _path_sz) \
    { rmdir(_path); }
#define UTEST_OPS_MKDIR(_path, _path_sz) \
    { mkdir(_path, S_IRWXU); }
#define UTEST_OPS_WRITE2FILE(_path, _path_sz) \
    { \
        int fd = open(_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
        write(fd, _wr_buf, _wr_buf_sz); \
        close(fd); \
    }

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__UTEST_TRANSCODER__H
