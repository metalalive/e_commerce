#include <fcntl.h>
#include <unistd.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include "timer_poll.h"

static void mock_timerpoll_deinit_cb(app_timer_poll_t *target) { mock(target); }

Ensure(app_timerpoll_init_failure_test) {
    uv_loop_t       *loop = uv_default_loop();
    app_timer_poll_t handle = {0};
    int              fd = -1;
    int              err = app_timer_poll_init(loop, &handle, fd);
    assert_that(err, is_equal_to(UV_EINVAL));
    { // assume another thread hasn't completed closing the timer-poll handle
        fd = 234;
        handle.poll.data = &handle;
        handle.timeout.data = &handle;
        err = app_timer_poll_init(loop, &handle, fd);
        assert_that(err, is_equal_to(UV_EADDRINUSE));
    }
    { // invalid file descriptor
        handle.poll.data = NULL;
        handle.timeout.data = NULL;
        err = app_timer_poll_init(loop, &handle, fd);
        assert_that(err, is_equal_to(UV_EBADF));
    }
    { // pass regular file descriptor is NOT allowed by epoll
        char tmpfile_path[30] = "./tmp/timerpoll_init_XXXXXX";
        int  fd = mkstemp(&tmpfile_path[0]);
        err = app_timer_poll_init(loop, &handle, fd);
        assert_that(err, is_equal_to(UV_EPERM));
        close(fd);
        unlink(&tmpfile_path[0]);
    }
} // end of app_timerpoll_init_failure_test

static void _app_timerpoll_test_setup(uv_loop_t *loop, app_timer_poll_t *handle, int pipe_fd_pair[2]) {
    int err = pipe2(pipe_fd_pair, O_NONBLOCK);
    assert_that(err, is_equal_to(0));
    err = app_timer_poll_init(loop, handle, pipe_fd_pair[0]);
    assert_that(err, is_equal_to(0));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(app_timer_poll_is_closing(handle), is_equal_to(0));
    assert_that(app_timer_poll_is_closed(handle), is_equal_to(0));
}

static void _app_timerpoll_test_teardown(uv_loop_t *loop, app_timer_poll_t *handle, int pipe_fd_pair[2]) {
    uv_os_fd_t osfd = 0;
    assert_that(uv_fileno((uv_handle_t *)&handle->poll, &osfd), is_equal_to(0));
    assert_that(osfd, is_greater_than(0));
    assert_that(osfd, is_equal_to(pipe_fd_pair[0]));
    handle->close_cb = mock_timerpoll_deinit_cb;
    app_timer_poll_deinit(handle);
    assert_that(app_timer_poll_is_closing(handle), is_equal_to(1));
    assert_that(app_timer_poll_is_closed(handle), is_equal_to(0));
    expect(mock_timerpoll_deinit_cb, when(target, is_equal_to(handle)));
    uv_run(loop, UV_RUN_ONCE);
    assert_that(app_timer_poll_is_closing(handle), is_equal_to(1));
    assert_that(app_timer_poll_is_closed(handle), is_equal_to(1));
    assert_that(close(pipe_fd_pair[0]), is_equal_to(0));
    assert_that(close(pipe_fd_pair[1]), is_equal_to(0));
}

Ensure(app_timerpoll_init_success_test) {
    uv_loop_t       *loop = uv_default_loop();
    app_timer_poll_t handle = {0};
    int              pipe_fd_pair[2] = {0};
    _app_timerpoll_test_setup(loop, &handle, pipe_fd_pair);
    _app_timerpoll_test_teardown(loop, &handle, pipe_fd_pair);
} // end of app_timerpoll_init_success_test

#define PIPE_READ_BUF_BYTES 30
typedef struct {
    app_timer_poll_t super;
    size_t           nread;
    char             rd_buf[PIPE_READ_BUF_BYTES];
} test_timerpoll_t;

static void mock_timerpoll_data_ready_cb(app_timer_poll_t *target, int status, int event) {
    mock(target, status, event);
    if (status == 0) {
        uv_os_fd_t        pipe_rd = 0;
        test_timerpoll_t *handle = (test_timerpoll_t *)target;
        uv_fileno((uv_handle_t *)&target->poll, &pipe_rd);
        size_t pos = handle->nread;
        handle->nread += read(pipe_rd, &handle->rd_buf[pos], PIPE_READ_BUF_BYTES);
    }
}

