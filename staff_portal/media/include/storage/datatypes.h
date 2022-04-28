#ifndef MEDIA__STORAGE_DATATYPES_H
#define MEDIA__STORAGE_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif
#include <stddef.h>

typedef enum {
    ASTORAGE_RESULT_COMPLETE = 1,
    ASTORAGE_RESULT_ACCEPT,
    ASTORAGE_RESULT_UNKNOWN_ERROR,
    ASTORAGE_RESULT_ARG_ERROR,
    ASTORAGE_RESULT_OS_ERROR,
    ASTORAGE_RESULT_DATA_ERROR
} ASA_RES_CODE;

struct _asa_op_base_cfg_s;

typedef void (*asa_mkdir_cb_t)(struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result);
typedef void (*asa_rmdir_cb_t)(struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result);
typedef void (*asa_open_cb_t) (struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result);
typedef void (*asa_close_cb_t)(struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result);
typedef void (*asa_seek_cb_t) (struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result, size_t pos);
typedef void (*asa_write_cb_t)(struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result, size_t nwrite);
typedef void (*asa_read_cb_t) (struct _asa_op_base_cfg_s *cfg, ASA_RES_CODE result, size_t nread);

struct _asa_op_base_cfg_s {
    struct {
        size_t size;
        void **entries;
    } cb_args;
    struct {
        struct {
            asa_mkdir_cb_t  cb;
            int    mode;
            struct {
                char *origin;
                char *curr_parent;
                char *tok_saveptr;
            } path;
        } mkdir;
        struct {
            asa_rmdir_cb_t  cb;
            char  *path; 
        } rmdir;
        struct {
            asa_open_cb_t  cb;
            char  *dst_path; 
            int    mode;
            int    flags;
        } open;
        struct {
            asa_close_cb_t cb;
        } close;
        struct {
            asa_write_cb_t cb;
            char *src;
            size_t src_sz;
            size_t src_max_nbytes;
        } write;
        struct {
            asa_read_cb_t cb;
            char *dst;
            size_t dst_sz;
        } read;
        struct {
            asa_seek_cb_t cb;
            size_t pos;
        } seek;
    } op;
}; // end of struct _asa_op_base_cfg_s

typedef struct _asa_op_base_cfg_s asa_op_base_cfg_t;

typedef struct {
    ASA_RES_CODE (*fn_mkdir)(asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_rmdir)(asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_open) (asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_close)(asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_seek) (asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_write)(asa_op_base_cfg_t *cfg);
    ASA_RES_CODE (*fn_read) (asa_op_base_cfg_t *cfg);
} asa_cfg_ops_t;

typedef struct {
    char *alias;
    char *base_path;
    asa_cfg_ops_t  ops;
} asa_cfg_t;

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__STORAGE_DATATYPES_H
