#include <sysexits.h>
#include <h2o.h>
#include "app_cfg.h"
#include "utils.h"
#include "models/pool.h"
#include "transcoder/cfg_parser.h"

#ifdef TCP_FASTOPEN
    #define APP_DEFAULT_LENGTH_TCP_FASTOPEN_QUEUE 150
#else
    #define APP_DEFAULT_LENGTH_TCP_FASTOPEN_QUEUE 0
#endif

#define APP_DEFAULT_MAX_CONNECTIONS 1024

#define MAX_PERIOD_KEEP_JWKS_IN_SECONDS 3600 // 1 hour

static app_cfg_t _app_cfg = {
    .server_glb_cfg =
        {
            .hosts = NULL,
            .http2 = {0},
            .mimemap = NULL,
        },
    .listeners = NULL,
    .num_listeners = 0,
    .pid_file = NULL,
    .access_logger = NULL,
    .error_log_fd = -1,
    .max_connections = APP_DEFAULT_MAX_CONNECTIONS,
    .run_mode = RUN_MODE_MASTER,
    .tfo_q_len = APP_DEFAULT_LENGTH_TCP_FASTOPEN_QUEUE,
    .launch_time = 0,
    .workers_sync_barrier =
        {._mutex = PTHREAD_MUTEX_INITIALIZER,
         ._cond = PTHREAD_COND_INITIALIZER,
         ._count = 0,
         ._out_of_wait = 0},
    .storages = {.size = 0, .capacity = 0, .entries = NULL},
    .state = {.num_curr_sessions = 0},
    .jwks =
        {.handle = NULL,
         .src_url = NULL,
         .last_update = 0,
         .is_rotating = ATOMIC_FLAG_INIT,
         .ca_path = NULL,
         .ca_format = NULL,
         .max_expiry_secs = MAX_PERIOD_KEEP_JWKS_IN_SECONDS},
    .workers = {.size = 0, .capacity = 0, .entries = NULL},
    .rpc = {.size = 0, .capacity = 0, .entries = NULL},
    .exe_path = NULL,
    .shutdown_requested = 0,
    .env_vars = {0},
}; // end of _app_cfg

void app_load_envvars(app_envvars_t *env_vars) {
    if (!env_vars || env_vars->inited) {
        return;
    }
    env_vars->sys_base_path = getenv("SYS_BASE_PATH");
    env_vars->db_host = getenv("DB_HOST");

    const char *db_port_str = getenv("DB_PORT");
    if (db_port_str) {
        env_vars->db_port = (uint16_t)atoi(db_port_str);
    }
    env_vars->inited = 1;
}

app_cfg_t *app_get_global_cfg(void) {
    app_load_envvars(&_app_cfg.env_vars);
    return &_app_cfg;
}

void app_global_cfg_set_exepath(const char *exe_path
) { // caller should ensure this function is invoked each time with single thread
    _app_cfg.exe_path = exe_path;
}

void deinit_app_cfg(app_cfg_t *app_cfg) {
    app_db_pool_map_deinit();
    app_transcoder_cfg_deinit(&app_cfg->transcoder);
    if (app_cfg->pid_file) {
        fclose(app_cfg->pid_file);
        app_cfg->pid_file = NULL;
    }
    if (app_cfg->workers.entries) {
        free(app_cfg->workers.entries);
        app_cfg->workers.entries = NULL;
    }
    if (app_cfg->error_log_fd > 0) {
        close(app_cfg->error_log_fd);
        app_cfg->error_log_fd = -1;
    } // should be done lastly
} // end of deinit_app_cfg

// side effect of this function is that it left pid file opened, be sure to close the file on
// program exit
int appcfg_parse_pid_file(json_t *obj, app_cfg_t *acfg) {
    FILE *_file = NULL;
    if (json_is_string(obj) && acfg) {
        const char *basepath = acfg->env_vars.sys_base_path;
        const char *pid_file_str = json_string_value(obj);
#define RUNNER(fullpath) fopen(fullpath, "w+")
        _file = PATH_CONCAT_THEN_RUN(basepath, pid_file_str, RUNNER);
#undef RUNNER
        if (_file) {
            fprintf(_file, "%d\n", (int)getpid());
            fflush(_file);
            acfg->pid_file = _file;
        } else {
            return EX_NOINPUT; // from <sysexits.h>
        }
    }
    return (_file ? EX_OK : EX_CONFIG);
} // TODO, remove pid file on program exit

int appcfg_parse_errlog_path(json_t *obj, app_cfg_t *acfg) {
    int fd = -1;
    if (json_is_string(obj) && acfg) {
        const char *basepath = acfg->env_vars.sys_base_path;
        const char *err_log_path = json_string_value(obj);
#define RUNNER(fullpath) h2o_access_log_open_log(fullpath)
        fd = PATH_CONCAT_THEN_RUN(basepath, err_log_path, RUNNER);
#undef RUNNER
        if (fd != -1) { // redirect stdout and stderr to error log
            int fd_stdout = 1, fd_stderr = 2;
            if (dup2(fd, fd_stdout) == -1 || dup2(fd, fd_stderr) == -1) {
                close(fd);
                fd = -1;
            } else {
                acfg->error_log_fd = fd;
            } // TODO, close error log fd later at some point
        }
    }
    return ((fd > 0) ? EX_OK : EX_CONFIG);
}

int appcfg_parse_num_workers(json_t *obj, app_cfg_t *_app_cfg) {
    // In this application, number of worker threads excludes the main thread
    int new_capacity = (int)json_integer_value(obj);
    if (new_capacity < 0) {
        goto error;
    }
    // TODO, free some of memory if new capacity is smaller than current one
    h2o_vector_reserve(NULL, &_app_cfg->workers, (size_t)new_capacity);
    // preserve space first, update thread ID later
    _app_cfg->workers.size = new_capacity;
    return 0;
error:
    return -1;
}

