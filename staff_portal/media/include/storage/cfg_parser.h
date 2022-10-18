#ifndef MEDIA__STORAGE_CFG_PARSER_H
#define MEDIA__STORAGE_CFG_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include "app_cfg.h"

void app_storage_cfg_deinit(app_cfg_t *app_cfg);

int parse_cfg_storages(json_t *objs, app_cfg_t *app_cfg);

asa_cfg_t * app_storage_cfg_lookup(const char *alias);

asa_op_base_cfg_t * app_storage__init_asaobj_helper (asa_cfg_t *storage, uint8_t num_cb_args,
        uint32_t rd_buf_bytes, uint32_t wr_buf_bytes );

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__STORAGE_CFG_PARSER_H
