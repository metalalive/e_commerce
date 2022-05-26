#ifndef MEDIA__APP_CFG_H
#define MEDIA__APP_CFG_H
#ifdef __cplusplus
extern "C" {
#endif

#include "datatypes.h"

struct  worker_init_data_t{
    app_cfg_t  *app_cfg;
    uv_loop_t  *loop;
    unsigned int cfg_thrd_idx;
};

app_cfg_t *app_get_global_cfg(void);

void  app_global_cfg_set_exepath(const char *exe_path);

void deinit_app_cfg(app_cfg_t *app_cfg);

int appcfg_parse_pid_file(json_t *obj, app_cfg_t *_app_cfg);
int appcfg_parse_errlog_path(json_t *obj, app_cfg_t *acfg);
int appcfg_parse_num_workers(json_t *obj, app_cfg_t *acfg);

void appcfg_notify_all_workers(app_cfg_t *acfg);
int  appcfg_start_workers(app_cfg_t *app_cfg, struct worker_init_data_t *data, void (*entry)(void *));
void appcfg_terminate_workers(app_cfg_t *app_cfg, struct worker_init_data_t *data);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEDIA__APP_CFG_H