int appcfg_parse_local_tmp_buf(json_t *obj, app_cfg_t *_app_cfg) {
    if (!json_is_object(obj)) {
        goto error;
    }
    json_t     *path_obj = json_object_get((const json_t *)obj, "path");
    json_t     *threshold_obj = json_object_get((const json_t *)obj, "threshold_in_bytes");
    const char *path = json_string_value(path_obj);
    int         threshold = (int)json_integer_value(threshold_obj);
    if (!path || threshold <= 0) {
        h2o_error_printf(
            "[parsing] invalid tmp_buf settings, path: %s , threshold: %d bytes\n", path, threshold
        );
        goto error;
    }
    // access check to the path
    if (access(path, F_OK | R_OK | W_OK) != 0) {
        h2o_error_printf("[parsing] not all requested permissions granted, path: %s\n", path);
        goto error;
    }
    _app_cfg->tmp_buf.threshold_bytes = (unsigned int)threshold;
    _app_cfg->tmp_buf.path = strdup(path); // TODO, ensure full path
    return 0;
error:
    return -1;
} // end of appcfg_parse_local_tmp_buf

void appcfg_notify_all_workers(app_cfg_t *app_cfg) {
    if (!app_cfg || !app_cfg->server_notifications.entries) {
        return;
    } // TODO , log error
    int idx = 0;
    for (idx = 0; idx < app_cfg->server_notifications.size; idx++) {
        h2o_multithread_receiver_t *receiver = app_cfg->server_notifications.entries[idx];
        // simply notify each worker without message
        h2o_multithread_send_message(receiver, NULL);
    }
}

int appcfg_start_workers(app_cfg_t *app_cfg, struct worker_init_data_t *data, void (*entry)(void *)) {
    if (!app_cfg || !data || !entry) {
        return -1;
    }
    int    idx = 0;
    int    ret = 0;
    size_t num_threads = app_cfg->workers.size + 1; // plus main thread
    h2o_vector_reserve(NULL, &app_cfg->server_notifications, num_threads);
    app_cfg->server_notifications.size = num_threads;
    // initiate worker threads first , than invoke run_loop() in this main thread
    for (idx = num_threads - 1; idx >= 0; idx--) {
        struct worker_init_data_t *data_ptr = &data[idx];
        *data_ptr =
            (struct worker_init_data_t){.app_cfg = app_cfg, .cfg_thrd_idx = (unsigned int)idx, .loop = NULL};
        if (idx == 0) {
            data_ptr->loop = uv_default_loop();
            entry((void *)data_ptr);
        } else if (idx > 0) {
            data_ptr->loop = (uv_loop_t *)h2o_mem_alloc(sizeof(uv_loop_t));
            ret = uv_loop_init(data_ptr->loop);
            if (ret != 0) {
                h2o_fatal(
                    "[system] failed to initialize loop at worker thread (index=%d), reason:%s \n", idx,
                    uv_strerror(ret)
                );
                break;
            }
            ret = uv_thread_create(&app_cfg->workers.entries[idx - 1], entry, (void *)data_ptr);
            if (ret != 0) {
                h2o_fatal(
                    "[system] failed to create worker thread (index=%d) , reason: %s \n", idx,
                    uv_strerror(ret)
                );
                break;
            }
        }
    } // end of workers iteration
    // wait until all worker threads exits
    for (idx = 0; idx < app_cfg->workers.size && ret == 0; idx++) {
        uv_thread_t tid = app_cfg->workers.entries[idx];
        if (uv_thread_join(&tid) != 0) {
            char errbuf[256];
            h2o_fatal("error on uv_thread_join : %s", h2o_strerror_r(errno, errbuf, sizeof(errbuf)));
        }
    }
    if (app_cfg->server_notifications.entries) {
        free(app_cfg->server_notifications.entries);
        app_cfg->server_notifications.entries = NULL;
        app_cfg->server_notifications.size = 0;
        app_cfg->server_notifications.capacity = 0;
    }
    return ret;
} // end of appcfg_start_workers

static void _app_loop_remain_handles_traverse(uv_handle_t *handle, void *arg) {
    // FIXME, this is workaround to ensure all handles are closed, there is a strange
    // issue when starting polling file descriptor using `uv_poll_start()` in libh2o client
    // request, the h2o connection will leave timer handle `uv_timer_t` in the event loop
    // and never clean up the timer even after the entire request completed.
    if (!uv_is_closing(handle)) {
        uv_close(handle, (uv_close_cb)NULL);
        uv_run(handle->loop, UV_RUN_NOWAIT);
    }
} // end of _app_loop_remain_handles_traverse

void appcfg_terminate_workers(app_cfg_t *app_cfg, struct worker_init_data_t *data) {
    if (!app_cfg || !data) {
        return;
    }
    size_t num_threads = app_cfg->workers.size + 1; // plus main thread
    int    idx = 0;
    int    ret = 0;
    for (idx = 0; idx < num_threads; idx++) {
        struct worker_init_data_t *data_ptr = &data[idx];
        if (data_ptr->loop) {
            uv_walk(data_ptr->loop, _app_loop_remain_handles_traverse, NULL);
            ret = uv_loop_close(data_ptr->loop);
            if (ret != 0) {
                h2o_error_printf(
                    "[system] failed to close loop at worker thread (index=%d), reason:%s \n",
                    data_ptr->cfg_thrd_idx, uv_strerror(ret)
                );
            }
            if (data_ptr->cfg_thrd_idx > 0) {
                free(data_ptr->loop);
                data_ptr->loop = NULL;
            } // for non-default loop
        }
    }
} // end of appcfg_terminate_workers
