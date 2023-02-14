#ifndef MEDIA__ACL_H
#define MEDIA__ACL_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o/memory.h>
#include "datatypes.h"
#include "models/datatypes.h"

typedef struct {
    uint32_t  usr_id;
    struct {
        uint8_t  edit_acl:1;
        uint8_t  transcode:1;
    } capability;
} aacl_data_t;

typedef struct {
    H2O_VECTOR(aacl_data_t) data;
    uint32_t  owner_usr_id;
    uint32_t  upld_req;
    char      type[sizeof(APP_FILETYPE_LABEL_VIDEO)];
    struct {
        uint8_t error:1; // TODO, rename to db_error
        uint8_t write_ok:1;
        uint8_t res_id_exists:1;
        uint8_t res_id_dup:1;
        uint8_t acl_exists:1;  // indicates status of file-level ACL
        uint8_t acl_visible:1; // means it is visible to everyone (even for anonymous clients)
    } flag;
} aacl_result_t;

#define  APP_ACL_CFG__COMMON_FIELDS \
    char  *resource_id; \
    uint32_t  usr_id; \
    void (*callback)(aacl_result_t *, void **usr_args); \
    uint8_t   fetch_acl:1;

typedef struct {
    APP_ACL_CFG__COMMON_FIELDS
    struct {
        void   **entries;
        uint16_t size;
    } usr_args;
    db_pool_t *db_pool;
    void  *loop;
} aacl_cfg_t;

int  app_acl_verify_resource_id (aacl_cfg_t *);

int  app_resource_acl_load(aacl_cfg_t *);

int  app_usrlvl_acl_save(aacl_cfg_t *, aacl_result_t *existing_data, json_t *new_data);

void  app_acl__build_update_lists (aacl_result_t *existing_data, json_t *new_data,
         aacl_data_t **data_update, size_t *num_update,
         aacl_data_t **data_delete, size_t *num_deletion,
         aacl_data_t  *data_insert, size_t *num_insertion );

int  app_filelvl_acl_save(aacl_cfg_t *, json_t *existing_data, json_t *new_data);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__ACL_H
