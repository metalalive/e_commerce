#include <assert.h>
#include <h2o.h>
#include <h2o/serverutil.h>

#include "app_cfg.h"
#include "timer_poll.h"
#include "rpc/cfg_parser.h"
#include "rpc/core.h"
#include "rpc/consumer.h"
#include "models/cfg_parser.h"
#include "models/pool.h"
#include "storage/cfg_parser.h"
#include "transcoder/cfg_parser.h"

// TODO, parameterize the delay time for heartbeat frame to send (to AMQP broker).
// When this application almost reaches timeout, it tries sending heartbeat frame
// to extend the connection lifetime (keep it active), this also avoids reconnecting
// at TCP level (which means it may have change of saving network latency)
#define RPC_REFRESH_CONNECTION_LATENCY_SECS 3

typedef struct {
    void  *conn;
    uv_loop_t  *loop;
    int    curr_fd;
    app_timer_poll_t timerpoll;
    uint8_t broker_active:1;
    uint8_t sock_fd_reopen:1;
} app_ctx_msgq_t;

typedef struct {
    uv_thread_t  thread_id;
    // used when notifying (waking up) the worker thread
    h2o_multithread_queue_t     *notify_queue;
    h2o_multithread_receiver_t   server_notifications;
    H2O_VECTOR(app_ctx_msgq_t)   msgq;
    void *mq_conns;
} app_ctx_worker_t;

static int _appworker_start_timerpoll(app_ctx_msgq_t *mq);


static int parse_cfg_params(const char *cfg_file_path, app_cfg_t *app_cfg)
{
    int err = 0;
    json_error_t jerror;
    json_t  *root = NULL;
    root = json_load_file(cfg_file_path, (size_t)0, &jerror);
    if (!json_is_object(root)) {
        h2o_error_printf("[parsing] decode error on JSON file %s at line %d, column %d\n",
               &jerror.source[0], jerror.line, jerror.column);
        err = -1;
        goto error;
    }
    {
        json_t *pid_file = json_object_get((const json_t *)root, "pid_file");
        json_t *filepath = json_object_get((const json_t *)pid_file, "rpc_consumer");
        err = appcfg_parse_pid_file(filepath, app_cfg);
        if (err) {  goto error; }
    }
    {
        json_t *err_log  = json_object_get((const json_t *)root, "error_log");
        json_t *filepath = json_object_get((const json_t *)err_log, "rpc_consumer");
        err = appcfg_parse_errlog_path(filepath, app_cfg);
        if (err) {  goto error; }
    }
    err = appcfg_parse_num_workers(json_object_get((const json_t *)root, "num_rpc_consumers"), app_cfg);
    if (err) {  goto error; }
    err = parse_cfg_rpc_callee(json_object_get((const json_t *)root, "rpc"), app_cfg);
    if (err) {  goto error; }
    err = parse_cfg_databases(json_object_get((const json_t *)root, "databases"), app_cfg);
    if (err) {  goto error; }
    err = parse_cfg_storages(json_object_get((const json_t *)root, "storages"), app_cfg);
    if (err) {  goto error; }
    err = parse_cfg_transcoder(json_object_get((const json_t *)root, "transcoder"), app_cfg);
    if (err) {  goto error; }
    err = appcfg_parse_local_tmp_buf(json_object_get((const json_t *)root, "tmp_buf"), app_cfg);
    if (err) {  goto error; }
    json_decref(root);
    return 0;
error:
    if (!root) {
        json_decref(root);
    }
    return -1;
} // end of parse_cfg_params

static void on_sigterm(int sig_num) {
    app_cfg_t *acfg = app_get_global_cfg();
    acfg->shutdown_requested = 1;
    appcfg_notify_all_workers(acfg);
    app_db_pool_map_signal_closing();
}

