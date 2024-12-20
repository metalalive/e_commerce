#include <cgreen/cgreen.h>
#include "network.h"

Ensure(resolve_net_addr_test) {
    struct addrinfo *ai = NULL;
    uint16_t expect_port = 8020;
    ai = resolve_net_addr(SOCK_STREAM, IPPROTO_TCP, "not.registered.domain.com", expect_port);
    assert_that(ai, is_equal_to(NULL));
    ai = resolve_net_addr(SOCK_DGRAM, IPPROTO_UDP, "localhost", expect_port);
    assert_that(ai, is_not_equal_to(NULL));
    // assert_that(ai->ai_next, is_equal_to(NULL));
    char actual_ip[INET_ADDRSTRLEN];
    inet_ntop(AF_INET, &((struct sockaddr_in *)ai->ai_addr)->sin_addr,
            (void *)&actual_ip[0], sizeof(actual_ip));
    uint16_t actual_port = htons(((struct sockaddr_in *)ai->ai_addr)->sin_port);
    printf("[debug] resolve_net_addr_test: actual_ip : %s \n", &actual_ip[0]);
    assert_that(strcmp("host.docker.internal", &actual_ip[0]) , is_equal_to(0));
    assert_that(strcmp("127.0.0.1", &actual_ip[0]) , is_equal_to(0));
    assert_that(expect_port, is_equal_to(actual_port));
    freeaddrinfo(ai);
} // end of resolve_net_addr_tests

static void _dummy_cb_on_nt_accept(uv_stream_t *server, int status)
{
}

Ensure(listener_access_test) {
    const char *expect_host = "localhost";
    uint16_t    expect_port = 8123;
    struct addrinfo *ai = NULL;
    app_cfg_listener_t **listeners = NULL;
    app_cfg_listener_t  *found = NULL, *_new = NULL;
    listeners = h2o_mem_alloc(sizeof(app_cfg_listener_t**));
    listeners[0] = NULL;
    found = find_existing_listener(listeners, ai);
    assert_that(found, is_equal_to(NULL));
    ai = resolve_net_addr(SOCK_STREAM, IPPROTO_TCP, expect_host, expect_port);
    assert_that(ai, is_not_equal_to(NULL));
    uv_tcp_t *nt_handle = create_network_handle(uv_default_loop(), ai, _dummy_cb_on_nt_accept, 64);
    assert_that(nt_handle, is_not_equal_to(NULL));
    _new = create_new_listener((uv_handle_t *)nt_handle);
    assert_that(_new, is_not_equal_to(NULL));
    h2o_append_to_null_terminated_list((void ***)&listeners, (void *)_new);
    assert_that(_new, is_equal_to(listeners[0]));
    found = find_existing_listener(listeners, ai);
    assert_that(found, is_not_equal_to(NULL));
    assert_that(found, is_equal_to(listeners[0]));
    // ----
    destroy_network_handle((uv_handle_t *)listeners[0]->nt_handle, (uv_close_cb)free);
    uv_run(uv_default_loop(), UV_RUN_ONCE);
    free_listener(listeners[0]);
    free(listeners);
    freeaddrinfo(ai);
} // end of listener_access_test


Ensure(atomic_num_conn_test) {
    int new_val = 0, idx = 0;
    app_cfg_t app_cfg = {.state = {.num_curr_connections = 0}};
    for(idx = 0; idx < 5; idx++) {
        new_val = atomic_num_connections(&app_cfg, 1);
        assert_that(new_val, is_equal_to(idx));
    }
    new_val = atomic_num_connections(&app_cfg, 0);
    assert_that(new_val, is_equal_to(idx));
    for(idx = 0; idx < 5; idx++) {
        new_val = atomic_num_connections(&app_cfg, -1);
        assert_that(new_val, is_equal_to(5 - idx));
    }
}// end of atomic_num_conn_test


TestSuite *app_network_util_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, resolve_net_addr_test);
    add_test(suite, listener_access_test);
    add_test(suite, atomic_num_conn_test);
    return suite;
}

