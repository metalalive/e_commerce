#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <amqp_tcp_socket.h>
#include "rpc/core.h"

Ensure(rpc_core_init_test__memory_error) {
#define  NUM_RPC_CFGS  2
    void *ctx = NULL;
    arpc_cfg_t  cfgs[NUM_RPC_CFGS] = {0};
    // amqp_socket_t, struct amqp_connection_state_t_  are incomplete type, which is
    // very inconvenient for testing cuz we cannot see the structure detail.
    amqp_socket_t *mock_socket = malloc(34);
    amqp_connection_state_t mock_conn_state = malloc(45);
    // assume entry #1 has connection error
    expect(amqp_new_connection, will_return(NULL));
    expect(amqp_tcp_socket_new, will_return(mock_socket));
    // assume entry #2 has socket error
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_tcp_socket_new, will_return(NULL));
    ctx = app_rpc_conn_init(&cfgs[0], NUM_RPC_CFGS);
    assert_that(ctx, is_not_null);
    if(ctx) {
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_conn_state)));
        expect(amqp_destroy_connection, will_return(0), when(conn_state, is_equal_to(mock_conn_state)));
        app_rpc_conn_deinit(ctx);
    }
    free(mock_conn_state);
#undef   NUM_RPC_CFGS
} // end of rpc_core_init_test__memory_error

Ensure(rpc_core_init_test__invalid_host) {
#define  INVALID_HOST  "bad.evil.host.com"
#define  INVALID_PORT  23456
    void *ctx = NULL;
    arpc_cfg_t cfg = {.credential = {.host=INVALID_HOST, .port=INVALID_PORT}};
    amqp_socket_t *mock_socket = malloc(34);
    amqp_connection_state_t mock_conn_state = malloc(45);
    expect(amqp_tcp_socket_new, will_return(mock_socket));
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_socket_open, will_return(AMQP_STATUS_BAD_URL),
            when(host, is_equal_to_string(INVALID_HOST)),
            when(port, is_equal_to(INVALID_PORT)) );
    expect(amqp_connection_close, when(conn_state, is_equal_to(mock_conn_state)));
    expect(amqp_destroy_connection, will_return(0), when(conn_state, is_equal_to(mock_conn_state)));
    ctx = app_rpc_conn_init(&cfg, 1);
    free(mock_socket);
    free(mock_conn_state);
    free(ctx);
#undef  INVALID_HOST
#undef  INVALID_PORT
} // end of rpc_core_init_test__invalid_host

Ensure(rpc_core_init_test__login_failure) {
    void *ctx = NULL;
    arpc_cfg_t cfg = {.credential = {.username="kafka", .host="another.host", .port=987}};
    amqp_socket_t *mock_socket = malloc(34);
    amqp_connection_state_t mock_conn_state = malloc(45);
    amqp_rpc_reply_t mock_reply = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION};
    expect(amqp_tcp_socket_new, will_return(mock_socket));
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_socket_open, will_return(AMQP_STATUS_OK));
    expect(amqp_login, will_return(&mock_reply));
    expect(amqp_connection_close, when(conn_state, is_equal_to(mock_conn_state)));
    expect(amqp_destroy_connection, will_return(0), when(conn_state, is_equal_to(mock_conn_state)));
    ctx = app_rpc_conn_init(&cfg, 1);
    {
        app_rpc_conn_deinit(ctx);
        free(mock_socket);
        free(mock_conn_state);
    }
} // end of rpc_core_init_test__login_failure

Ensure(rpc_core_init_test__declare_queue_error) {
    void *ctx = NULL;
    arpc_cfg_bind_t bind_cfg = {.q_name="my_queue"};
    arpc_cfg_t cfg = {.bindings = {.size=1, .entries=&bind_cfg}};
    amqp_socket_t *mock_socket = malloc(34);
    amqp_connection_state_t mock_conn_state = malloc(45);
    amqp_channel_open_ok_t   mock_chn_result = {.channel_id = {.len=4 , .bytes=(void*)"RTYU"}};
    amqp_queue_declare_ok_t  mock_newq_result = {0};
    amqp_rpc_reply_t mock_reply_login = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_channel_close_t chn_err_newq = {.reply_code = 403,
        .reply_text = {.len = 9, .bytes = (void*)"no permit"}};
    amqp_rpc_reply_t mock_reply_newq = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
        .reply = {.decoded = (void *)&chn_err_newq}
    };
    expect(amqp_tcp_socket_new, will_return(mock_socket));
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_socket_open, will_return(AMQP_STATUS_OK));
    expect(amqp_login, will_return(&mock_reply_login));
    expect(amqp_channel_open, will_return(&mock_chn_result));
    expect(amqp_queue_declare, will_return(&mock_newq_result));
    expect(amqp_get_rpc_reply, will_return(&mock_reply_newq));
    ctx = app_rpc_conn_init(&cfg, 1);
    { // the connection object won't be destroyed even if app failed to declare queue
        expect(amqp_connection_close);
        expect(amqp_destroy_connection, will_return(0));
        app_rpc_conn_deinit(ctx);
        free(mock_socket);
        free(mock_conn_state);
    }
} // end of rpc_core_init_test__declare_queue_error

Ensure(rpc_core_init_test__binding_error) {
    void *ctx = NULL;
    arpc_cfg_bind_t bind_cfg = {.q_name="your_queue", .exchange_name="app_ex",
         .routing_key="app.some_function.10ijkace" };
    arpc_cfg_t cfg = {.bindings = {.size=1, .entries=&bind_cfg}};
    amqp_socket_t *mock_socket = malloc(34);
    amqp_connection_state_t mock_conn_state = malloc(45);
    amqp_channel_open_ok_t   mock_chn_result = {.channel_id = {.len=4 , .bytes=(void*)"RTYU"}};
    amqp_queue_declare_ok_t  mock_newq_return = {0};
    amqp_queue_bind_ok_t     mock_bind_return = {0};
    amqp_rpc_reply_t mock_reply_login = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_rpc_reply_t mock_reply_newq  = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_rpc_reply_t mock_reply_bind  = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION};
    expect(amqp_tcp_socket_new, will_return(mock_socket));
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_socket_open, will_return(AMQP_STATUS_OK));
    expect(amqp_login, will_return(&mock_reply_login));
    expect(amqp_channel_open, will_return(&mock_chn_result));
    expect(amqp_queue_declare, will_return(&mock_newq_return));
    expect(amqp_get_rpc_reply, will_return(&mock_reply_newq));
    expect(amqp_queue_bind, will_return(&mock_bind_return));
    expect(amqp_get_rpc_reply, will_return(&mock_reply_bind));
    ctx = app_rpc_conn_init(&cfg, 1);
    { // the connection object won't be destroyed even when binding failure happened
        expect(amqp_connection_close);
        expect(amqp_destroy_connection, will_return(0));
        app_rpc_conn_deinit(ctx);
        free(mock_socket);
        free(mock_conn_state);
    }
} // end of rpc_core_init_test__binding_error


TestSuite *app_rpc_core_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_core_init_test__memory_error);
    add_test(suite, rpc_core_init_test__invalid_host);
    add_test(suite, rpc_core_init_test__login_failure);
    add_test(suite, rpc_core_init_test__declare_queue_error);
    add_test(suite, rpc_core_init_test__binding_error);
    return suite;
}