#ifdef LIBC_HAS_BACKTRACE
static void on_sigfatal(int sig_num) {
    app_cfg_t *acfg = app_get_global_cfg();
    // TODO, report error than exit
    deinit_app_cfg(acfg);
    raise(sig_num);
}
#endif // end of LIBC_HAS_BACKTRACE

static void init_signal_handler(void) {
    h2o_set_signal_handler(SIGTERM, on_sigterm);
    h2o_set_signal_handler(SIGINT,  on_sigterm);
    h2o_set_signal_handler(SIGPIPE, SIG_IGN); // ignore
#ifdef LIBC_HAS_BACKTRACE
    h2o_set_signal_handler(SIGABRT, on_sigfatal);
    h2o_set_signal_handler(SIGSEGV, on_sigfatal);
    h2o_set_signal_handler(SIGBUS,  on_sigfatal);
    h2o_set_signal_handler(SIGILL,  on_sigfatal);
    h2o_set_signal_handler(SIGFPE,  on_sigfatal);
#endif // end of LIBC_HAS_BACKTRACE
}

static void on_server_notification(h2o_multithread_receiver_t *receiver, h2o_linklist_t *msgs) {
    fprintf(stderr, "on_rpc_consumer_notification invoked \n");
    // the notification is used only for exitting h2o_evloop_run; actual changes are done in the main loop of run_loop
}

static void appworker_deinit_context(app_ctx_worker_t *ctx, uv_loop_t *loop)
{ // de-init worker context
    app_rpc_conn_deinit(ctx->mq_conns);
    if(ctx->msgq.entries) {
        for(size_t idx = 0; idx < ctx->msgq.size; idx++) {
            app_ctx_msgq_t *mq = &ctx->msgq.entries[idx];
            mq->timerpoll.close_cb = NULL;
            int err = app_timer_poll_deinit(&mq->timerpoll);
            if(err) {
                fprintf(stderr, "[worker] failed to de-init timer_poll on app cfg (idx=%lu) \n", idx);
            }
            mq->broker_active = 0;
        }
        uv_run(loop, UV_RUN_NOWAIT);
        free(ctx->msgq.entries);
        ctx->msgq.entries = NULL;
        ctx->msgq.size = 0;
        ctx->msgq.capacity = 0;
    }
    if(ctx->notify_queue) {
        h2o_multithread_unregister_receiver(ctx->notify_queue, &ctx->server_notifications);
        h2o_multithread_destroy_queue(ctx->notify_queue);
        ctx->notify_queue = NULL;
        uv_run(loop, UV_RUN_ONCE);
    }
} // end of appworker_deinit_context

static int appworker_init_context(app_ctx_worker_t *ctx, struct worker_init_data_t *init_data)
{ // init worker context
    int err = 0;
    app_cfg_t  *acfg = init_data->app_cfg;
    unsigned int thread_index = init_data->cfg_thrd_idx;
    ctx->thread_id = uv_thread_self();
    void *mq_conns  = app_rpc_conn_init(acfg->rpc.entries, acfg->rpc.size);
    if(!mq_conns) { // each thread has AMQP connection to each broker, no need to apply lock
        fprintf(stderr, "[worker] connection failure on message queue\n");
        err = -1;
        goto done;
    }
    ctx->mq_conns = mq_conns;
    h2o_vector_reserve(NULL, &ctx->msgq, acfg->rpc.size);
    ctx->msgq.size = acfg->rpc.size;
    memset(ctx->msgq.entries, 0, sizeof(app_ctx_msgq_t) * ctx->msgq.capacity);
    for(size_t idx = 0; idx < ctx->msgq.size; idx++) {
        app_ctx_msgq_t *mq = &ctx->msgq.entries[idx];
        void *mq_conn = app_rpc_context_lookup(mq_conns, (const char *)acfg->rpc.entries[idx].alias);
        int fd = app_rpc_get_sockfd(mq_conn);
        // periodically send heartbeat frame to AMQP broker, when the connection almost reaches timeout 
        err = app_timer_poll_init(init_data->loop, &mq->timerpoll, fd);
        if(err) {
            fprintf(stderr, "[worker] failed to init timer_poll on app cfg (idx=%lu) \n", idx);
            goto done;
        }
        mq->conn = mq_conn;
        mq->loop = init_data->loop;
        mq->curr_fd = fd;
        mq->broker_active = 1;
        mq->sock_fd_reopen = 0;
    } // end of loop
    ctx->notify_queue = h2o_multithread_create_queue((h2o_loop_t *)init_data->loop);
    h2o_multithread_register_receiver(ctx->notify_queue, &ctx->server_notifications, on_server_notification);
    acfg->server_notifications.entries[thread_index] = &ctx->server_notifications;
done:
    return err;
} // end of appworker_init_context


