#ifndef MEDIA__STORAGE_LOCALFS_H
#define MEDIA__STORAGE_LOCALFS_H
#ifdef __cplusplus
extern "C" {
#endif

#include <uv.h>
#include "storage/datatypes.h"

typedef struct {
    asa_op_base_cfg_t  super;
    uv_loop_t   *loop;
    uv_fs_t      file;
} asa_op_localfs_cfg_t;

ASA_RES_CODE app_storage_localfs_mkdir (asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_rmdir (asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_open (asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_close(asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_read (asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_write(asa_op_base_cfg_t *cfg);

ASA_RES_CODE app_storage_localfs_seek (asa_op_base_cfg_t *cfg);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__STORAGE_LOCALFS_H