Ensure(app_timerpoll_uninit_failure_test) {
    app_timer_poll_t handle = {0};
    int              err = 0;
    err = app_timer_poll_start(&handle, 123, UV_READABLE, mock_timerpoll_data_ready_cb);
    assert_that(err, is_equal_to(UV_EINVAL));
    err = app_timer_poll_stop(&handle);
    assert_that(err, is_equal_to(UV_EINVAL));
    err = app_timer_poll_deinit(&handle);
    assert_that(err, is_equal_to(UV_EINVAL));
} // end of app_timerpoll_uninit_failure_test

Ensure(app_timerpoll_target_fd_working_test) {
    uv_loop_t       *loop = uv_default_loop();
    test_timerpoll_t handle = {0};
    int              pipe_fd_pair[2] = {0};
    int              err = 0;
    _app_timerpoll_test_setup(loop, &handle.super, pipe_fd_pair);
    {
        uint64_t timeout_ms = 3000;
        uint32_t expect_events = UV_READABLE;
        err = app_timer_poll_start(&handle.super, timeout_ms, expect_events, mock_timerpoll_data_ready_cb);
        assert_that(err, is_equal_to(0));
        uv_run(loop, UV_RUN_NOWAIT);
        write(pipe_fd_pair[1], "C10K", 4);
        expect(
            mock_timerpoll_data_ready_cb, when(target, is_equal_to(&handle.super)),
            when(status, is_equal_to(0))
        );
        uv_run(loop, UV_RUN_ONCE);
        assert_that(handle.nread, is_equal_to(4));
        write(pipe_fd_pair[1], "DroneIT", 7);
        expect(
            mock_timerpoll_data_ready_cb, when(target, is_equal_to(&handle.super)),
            when(status, is_equal_to(0))
        );
        uv_run(loop, UV_RUN_ONCE);
        assert_that(handle.nread, is_equal_to(11));
        assert_that(
            &handle.rd_buf[0], is_equal_to_string("C10K"
                                                  "DroneIT")
        );
        write(pipe_fd_pair[1], "NandGate", 8);
        expect(
            mock_timerpoll_data_ready_cb, when(target, is_equal_to(&handle.super)),
            when(status, is_equal_to(0))
        );
        uv_run(loop, UV_RUN_ONCE);
        assert_that(handle.nread, is_equal_to(19));
        assert_that(
            &handle.rd_buf[0], is_equal_to_string("C10K"
                                                  "DroneIT"
                                                  "NandGate")
        );
    }
    {
        err = app_timer_poll_stop(&handle.super);
        assert_that(err, is_equal_to(0));
    }
    _app_timerpoll_test_teardown(loop, &handle.super, pipe_fd_pair);
} // end of app_timerpoll_target_fd_working_test

Ensure(app_timerpoll_target_fd_timeout_test) {
    uv_loop_t       *loop = uv_default_loop();
    test_timerpoll_t handle = {0};
    int              pipe_fd_pair[2] = {0};
    int              err = 0;
    _app_timerpoll_test_setup(loop, &handle.super, pipe_fd_pair);
    {
        uint64_t timeout_ms = 800;
        uint32_t expect_events = UV_READABLE;
        err = app_timer_poll_start(&handle.super, timeout_ms, expect_events, mock_timerpoll_data_ready_cb);
        assert_that(err, is_equal_to(0));
        write(pipe_fd_pair[1], "Haiyah", 6);
        expect(
            mock_timerpoll_data_ready_cb, when(target, is_equal_to(&handle.super)),
            when(status, is_equal_to(0))
        );
        uv_run(loop, UV_RUN_ONCE);
        assert_that(handle.nread, is_equal_to(6));
        assert_that(&handle.rd_buf[0], is_equal_to_string("Haiyah"));
        expect(
            mock_timerpoll_data_ready_cb, when(target, is_equal_to(&handle.super)),
            when(status, is_equal_to(UV_ETIMEDOUT))
        );
        uv_run(loop, UV_RUN_ONCE);
        // will turn off poll handle once timeout happened
    }
    {
        err = app_timer_poll_stop(&handle.super);
        assert_that(err, is_equal_to(0));
    }
    _app_timerpoll_test_teardown(loop, &handle.super, pipe_fd_pair);
} // end of app_timerpoll_target_fd_timeout_test
#undef PIPE_READ_BUF_BYTES

TestSuite *app_timer_poll_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, app_timerpoll_init_failure_test);
    add_test(suite, app_timerpoll_init_success_test);
    add_test(suite, app_timerpoll_uninit_failure_test);
    add_test(suite, app_timerpoll_target_fd_working_test);
    add_test(suite, app_timerpoll_target_fd_timeout_test);
    return suite;
} // end of app_timer_poll_tests
