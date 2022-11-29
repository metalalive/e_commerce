#ifndef MEDIA__ACL_H
#define MEDIA__ACL_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o/memory.h>
#include "models/datatypes.h"

typedef struct {
    uint32_t  usr_id;
    struct {
        uint8_t  renew:1;
        uint8_t  edit_acl:1;
        uint8_t  transcode:1;
    } capability;
} aacl_data_t;

typedef struct {
    H2O_VECTOR(aacl_data_t) data;
    struct {
        uint8_t error:1;
        uint8_t write_ok:1;
    } flag;
} aacl_result_t;

#define  APP_ACL_CFG__COMMON_FIELDS \
    char  *resource_id; \
    void  *usrdata; \
    uint32_t  usr_id; \
    void (*callback)(aacl_result_t *, void *usr_data);

typedef struct {
    APP_ACL_CFG__COMMON_FIELDS
    db_pool_t *db_pool;
    void  *loop;
} aacl_cfg_t;

int  app_resource_acl_load(aacl_cfg_t *);

int  app_resource_acl_save(aacl_cfg_t *, aacl_result_t *existing_data, json_t *new_data);

void  app_acl__build_update_lists (aacl_result_t *existing_data, json_t *new_data,
         aacl_data_t **data_update, size_t *num_update,
         aacl_data_t **data_delete, size_t *num_deletion,
         aacl_data_t  *data_insert, size_t *num_insertion );

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__ACL_H
