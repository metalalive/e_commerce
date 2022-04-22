#include <sys/types.h>
#include <unistd.h>
#include <sched.h>
#include <signal.h>
#include <assert.h>
#include <errno.h>
#ifdef LIBC_HAS_BACKTRACE
#include <execinfo.h>
#endif // end of LIBC_HAS_BACKTRACE
#include <h2o.h>
#include <h2o/serverutil.h>
#include <mysql.h>

#include "app.h"
#include "auth.h"
#include "network.h"
#include "cfg_parser.h"
#include "models/pool.h"

struct  worker_init_data_t{
    app_cfg_t  *app_cfg;
    uv_loop_t  *loop;
    unsigned int cfg_thrd_idx;
};

#define MAX_PERIOD_KEEP_JWKS_IN_SECONDS  3600 // 1 hour
static app_cfg_t _app_cfg = {
    .server_glb_cfg = {
        .hosts = NULL,
        .http2 = {0},
        .mimemap = NULL,
    },
    .listeners     = NULL,
    .num_listeners = 0,
    .pid_file      = NULL,
    .access_logger = NULL,
    .error_log_fd = -1,
    .max_connections = APP_DEFAULT_MAX_CONNECTIONS,
    .run_mode = RUN_MODE_MASTER,
    .workers = {.size = 0, .capacity = 0, .entries = NULL},
    .tfo_q_len = APP_DEFAULT_LENGTH_TCP_FASTOPEN_QUEUE,
    .exe_path = NULL,
    .launch_time = 0,
    .shutdown_requested = 0,
    .workers_sync_barrier = H2O_BARRIER_INITIALIZER(SIZE_MAX),
    .storages = {.size = 0, .capacity = 0, .entries = NULL},
    .state = {.num_curr_sessions=0},
    .jwks = {
        .handle = NULL, .src_url=NULL, .last_update = 0, .is_rotating = ATOMIC_FLAG_INIT,
        .ca_path = NULL, .ca_format = NULL, .max_expiry_secs = MAX_PERIOD_KEEP_JWKS_IN_SECONDS
    }
}; // end of _app_cfg


static void deinit_app_cfg(app_cfg_t *app_cfg) {
    int idx = 0;
    if(app_cfg->listeners) {
        for(idx = 0; app_cfg->listeners[idx] != NULL; idx++)
        {
            free_listener(app_cfg->listeners[idx]);
            app_cfg->listeners[idx] = NULL;
        }
        free(app_cfg->listeners);
        app_cfg->listeners = NULL;
    }
    // TODO, deallocate space assigned to worker threads
    h2o_config_dispose(&app_cfg->server_glb_cfg);
    app_db_pool_map_deinit();
    if(app_cfg->tmp_buf.path) {
        free(app_cfg->tmp_buf.path);
        app_cfg->tmp_buf.path = NULL;
    }
    if(app_cfg->pid_file) {
        fclose(app_cfg->pid_file);
        app_cfg->pid_file = NULL;
    }
    if(app_cfg->jwks.handle != NULL) {
        r_jwks_free(app_cfg->jwks.handle);
        app_cfg->jwks.handle = NULL;
    }
    if(app_cfg->workers.entries) {
        free(app_cfg->workers.entries);
        app_cfg->workers.entries = NULL;
    }
    if(app_cfg->storages.entries) {
        for(idx = 0; idx < app_cfg->storages.size; idx++) {
            asa_cfg_t *asacfg = &app_cfg->storages.entries[idx];
            if(asacfg->alias) {
                free(asacfg->alias);
                asacfg->alias = NULL;
            }
            if(asacfg->base_path) {
                free(asacfg->base_path);
                asacfg->base_path = NULL;
            }
        }
        free(app_cfg->storages.entries);
        app_cfg->storages.capacity = 0;
        app_cfg->storages.entries = NULL; 
    }
    if(app_cfg->jwks.src_url != NULL) {
        free(app_cfg->jwks.src_url);
        app_cfg->jwks.src_url = NULL;
    }
    if(app_cfg->error_log_fd > 0) {
        close(app_cfg->error_log_fd);
        app_cfg->error_log_fd = -1;
    } // should be done lastly
} // end of deinit_app_cfg


