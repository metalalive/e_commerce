#include <string.h>
#include "timer_poll.h"
// TODO, defind error code to return

// should be linked to libuv library since they are NOT puclic APIs in libuv
//// extern int uv__fd_exists(uv_loop_t *, int);
//// extern int uv__io_check_fd(uv_loop_t *, int);
//// extern int uv__nonblock_ioctl(int, int);

static void _app_timerpoll_timeout_cb(uv_timer_t *timeout)
{
    app_timer_poll_t *handle = (app_timer_poll_t *) timeout->data;
    uv_poll_stop(&handle->poll);
    if(handle->poll_cb) {
        handle->poll_cb(handle, UV_ETIMEDOUT, 0);
    } // TODO, otherwise logging warning
}

static void _app_timerpoll_poll_cb(uv_poll_t  *poll, int status, int event)
{
    app_timer_poll_t *handle = (app_timer_poll_t *) poll->data;
    if(handle->poll_cb) {
        handle->poll_cb(handle, status, event);
    } // TODO, otherwise logging warning
}

static void _app_timerpoll_close_poll_cb(uv_handle_t* poll)
{
    app_timer_poll_t *handle = (app_timer_poll_t *) poll->data;
    poll->data = NULL;
    poll->loop = NULL;
    if(handle->close_cb && app_timer_poll_is_closed(handle)) {
        (handle->close_cb)(handle);
    } // ensure either this function or the function below invokes user-defined callback
}
 
static void _app_timerpoll_close_timer_cb(uv_handle_t* timeout)
{
    app_timer_poll_t *handle = (app_timer_poll_t *) timeout->data;
    timeout->data = NULL;
    timeout->loop = NULL;
    if(handle->close_cb && app_timer_poll_is_closed(handle)) {
        (handle->close_cb)(handle);
    }
}



int app_timer_poll_init(uv_loop_t *loop, app_timer_poll_t *handle, int fd)
{
    int ret = 0;
    if(!loop || !handle || fd <= 0) {
        ret = UV_EINVAL; // error
        goto done;
    }
    if(!app_timer_poll_is_closed(handle)) {
        ret = UV_EADDRINUSE; // hasn't been de-initialized , omit and return 
        goto done;
    }
    ret = uv_poll_init(loop, &handle->poll, fd);
    if(ret < 0) {
        goto done;
    }
    ret = uv_timer_init(loop, &handle->timeout);
    if(ret < 0) {
        uv_close((uv_handle_t *)&handle->poll, NULL);
        goto done;
    }
    handle->poll.data    = (void *)handle;
    handle->timeout.data = (void *)handle;
done:
    return ret;
} // end of app_timer_poll_init


int app_timer_poll_deinit(app_timer_poll_t *handle)
{
    if(!handle->poll.loop || !handle->timeout.loop) {
        return UV_EINVAL;
    }
    if(!uv_is_closing((const uv_handle_t *)&handle->poll)) {
        uv_close((uv_handle_t *)&handle->poll, _app_timerpoll_close_poll_cb);
    }
    if(!uv_is_closing((const uv_handle_t *)&handle->timeout)) {
        uv_close((uv_handle_t *)&handle->timeout, _app_timerpoll_close_timer_cb);
    }
    return 0;
} // end of app_timer_poll_deinit


uint8_t app_timer_poll_is_closing(app_timer_poll_t *handle) {
    if(!handle) {
        return 1;
    } else {
        return uv_is_closing((uv_handle_t *)&handle->poll) &&
            uv_is_closing((uv_handle_t *)&handle->timeout);
    }
} // end of app_timer_poll_is_closing

uint8_t app_timer_poll_is_closed(app_timer_poll_t *handle) {
    if(!handle) {
        return 1;
    } else {
        return !handle->poll.data && !handle->timeout.data;
    }
} // end of app_timer_poll_is_closed


int app_timer_poll_change_fd(app_timer_poll_t *handle, int new_fd) {
    // change file descriptor without initializing uv_poll_t , TODO: verify
    int err = 0;
    if(handle && new_fd) {
        err = UV_EINVAL;
        goto done;
    }
    uint8_t closed  = app_timer_poll_is_closed(handle);
    uint8_t closing = app_timer_poll_is_closing(handle);
    uv_poll_t *poll = &handle->poll;
    if(closed || closing) {
        err = UV_EAGAIN;
        goto done;
    }
    // TODO --- copy the code from internal.c in  libuv
    //// if(uv__fd_exists(poll->loop, new_fd)) {
    ////     err = UV_EEXIST;
    ////     goto done;
    //// }
    //// err = uv__io_check_fd(poll->loop, new_fd);
    //// if(err) { goto done; }
    //// err = uv__nonblock_ioctl(new_fd, 1);
    //// if(err) { goto done; }
    uv_poll_stop(poll);
    poll->io_watcher.fd = new_fd;
done:
    return err;
} // end of app_timer_poll_change_fd


int app_timer_poll_start(app_timer_poll_t *handle, uint64_t timeout_ms, uint32_t events, timerpoll_poll_cb poll_cb)
{
    int ret = 0;
    if(!poll_cb || !handle || !handle->timeout.loop || !handle->poll.loop
            || timeout_ms == 0 || events == 0) {
        ret = UV_EINVAL; // error cuased by argument
        goto done;
    }
    handle->poll_cb = poll_cb;
    ret = uv_poll_start(&handle->poll, (int)events, _app_timerpoll_poll_cb);
    if(ret < 0) {
        goto done;
    }
    uint64_t repeat = 0;
    ret = uv_timer_start(&handle->timeout, _app_timerpoll_timeout_cb, timeout_ms, repeat);
    if(ret < 0) {
        uv_poll_stop(&handle->poll);
        goto done;
    }
done:
    return ret;
} // end of app_timer_poll_start

int app_timer_poll_stop(app_timer_poll_t *handle)
{
    if(!handle || !handle->timeout.loop || !handle->poll.loop
            || uv_is_closing((const uv_handle_t *)&handle->poll)
            || uv_is_closing((const uv_handle_t *)&handle->timeout)) {
        return UV_EINVAL;
    }
    uv_timer_stop(&handle->timeout);
    uv_poll_stop(&handle->poll);
    return 0;
} // end of app_timer_poll_stop

