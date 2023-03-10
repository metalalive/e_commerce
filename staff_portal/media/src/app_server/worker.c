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

#include "app_cfg.h"
#include "app_server.h"
#include "auth.h"
#include "network.h"
#include "cfg_parser.h"
#include "models/pool.h"
#include "storage/cfg_parser.h"
#include "rpc/cfg_parser.h"
#include "rpc/core.h"

typedef struct {
    h2o_accept_ctx_t  accept_ctx; // context applied when accepting new request associated with the listener
    uv_handle_t  *nt_handle; // network handle associated with the listener
} app_ctx_listener_t;

typedef struct {
    uv_thread_t  thread_id;
    app_ctx_listener_t *listeners;
    unsigned int num_listeners;
    // used when notifying (waking up) the worker thread
    h2o_multithread_receiver_t  server_notifications;
} app_ctx_worker_t;


static void deinit_app_server_cfg(app_cfg_t *app_cfg) {
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
    if(app_cfg->tmp_buf.path) {
        free(app_cfg->tmp_buf.path);
        app_cfg->tmp_buf.path = NULL;
    }
    if(app_cfg->jwks.handle != NULL) {
        r_jwks_free(app_cfg->jwks.handle);
        app_cfg->jwks.handle = NULL;
    }
    if(app_cfg->storages.entries) {
        app_storage_cfg_deinit(app_cfg);
    }
    for(idx = 0; idx < app_cfg->rpc.size; idx++) {
        app_rpc_cfg_deinit(&app_cfg->rpc.entries[idx]);
    }
    if(app_cfg->jwks.src_url != NULL) {
        free(app_cfg->jwks.src_url);
        app_cfg->jwks.src_url = NULL;
    }
    if(app_cfg->access_logger != NULL) {
        int ret = 0;
        while(!ret)
            ret = h2o_mem_release_shared(app_cfg->access_logger);
        app_cfg->access_logger = NULL;
    }
    deinit_app_cfg(app_cfg);
    r_global_close(); // rhonabwy JWT library
} // end of deinit_app_server_cfg


static  void on_tcp_close(uv_handle_t *client_conn) {
    atomic_num_connections(app_get_global_cfg(), -1);
    // the handle created in init_client_tcp_socket has to be freed if it is already closed
    free(client_conn);
}


static void on_tcp_accept(uv_stream_t *server, int status) {
    app_ctx_listener_t *ctx = (app_ctx_listener_t *) server->data;
    // TLS handshake takes about 1 ms, this limits the latency induced by TLS handshakes to 10 ms per event loop
    size_t num_accepts = 10;
    app_cfg_t *acfg = app_get_global_cfg();
    do { // TODO, figure out how to use libh2o to monitor and de-init connections which hang
        int curr_num_conns = atomic_num_connections(acfg, 1);
        if(curr_num_conns >= acfg->max_connections) {
            atomic_num_connections(acfg, -1);
            h2o_error_printf("[worker] ID = %lu, number of connections exceeds the limit %d \n",
                 (unsigned long int)uv_thread_self(), acfg->max_connections );
            // TODO, return http response status 429 (too many requests)
            break;
        }
        h2o_socket_t *sock = init_client_tcp_socket(server, on_tcp_close);
        if(!sock) {
            atomic_num_connections(acfg, -1);
            h2o_error_printf("[worker] ID = %lu, end of pending connection reached \n", (unsigned long int)uv_thread_self() );
            // TODO, free space in `sock`, return http response status 500 (internal error)
            break;
        }
        assert(&ctx->accept_ctx);
        h2o_accept(&ctx->accept_ctx, sock);
    } while (--num_accepts > 0);
} // end of on_tcp_accept


static void on_server_notification(h2o_multithread_receiver_t *receiver, h2o_linklist_t *msgs) {
    fprintf(stdout, "on_app_server_notification invoked \n");
    // the notification is used only for exitting h2o_evloop_run; actual changes are done in the main loop of run_loop
}