static void on_tcp_close(uv_handle_t *client_conn) {
    atomic_num_connections(&_app_cfg, -1);
    // the handle created in init_client_tcp_socket, its memory should be freed in
    //  network.c instead of app.c (TODO: refactor)
    client_conn->data = NULL; // pointer to callback function can be set NULL directly
    destroy_network_handle(client_conn, (uv_close_cb)free);
}


static void on_tcp_accept(uv_stream_t *server, int status) {
    app_ctx_listener_t *ctx = (app_ctx_listener_t *) server->data;
    // TLS handshake takes about 1 ms, this limits the latency induced by TLS handshakes to 10 ms per event loop
    size_t num_accepts = 10;
    do {
        int curr_num_conns = atomic_num_connections(&_app_cfg, 1);
        if(curr_num_conns >= _app_cfg.max_connections) {
            atomic_num_connections(&_app_cfg, -1);
            h2o_error_printf("[worker] ID = %lu, number of connections exceeds the limit %d \n",
                 (unsigned long int)uv_thread_self(), _app_cfg.max_connections );
            // TODO, return http response status 429 (too many requests)
            break;
        }
        h2o_socket_t *sock = init_client_tcp_socket(server, on_tcp_close);
        if(!sock) {
            atomic_num_connections(&_app_cfg, -1);
            h2o_error_printf("[worker] ID = %lu, end of pending connection reached \n", (unsigned long int)uv_thread_self() );
            // TODO, free space in `sock`, return http response status 500 (internal error)
            break;
        }
        assert(&ctx->accept_ctx);
        h2o_accept(&ctx->accept_ctx, sock);
    } while (--num_accepts > 0);
} // end of on_tcp_accept


static void on_server_notification(h2o_multithread_receiver_t *receiver, h2o_linklist_t *msgs) {
    fprintf(stdout, "on_server_notification invoked \n");
    // the notification is used only for exitting h2o_evloop_run; actual changes are done in the main loop of run_loop
}

static void notify_all_workers(app_cfg_t *app_cfg) {
    int idx = 0;
    for(idx = 0; idx < app_cfg->server_notifications.size; idx++) {
        h2o_multithread_receiver_t *receiver = app_cfg->server_notifications.entries[idx];
        // simply notify each worker without message
        h2o_multithread_send_message(receiver, NULL);
    }
}


int app_server_ready(void) {
    int workers_ready = h2o_barrier_done(&_app_cfg.workers_sync_barrier);
    int jwks_ready = r_jwks_is_valid(_app_cfg.jwks.handle) == RHN_OK;
    if(workers_ready && !jwks_ready) {
        app_rotate_jwks_store(&_app_cfg.jwks);
    }
    return workers_ready && jwks_ready;
}

static void on_sigterm(int sig_num) {
    _app_cfg.shutdown_requested = 1;
    if(!app_server_ready()) {
        exit(0);
    } // shutdown immediately if initialization hasn't been done yet
    notify_all_workers(&_app_cfg);
    app_db_pool_map_signal_closing();
}

#ifdef LIBC_HAS_BACKTRACE
static void on_sigfatal(int sig_num) {
    // re-apply default action (signal handler) after doing following
    h2o_set_signal_handler(sig_num, SIG_DFL);
    if(sig_num != SIGINT) { // print stack backtrace
        const int num_frames = 128;
        int num_used = 0;
        void *frames[num_frames];
        num_used = backtrace(frames, num_frames);
        backtrace_symbols_fd(frames, num_used, _app_cfg.error_log_fd);
    }
    deinit_app_cfg(&_app_cfg);
    raise(sig_num);
}
#endif // end of LIBC_HAS_BACKTRACE


int init_security(void) {
    uint64_t opts = OPENSSL_INIT_LOAD_SSL_STRINGS | OPENSSL_INIT_LOAD_CRYPTO_STRINGS;
    int err = (OPENSSL_init_ssl(opts, NULL) == 0);
    return err;
}


static void register_global_access_log(h2o_globalconf_t *glbcfg, h2o_access_log_filehandle_t *logfh) {
    int idx = 0, jdx = 0;
    for (idx = 0; glbcfg->hosts[idx]; idx++) {
        h2o_hostconf_t *hostcfg = glbcfg->hosts[idx];
        for (jdx = 0; jdx < hostcfg->paths.size; jdx++) {
            h2o_pathconf_t *pathcfg = hostcfg->paths.entries[jdx];
            h2o_access_log_register(pathcfg, logfh);
        }
    } // TODO, figure out how access logger handler works
} // end of register_global_access_log


