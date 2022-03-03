#ifndef MEDIA__TIMER_POLL_H
#define MEDIA__TIMER_POLL_H
#ifdef __cplusplus
extern "C" {
#endif

#include <uv.h>

typedef struct app_timer_poll_s app_timer_poll_t;

typedef void (*timerpoll_timeout_cb)(app_timer_poll_t *target, int status);
typedef void (*timerpoll_close_cb)(app_timer_poll_t *target);

struct app_timer_poll_s {
    void  *data;
    int    flags;
    uv_poll_t  poll;
    uv_timer_t timeout;
    timerpoll_timeout_cb timeout_cb;
    timerpoll_close_cb   close_cb;
};

// the given file descriptor `fd` is monitored in the event loop `loop`
int app_timer_poll_init(uv_loop_t *loop, app_timer_poll_t *handle, int fd, int flags);

int app_timer_poll_start(app_timer_poll_t *handle, uint32_t timeout, timerpoll_timeout_cb timeout_cb);

int app_timer_poll_stop(app_timer_poll_t *handle);

int app_timer_poll_deinit(app_timer_poll_t *handle, timerpoll_close_cb  close_cb);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TIMER_POLL_H