static  void appworker_timerpoll_message_cb(app_timer_poll_t *target, int status, int event)
{
    app_ctx_msgq_t *mq = H2O_STRUCT_FROM_MEMBER(app_ctx_msgq_t, timerpoll, target);
    uint8_t expected_timeout_reaching = status == UV_ETIMEDOUT;
    uint8_t unexpected_timeout_reached = (event & UV_DISCONNECT) != 0;
    uint8_t reconnect_required = expected_timeout_reaching || unexpected_timeout_reached;
    uint8_t reconnect_ok = 0;
    ARPC_STATUS_CODE res = APPRPC_RESP_OK;
    if(reconnect_required) {
        if(expected_timeout_reaching) { // try consuming, reconnect if error returned
            res = app_rpc_consume_message(mq->conn, mq->loop);
            // operation timeout means empty queue, also implicitly means the connection is still active
            reconnect_ok = (res == APPRPC_RESP_OK) || (res == APPRPC_RESP_MSGQ_OPERATION_TIMEOUT);
            if(reconnect_ok) {
                _appworker_start_timerpoll(mq);
            }
        }
        if(!reconnect_ok) {
            res = app_rpc_close_connection(mq->conn);
            res = app_rpc_open_connection(mq->conn);
            reconnect_ok = (res == APPRPC_RESP_OK);
        }
    } else {
        reconnect_ok = 1;
    }
    if(reconnect_ok) {
        app_rpc_consume_message(mq->conn, mq->loop);
        if(!mq->broker_active) {
            // will re-init both poller and timer in next event-loop iteration
            uv_timer_stop(&target->timeout);
            mq->sock_fd_reopen = 1;
            mq->broker_active = 1;
            fprintf(stderr, "[RPC comsumer] reconnect successfully after AMQP broker is down \n");
        }
    } else {
        if(mq->broker_active) {
            // disable poller, still keep timer running at the same interval,
            // so the consumer can reconnect in the future once the message broker
            // is ready again.
            uv_poll_stop(&target->poll);
            mq->broker_active = 0;
            fprintf(stderr, "[RPC comsumer] AMQP broker is down \n");// TODO, log error
        }
        uint64_t repeat = 0;
        uint64_t timeout_ms = 20 * 1000;
        uv_timer_start(&target->timeout, target->timeout.timer_cb, timeout_ms, repeat);
    }
} // end of appworker_timerpoll_message_cb


static int _appworker_start_timerpoll(app_ctx_msgq_t *mq)
{
    arpc_cfg_t *rpc_cfg = app_rpc_get_config(mq->conn);
    uint32_t events = UV_READABLE | UV_DISCONNECT;
    uint64_t timeout_ms = ((uint64_t)rpc_cfg->attributes.timeout_secs - RPC_REFRESH_CONNECTION_LATENCY_SECS) * 1000;
    return  app_timer_poll_start(&mq->timerpoll, timeout_ms, events, appworker_timerpoll_message_cb);
}