int app_server_ready(void) {
    app_cfg_t *acfg = app_get_global_cfg();
    int workers_ready = h2o_barrier_done(&acfg->workers_sync_barrier);
    int jwks_ready = r_jwks_is_valid(acfg->jwks.handle) == RHN_OK;
    if(workers_ready && !jwks_ready) {
        app_rotate_jwks_store(&acfg->jwks);
    }
    return workers_ready && jwks_ready;
}

static void on_sigterm(int sig_num) {
    app_cfg_t *acfg = app_get_global_cfg();
    if(acfg->shutdown_requested == 0) {
        acfg->shutdown_requested = APP_GRACEFUL_SHUTDOWN;
    } else if(acfg->shutdown_requested == APP_GRACEFUL_SHUTDOWN) {
        acfg->shutdown_requested = APP_HARD_SHUTDOWN;
    }
    if(!app_server_ready()) {
        exit(0);
    } // shutdown immediately if initialization hasn't been done yet
    appcfg_notify_all_workers(acfg);
    app_db_pool_map_signal_closing();
}

#ifdef LIBC_HAS_BACKTRACE
static void on_sigfatal(int sig_num) {
    // re-apply default action (signal handler) after doing following
    h2o_set_signal_handler(sig_num, SIG_DFL);
    app_cfg_t *acfg = app_get_global_cfg();
    if(sig_num != SIGINT) { // print stack backtrace
        const int num_frames = 128;
        int num_used = 0;
        void *frames[num_frames];
        num_used = backtrace(frames, num_frames);
        backtrace_symbols_fd(frames, num_used, acfg->error_log_fd);
    }
    raise(sig_num);
}
#endif // end of LIBC_HAS_BACKTRACE


int init_security(void) {
    uint64_t opts = OPENSSL_INIT_LOAD_SSL_STRINGS | OPENSSL_INIT_LOAD_CRYPTO_STRINGS;
    int err = OPENSSL_init_ssl(opts, NULL) == 0;
    r_global_init(); // rhonabwy JWT library
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
    h2o_vector_reserve(NULL, &http_ctx->storage , 2);
    http_ctx->storage.entries[0] = (h2o_context_storage_item_t) {.dispose = NULL, .data = (void *)&cfg->jwks};
    http_ctx->storage.entries[1] = (h2o_context_storage_item_t) {.dispose = app_rpc_conn_deinit,
        .data = app_rpc_conn_init(cfg->rpc.entries, cfg->rpc.size) };
    http_ctx->storage.size = 2;
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
    while((atomic_num_connections(app_cfg, 0) > 0) && (app_cfg->shutdown_requested != APP_HARD_SHUTDOWN))
    {
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


static int appserver_start_workers(app_cfg_t *app_cfg) {
    size_t num_threads = app_cfg->workers.size + 1; // plus main thread
    struct worker_init_data_t  worker_data[num_threads];
    h2o_barrier_init(&app_cfg->workers_sync_barrier, num_threads);
    int err = appcfg_start_workers(app_cfg, &worker_data[0], run_loop);
    if(!err) {
        app_db_poolmap_close_all_conns(worker_data[0].loop);
        while(!app_db_poolmap_check_all_conns_closed()) {
            int ms = 500;
            uv_sleep(ms);
        }
    }
    appcfg_terminate_workers(app_cfg, &worker_data[0]);
    return err;
} // end of appserver_start_workers

// TODO, flush log message to centralized service e.g. ELK stack
int start_application(const char *cfg_file_path, const char *exe_path)
{
    int err = 0;
    app_global_cfg_set_exepath(exe_path);
    app_cfg_t *acfg = app_get_global_cfg();
    atomic_init(&acfg->state.num_curr_connections , 0);
    h2o_config_init(&acfg->server_glb_cfg);
    err = init_security();
    if(err) { goto done; }
    err = parse_cfg_params(cfg_file_path, acfg);
    if(err) { goto done; }
    register_global_access_log(&acfg->server_glb_cfg , acfg->access_logger);
    init_signal_handler();
    err = appserver_start_workers(acfg);
done:
    // ISSUE: if JWKS hasn't been updated from auth server, this app server will unexpectedly skip
    //     de-initialization  deinit_app_server_cfg() after workers completed
    deinit_app_server_cfg(acfg);
    return err;
} // end of start_application