static void init_signal_handler(void) {
    h2o_set_signal_handler(SIGTERM, on_sigterm);
    h2o_set_signal_handler(SIGPIPE, SIG_IGN); // ignore
#ifdef LIBC_HAS_BACKTRACE
    h2o_set_signal_handler(SIGABRT, on_sigfatal);
    h2o_set_signal_handler(SIGSEGV, on_sigfatal);
    h2o_set_signal_handler(SIGBUS,  on_sigfatal);
    h2o_set_signal_handler(SIGILL,  on_sigfatal);
    h2o_set_signal_handler(SIGINT,  on_sigfatal);
    h2o_set_signal_handler(SIGFPE,  on_sigfatal);
#endif // end of LIBC_HAS_BACKTRACE
}

static void worker_dup_network_handle(app_ctx_listener_t *ctx, const app_cfg_t *cfg, uv_loop_t *loop, unsigned int thread_index)
{
    app_cfg_listener_t *listener = NULL;
    int idx = 0;
    for(idx = 0; (listener = cfg->listeners[idx]) != NULL; idx++) {
        uv_handle_t *nt_handle = listener->nt_handle;
        // each worker (excluding main thread) duplicate network handle from config object
        uv_nt_handle_data *nt_attr = (uv_nt_handle_data *)nt_handle->data;
        assert(nt_attr != NULL);
        // duplicate network handler for each worker thread
        struct sockaddr sa = {0};
        int sa_len = sizeof(sa); // has to indicate length of sockaddr structure
        uv_tcp_getsockname((uv_tcp_t *)nt_handle, &sa, &sa_len);
        assert(sa_len > 0);
        struct addrinfo ai = {
            .ai_addr = &sa, .ai_next = NULL, .ai_family = nt_attr->ai_family,
            .ai_flags = nt_attr->ai_flags, .ai_socktype = nt_attr->ai_socktype,
            .ai_protocol = nt_attr->ai_protocol
        };
        nt_handle = (uv_handle_t *)create_network_handle( loop, &ai,
                  on_tcp_accept,  cfg->tfo_q_len );
        assert(nt_handle != NULL);
        // network handle stores the pointer to listener context, which will be used
        // later on accepting request
        nt_handle->data = (void *)&ctx[idx];
        ctx[idx].nt_handle = nt_handle;
    } // end of listener iteration
} // end of worker_dup_network_handle


static void worker_init_accept_ctx(app_ctx_listener_t *ctx, const app_cfg_t *cfg, h2o_context_t *http_ctx)
{
    app_cfg_listener_t *listener = NULL;
    int idx = 0;
    for(idx = 0; (listener = cfg->listeners[idx]) != NULL; idx++) {
        h2o_hostconf_t **hosts = h2o_mem_alloc(sizeof(h2o_hostconf_t *) * 2);
        hosts[0] = listener->hostconf;
        hosts[1] = NULL; // NULL-terminated list
        ctx[idx].accept_ctx = (h2o_accept_ctx_t) {
            .ctx = http_ctx,  .hosts = hosts,  .ssl_ctx = listener->security.ctx,
            .http2_origin_frame = NULL, .expect_proxy_line = 0,
            .libmemcached_receiver = NULL
        };
    } // end of listener iteration
    // some context-scope data is required for each http request
    assert(http_ctx->storage.capacity == 0);
    assert(http_ctx->storage.size == 0);
    h2o_vector_reserve(NULL, &http_ctx->storage , 1);
    http_ctx->storage.entries[0] = (h2o_context_storage_item_t) {.dispose = NULL, .data = (void *)&cfg->jwks};
    http_ctx->storage.size = 1;
} // end of worker_init_accept_ctx


static void worker_deinit_context(app_ctx_worker_t *ctx)
{
    app_ctx_listener_t *listener = NULL;
    int idx = 0;
    for(idx = 0; idx < ctx->num_listeners; idx++) {
        listener = &ctx->listeners[idx];
        listener->nt_handle->data = NULL; // can set NULL directly because it is stack space
        destroy_network_handle(listener->nt_handle, (uv_close_cb)free);
        free(listener->accept_ctx.hosts);
        listener->accept_ctx.hosts = NULL;
    }
} // end of worker_deinit_context


