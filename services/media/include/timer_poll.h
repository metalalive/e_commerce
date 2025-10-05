#ifndef MEDIA__TIMER_POLL_H
#define MEDIA__TIMER_POLL_H
#ifdef __cplusplus
extern "C" {
#endif

#include <uv.h>

typedef struct app_timer_poll_s app_timer_poll_t;

typedef void (*timerpoll_close_cb)(app_timer_poll_t *target);
typedef void (*timerpoll_poll_cb)(app_timer_poll_t *target, int status, int event);

struct app_timer_poll_s {
    uv_poll_t          poll;
    uv_timer_t         timeout;
    timerpoll_poll_cb  poll_cb;
    timerpoll_close_cb close_cb;
};

// the given file descriptor `fd` is monitored in the event loop `loop`
int app_timer_poll_init(uv_loop_t *loop, app_timer_poll_t *handle, int fd);
int app_timer_poll_deinit(app_timer_poll_t *handle);

int app_timer_poll_start(
    app_timer_poll_t *handle, uint64_t timeout_ms, uint32_t events, timerpoll_poll_cb poll_cb
);
int app_timer_poll_stop(app_timer_poll_t *handle);

uint8_t app_timer_poll_is_closing(app_timer_poll_t *handle);
uint8_t app_timer_poll_is_closed(app_timer_poll_t *handle);

int app_timer_poll_change_fd(app_timer_poll_t *, int);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TIMER_POLL_H
