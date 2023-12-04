#ifndef MEDIA__API_FILEFETCH_COMMON_H
#define MEDIA__API_FILEFETCH_COMMON_H
#ifdef __cplusplus
extern "C" {
#endif
#include "middleware.h"
#include "transcoder/file_processor.h"

typedef asa_op_localfs_cfg_t * (*cache_init_fn_t)(void *loop, json_t *spec, json_t *err_info,
       uint8_t num_cb_args, uint32_t buf_sz, asa_open_cb_t usr_init_cb, asa_close_cb_t usr_deinit_cb);

typedef void (*cache_proceed_fn_t)(asa_op_base_cfg_t *, asa_cch_proceed_cb_t);

int  api_filefetch_start_caching (h2o_req_t *, h2o_handler_t *, app_middleware_node_t *,
        json_t *spec, json_t *err_info, cache_init_fn_t, cache_proceed_fn_t);

void  api_init_filefetch__deinit_common (h2o_req_t *, h2o_handler_t *, app_middleware_node_t *,
        json_t *qparams, json_t *res_body);

int api_abac_pep__init_filefetch (h2o_handler_t *, h2o_req_t *, app_middleware_node_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEDIA__API_FILEFETCH_COMMON_H