static void migrate_worker_to_cpu(uv_thread_t tid, unsigned int cpu_id) {
    // currently this application evenly distributes all worker threads to all CPU cores
#if defined(HAS_PTHREAD_SETAFFINITY_NP)
    cpu_set_t cpu_set;
    CPU_ZERO(&cpu_set);
    CPU_SET(cpu_id, &cpu_set);
    int ret = pthread_setaffinity_np(tid, sizeof(cpu_set_t), &cpu_set);
    if(ret != 0) {
        h2o_error_printf("[system] failed to migrate thread (id = %ld) to CPU core %u \n",
                (long int)tid, cpu_id);
    }
#endif // end of HAS_PTHREAD_SETAFFINITY_NP
} // end of migrate_worker_to_cpu


static void run_loop(void *data) {
    struct worker_init_data_t *init_data = (struct worker_init_data_t *)data;
    app_cfg_t  *app_cfg = init_data->app_cfg;
    app_ctx_listener_t  ctx_listeners[app_cfg->num_listeners];
    app_ctx_worker_t    ctx_worker = {
        .listeners = &ctx_listeners[0],
        .num_listeners = app_cfg->num_listeners,
        .thread_id=uv_thread_self() };
    h2o_context_t server_ctx; // shared among listeners
    unsigned int thread_index = init_data->cfg_thrd_idx;
    size_t num_cpus = h2o_numproc();
    int idx = 0;
    migrate_worker_to_cpu(ctx_worker.thread_id, (thread_index % num_cpus));
    h2o_context_init(&server_ctx, init_data->loop, &app_cfg->server_glb_cfg);
    h2o_multithread_register_receiver(server_ctx.queue, &ctx_worker.server_notifications,
           on_server_notification );
    app_cfg->server_notifications.entries[thread_index] = &ctx_worker.server_notifications;
    worker_dup_network_handle(ctx_worker.listeners, app_cfg, init_data->loop, thread_index);
    worker_init_accept_ctx(ctx_worker.listeners, app_cfg, &server_ctx);
    // stop at here until all other threads reach this point, then serve requests concurrently
    h2o_barrier_wait(&app_cfg->workers_sync_barrier);
    if(thread_index == 0) {
        for(idx = 0; idx < app_cfg->num_listeners; idx++) {
            uv_handle_t *nt_handle = app_cfg->listeners[idx]->nt_handle;
            destroy_network_handle(nt_handle, (uv_close_cb)free);
        } // close network handle in config object here prior to start running the loop
    } // free memory allocated to `nt_attr` in app_cfg->listeners
    while (!app_cfg->shutdown_requested) {
        // TODO, refine a design for application-defined hook functions in this run loop
        app_rotate_jwks_store(&app_cfg->jwks);
        uv_run(init_data->loop, UV_RUN_ONCE);
    } // end of main event loop
    if(thread_index == 0) {
        h2o_error_printf("[system] graceful shutdown starts \n");
    }
    h2o_context_request_shutdown(&server_ctx);
    while(atomic_num_connections(app_cfg, 0) > 0) {
        uv_run(init_data->loop, UV_RUN_ONCE);
    } // wait until all client requests are handled & connection closed
    // close internal timer used only for timeout on graceful shutdown
    h2o_timer_unlink(&server_ctx.http2._graceful_shutdown_timeout);
    h2o_timer_unlink(&server_ctx.http3._graceful_shutdown_timeout);
    h2o_multithread_unregister_receiver(server_ctx.queue, &ctx_worker.server_notifications);
    h2o_context_dispose(&server_ctx);
    worker_deinit_context(&ctx_worker);
    uv_run(init_data->loop, UV_RUN_ONCE);
} // end of run_loop


static void _app_loop_remain_handles_traverse(uv_handle_t *handle, void *arg) {
    // FIXME, this is workaround to ensure all handles are closed, there is a strange
    // issue when starting polling file descriptor using `uv_poll_start()` in libh2o client
    // request, the h2o connection will leave timer handle `uv_timer_t` in the event loop
    // and never clean up the timer even after the entire request completed.
    if(!uv_is_closing(handle)) {
        uv_close(handle, (uv_close_cb)NULL);
        uv_run(handle->loop, UV_RUN_NOWAIT);
    }
} // end of _app_loop_remain_handles_traverse