static void _appworker_reinit_timerpoll_cb(app_timer_poll_t *target) {
    app_ctx_msgq_t *mq = H2O_STRUCT_FROM_MEMBER(app_ctx_msgq_t, timerpoll, target);
    int new_fd = app_rpc_get_sockfd(mq->conn);
    int err = app_timer_poll_init(mq->loop, target, new_fd);
    if(!err) {
        err = _appworker_start_timerpoll(mq);
    }
    mq->curr_fd = new_fd;
    mq->sock_fd_reopen = 0;
    target->close_cb = NULL;
    // fprintf(stderr, "[RPC comsumer] after reinit timer-poll, err=%s (%d) \n", uv_strerror(err), err);
}

static void _appworker_maybe_reinit_timerpoll(app_ctx_worker_t *ctx) {
    for(size_t idx = 0; idx < ctx->msgq.size; idx++) {
        app_ctx_msgq_t *mq = &ctx->msgq.entries[idx];
        int old_fd = mq->curr_fd;
        int new_fd = app_rpc_get_sockfd(mq->conn);
        if(mq->sock_fd_reopen && new_fd > 0) {
            if(old_fd == new_fd) {
                _appworker_start_timerpoll(mq);
                mq->sock_fd_reopen = 0;
            } else {
                mq->timerpoll.close_cb = _appworker_reinit_timerpoll_cb;
                app_timer_poll_deinit(&mq->timerpoll);
                // fprintf(stderr, "[RPC comsumer] reopen sock fd to AMQP broker, old_fd=%d, new_fd=%d \n", old_fd, new_fd);
            }
        } // start timer-poll for each connection to message queue
    }
} // end of _appworker_maybe_reinit_timerpoll


static size_t get_num_of_pending_requests(app_ctx_worker_t *ctx)
{
    // TODO, gracefully shutdown the RPC workers
    return 0;
}


static  int appworker_waiting_messages(app_ctx_worker_t *ctx, app_cfg_t  *acfg, uv_loop_t *loop)
{
    int err = 0;
    for(size_t idx = 0; idx < ctx->msgq.size; idx++) {
        err = _appworker_start_timerpoll(&ctx->msgq.entries[idx]);
        if(err) { goto done; }
    } // start timer-poll for each connection to message queue
    while (!acfg->shutdown_requested) {
        uv_run(loop, UV_RUN_ONCE);
        _appworker_maybe_reinit_timerpoll(ctx);
    } // end of main event loop
    while(get_num_of_pending_requests(ctx) > 0) {
        uv_run(loop, UV_RUN_ONCE);
        _appworker_maybe_reinit_timerpoll(ctx);
    } // wait until all published message are handled & results are replied
done:
    return err;
} // end of appworker_waiting_messages

static void run_app_worker(void *data) {
    struct worker_init_data_t *init_data = (struct worker_init_data_t *)data;
    app_ctx_worker_t  ctx_worker = {0};
    int err = appworker_init_context(&ctx_worker, init_data);
    if(!err) {
        err = appworker_waiting_messages(&ctx_worker, init_data->app_cfg, init_data->loop);
    }
    appworker_deinit_context(&ctx_worker, init_data->loop);
} // end of run_app_worker

static int start_workers(app_cfg_t *app_cfg) {
    size_t num_threads = app_cfg->workers.size + 1; // plus main thread
    struct worker_init_data_t  worker_data[num_threads];
    int err = appcfg_start_workers(app_cfg, &worker_data[0], run_app_worker);
    appcfg_terminate_workers(app_cfg, &worker_data[0]);
    return err;
} // end of start_workers

int start_application(const char *cfg_file_path, const char *exe_path)
{
    int err = 0;
    app_global_cfg_set_exepath(exe_path);
    app_cfg_t *acfg = app_get_global_cfg();
    err = parse_cfg_params(cfg_file_path, acfg);
    if(err) {goto done;}
    init_signal_handler();
    err = start_workers(acfg);
done:
    deinit_app_cfg(acfg);
    return err;
}
