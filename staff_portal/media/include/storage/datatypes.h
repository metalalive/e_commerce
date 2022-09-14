#ifndef MEDIA__STORAGE_DATATYPES_H
#define MEDIA__STORAGE_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif
#include <stddef.h>

typedef enum {
    ASTORAGE_RESULT_COMPLETE = 1,
    ASTORAGE_RESULT_ACCEPT,
    ASTORAGE_RESULT_EOF_SCAN, // end of file
    ASTORAGE_RESULT_UNKNOWN_ERROR,
    ASTORAGE_RESULT_ARG_ERROR,
    ASTORAGE_RESULT_OS_ERROR,
    ASTORAGE_RESULT_DATA_ERROR
} ASA_RES_CODE;

struct _asa_cfg_s;
struct _asa_op_base_cfg_s;

typedef void (*asa_mkdir_cb_t) (struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_rmdir_cb_t) (struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_scandir_cb_t)(struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_rename_cb_t)(struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_unlink_cb_t)(struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_open_cb_t) (struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_close_cb_t)(struct _asa_op_base_cfg_s *, ASA_RES_CODE);
typedef void (*asa_seek_cb_t) (struct _asa_op_base_cfg_s *, ASA_RES_CODE, size_t pos);
typedef void (*asa_write_cb_t)(struct _asa_op_base_cfg_s *, ASA_RES_CODE, size_t nwrite);
typedef void (*asa_read_cb_t) (struct _asa_op_base_cfg_s *, ASA_RES_CODE, size_t nread);

struct _asa_op_base_cfg_s {
    struct {
        size_t size;
        void **entries;
    } cb_args;
    struct _asa_cfg_s  *storage;
    struct {
        struct {
            asa_mkdir_cb_t  cb;
            int    mode;
            struct {
                char *prefix;
                char *origin;
                char *curr_parent;
                char *tok_saveptr;
            } path;
            uint8_t  _allow_exists:1;
        } mkdir;
        struct { // delete an empty folder
            asa_rmdir_cb_t  cb;
            char  *path; 
        } rmdir;
        struct {
            asa_scandir_cb_t  cb;
            char  *path; 
        } scandir;
        struct { // move folder
            asa_rename_cb_t  cb;
            struct {
                char  *_new;
                char  *_old;
            } path;
        } rename;
        struct { // delete a file
            asa_unlink_cb_t  cb;
            char  *path; 
        } unlink;
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
            int offset;
        } write;
        struct {
            asa_read_cb_t cb;
            char *dst;
            size_t dst_sz;
            size_t dst_max_nbytes;
            int offset;
        } read;
        struct {
            asa_seek_cb_t cb;
            size_t pos;
        } seek;
    } op;
}; // end of struct _asa_op_base_cfg_s

typedef struct _asa_op_base_cfg_s asa_op_base_cfg_t;

typedef enum {
    ASA_DIRENT_UNKNOWN = 0,
    ASA_DIRENT_FILE,
    ASA_DIRENT_DIR,
    ASA_DIRENT_LINK,
} asa_dirent_type_t;

typedef struct {
    char *name;
    asa_dirent_type_t  type;
} asa_dirent_t;

typedef struct {
    ASA_RES_CODE (*fn_mkdir)(asa_op_base_cfg_t *, uint8_t  allow_exists);
    ASA_RES_CODE (*fn_rmdir)(asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_open) (asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_close)(asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_seek) (asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_write)(asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_read) (asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_unlink) (asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_rename) (asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_scandir)(asa_op_base_cfg_t *);
    ASA_RES_CODE (*fn_scandir_next)(asa_op_base_cfg_t *, asa_dirent_t *);
} asa_cfg_ops_t;

typedef struct _asa_cfg_s {
    char *alias;
    char *base_path;
    asa_cfg_ops_t  ops;
} asa_cfg_t;

#define APP_STORAGE_USE_CURRENT_FILE_OFFSET  -1

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__STORAGE_DATATYPES_H