static int start_workers(app_cfg_t *app_cfg) {
    size_t num_threads = app_cfg->workers.size + 1; // plus main thread
    struct worker_init_data_t  worker_data[num_threads];
    int idx = 0;
    int ret = 0;
    h2o_vector_reserve(NULL, &app_cfg->server_notifications, num_threads);
    app_cfg->server_notifications.size = num_threads;
    h2o_barrier_init(&app_cfg->workers_sync_barrier, num_threads);
    // initiate worker threads first , than invoke run_loop() in this main thread
    for(idx = num_threads - 1; idx >= 0 ; idx--) {
        struct worker_init_data_t  *data_ptr = &worker_data[idx];
        *data_ptr = (struct worker_init_data_t){.app_cfg = app_cfg,
             .cfg_thrd_idx = (unsigned int)idx, .loop = NULL };
        if(idx == 0) {
            data_ptr->loop = uv_default_loop();
            run_loop((void *)data_ptr);
        } else if (idx > 0) {
            data_ptr->loop = (uv_loop_t *)h2o_mem_alloc(sizeof(uv_loop_t));
            ret = uv_loop_init(data_ptr->loop);
            if(ret != 0) {
                h2o_fatal("[system] failed to initialize loop at worker thread (index=%d), reason:%s \n",
                        idx, uv_strerror(ret));
                goto done;
            }
            ret = uv_thread_create( &app_cfg->workers.entries[idx - 1], run_loop, (void *)data_ptr );
            if(ret != 0) {
                h2o_fatal("[system] failed to create worker thread (index=%d) , reason: %s \n",
                        idx, uv_strerror(ret));
                goto done;
            }
        }
    } // end of workers iteration
    // wait until all worker threads exits
    for(idx = 0; idx < app_cfg->workers.size; idx++) {
        uv_thread_t tid = app_cfg->workers.entries[idx];
        if(uv_thread_join(&tid) != 0) {
            char errbuf[256];
            h2o_fatal("error on uv_thread_join : %s", h2o_strerror_r(errno, errbuf, sizeof(errbuf)));
        }
    }
    app_db_poolmap_close_all_conns(worker_data[0].loop);
    while(!app_db_poolmap_check_all_conns_closed()) {
        int ms = 500;
        uv_sleep(ms);
    }
done:
    for(idx = 0; idx < num_threads; idx++) {
        struct worker_init_data_t  *data_ptr = &worker_data[idx];
        if(data_ptr->loop) {
            uv_walk(data_ptr->loop, _app_loop_remain_handles_traverse, NULL);
            ret = uv_loop_close(data_ptr->loop);
            if(ret != 0) {
                h2o_error_printf("[system] failed to close loop at worker thread (index=%d), reason:%s \n",
                        data_ptr->cfg_thrd_idx, uv_strerror(ret));
            }
            if(data_ptr->cfg_thrd_idx > 0) {
                free(data_ptr->loop);
                data_ptr->loop = NULL;
            } // for non-default loop
        }
    }
    return  ret;
} // end of start_workers


int start_application(const char *cfg_file_path, const char *exe_path)
{   // TODO, flush log message to centralized service e.g. ELK stack
    int err = 0;
    _app_cfg.exe_path = exe_path;
    atomic_init(&_app_cfg.state.num_curr_connections , 0);
    h2o_config_init(&_app_cfg.server_glb_cfg);
    r_global_init(); // rhonabwy JWT library
    err = init_security();
    if(err) { goto done; }
    const char *mysql_groups[] = {"client", NULL};
    err = mysql_library_init(0, NULL, (char **)mysql_groups);
    if(err) { goto done; }
    err = parse_cfg_params(cfg_file_path, &_app_cfg);
    if(err) { goto done; }
    register_global_access_log(&_app_cfg.server_glb_cfg , _app_cfg.access_logger);
    init_signal_handler();
    err = start_workers(&_app_cfg);
done:
    deinit_app_cfg(&_app_cfg);
    mysql_library_end();
    r_global_close(); // rhonabwy JWT library
    return err;
} // end of start_application

