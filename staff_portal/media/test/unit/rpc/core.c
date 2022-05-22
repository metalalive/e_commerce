#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
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


static __attribute__((optimize("O0"))) ARPC_STATUS_CODE utest1_arpc_replyq_render_fn(
        const char *pattern, arpc_exe_arg_t *args, char *wr_buf, size_t wr_sz)
{ return (ARPC_STATUS_CODE)mock(pattern, args, wr_buf, wr_sz); }


#define MAX_BYTES_JOB_ID    70
Ensure(rpc_start_test__cfg_missing) {
    char job_id_raw[MAX_BYTES_JOB_ID] = {0};
#pragma GCC diagnostic ignored "-Wint-conversion"
    void *dummy[2] = {0x123, 0x456};
#pragma GCC diagnostic pop
    amqp_socket_t  *mock_mq_sock = (amqp_socket_t  *)dummy[0];
    amqp_connection_state_t  mock_mq_conn = (amqp_connection_state_t)dummy[1];
    arpc_cfg_bind_t mock_bind_cfg = {.routing_key="rpc.media.utest_operation_2", .reply={
        .queue={.name_pattern="xyz123", .render_fn=utest1_arpc_replyq_render_fn } }};
    arpc_cfg_t  mock_cfg = {.alias="utest_mqbroker_2", .bindings={.capacity=1,
        .size=1, .entries=&mock_bind_cfg }};
    struct arpc_ctx_t mock_ctx = {.ref_cfg=&mock_cfg, .sock=mock_mq_sock, .conn=mock_mq_conn};
    struct arpc_ctx_list_t mock_ctx_lst = {.size=1, .entries=&mock_ctx};
    arpc_exe_arg_t  rpc_arg = {
        .conn=(void *)&mock_ctx_lst, .job_id = {.bytes=&job_id_raw[0], .len=MAX_BYTES_JOB_ID },
        .msg_body = {.len=2, .bytes="{}"},  .alias = "utest_mqbroker_1", .usr_data = NULL,
        .routing_key = "rpc.media.utest_operation_1",
    };
    // subcase #1, RPC config not found
    ARPC_STATUS_CODE status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_MSGQ_CONNECTION_ERROR));
    // subcase #2, RPC config found, binding information not found
    rpc_arg.alias = mock_cfg.alias;
    status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_ARG_ERROR));
    // subcase #3, RPC config and binding information found, but render function goes wrong
    rpc_arg.routing_key = mock_cfg.bindings.entries[0].routing_key;
    expect(utest1_arpc_replyq_render_fn, will_return(APPRPC_RESP_OS_ERROR));
    status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_OS_ERROR));
    assert_that(rpc_arg.job_id.bytes, is_equal_to_string(""));
} // end of rpc_start_test__cfg_missing

Ensure(rpc_start_test__reconnected_published) {
    const char *msg_body_raw = "{\"rpc_field_1\": \"some_str_value\", \"rpc_field_2\": 98760}";
    size_t msg_body_raw_sz = strlen(msg_body_raw);
    char job_id_raw[MAX_BYTES_JOB_ID] = {0};
#pragma GCC diagnostic ignored "-Wint-conversion"
    void *dummy[2] = {0x123, 0x456};
#pragma GCC diagnostic pop
    amqp_socket_t  *mock_mq_sock = (amqp_socket_t  *)dummy[0];
    amqp_connection_state_t  mock_mq_conn = (amqp_connection_state_t)dummy[1];
    arpc_cfg_bind_t mock_bind_cfg = {.routing_key="rpc.media.utest_operation_1", .exchange_name="exc257",
        .reply={ .queue={.name_pattern="xyz123", .render_fn=NULL},
            .correlation_id={.render_fn=NULL, .name_pattern="beeBubble"} }};
    arpc_cfg_t  mock_cfg = {.alias="utest_mqbroker_1", .bindings={.capacity=1,
        .size=1, .entries=&mock_bind_cfg }};
    struct arpc_ctx_t mock_ctx = {.ref_cfg=&mock_cfg, .sock=mock_mq_sock, .conn=mock_mq_conn};
    struct arpc_ctx_list_t mock_ctx_lst = {.size=1, .entries=&mock_ctx};
    arpc_exe_arg_t  rpc_arg = {
        .conn=(void *)&mock_ctx_lst, .job_id = {.bytes=&job_id_raw[0], .len=MAX_BYTES_JOB_ID },
        .msg_body = {.len=msg_body_raw_sz, .bytes=msg_body_raw},  .alias=mock_cfg.alias,
        .usr_data = NULL,  .routing_key=mock_bind_cfg.routing_key,
    };
    {
        amqp_rpc_reply_t mock_reply_conn_err = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
            .library_error=AMQP_STATUS_HEARTBEAT_TIMEOUT};
        amqp_rpc_reply_t mock_reply_ok = {.reply_type=AMQP_RESPONSE_NORMAL};
        amqp_channel_open_ok_t   mock_chn_result = {.channel_id = {.len=4 , .bytes=(void*)"RTYU"}};
        expect(amqp_queue_declare);
        expect(amqp_get_rpc_reply, will_return(&mock_reply_conn_err));
        expect(amqp_error_string2, will_return(""));
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_destroy_connection);
        expect(amqp_new_connection, will_return(mock_mq_conn));
        expect(amqp_tcp_socket_new, will_return(mock_mq_sock));
        expect(amqp_socket_open, will_return(AMQP_STATUS_OK));
        expect(amqp_login, will_return(&mock_reply_ok));
        expect(amqp_channel_open, will_return(&mock_chn_result));
        expect(amqp_queue_declare);
        expect(amqp_get_rpc_reply, will_return(&mock_reply_ok));
        expect(amqp_basic_publish, will_return(AMQP_STATUS_OK), when(raw_body, is_equal_to_string(msg_body_raw)));
    }
    assert_that(rpc_arg.job_id.bytes, is_equal_to_string(""));
    ARPC_STATUS_CODE status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_ACCEPTED));
    assert_that(rpc_arg.job_id.bytes, is_equal_to_string(mock_bind_cfg.reply.correlation_id.name_pattern));
} // end of rpc_start_test__reconnected_published


Ensure(rpc_start_test__publish_failure) {
    char job_id_raw[MAX_BYTES_JOB_ID] = {0};
#pragma GCC diagnostic ignored "-Wint-conversion"
    void *dummy[2] = {0x123, 0x456};
#pragma GCC diagnostic pop
    amqp_socket_t  *mock_mq_sock = (amqp_socket_t  *)dummy[0];
    amqp_connection_state_t  mock_mq_conn = (amqp_connection_state_t)dummy[1];
    arpc_cfg_bind_t mock_bind_cfg = {.routing_key="rpc.media.utest_operation_1", .exchange_name="exc257",
        .reply={ .queue={.name_pattern="xyz123", .render_fn=NULL},
            .correlation_id={.render_fn=NULL, .name_pattern="beeBubble"} }};
    arpc_cfg_t  mock_cfg = {.alias="utest_mqbroker_1", .bindings={.capacity=1,
        .size=1, .entries=&mock_bind_cfg }};
    struct arpc_ctx_t mock_ctx = {.ref_cfg=&mock_cfg, .sock=mock_mq_sock, .conn=mock_mq_conn};
    struct arpc_ctx_list_t mock_ctx_lst = {.size=1, .entries=&mock_ctx};
    arpc_exe_arg_t  rpc_arg = {
        .conn=(void *)&mock_ctx_lst, .job_id = {.bytes=&job_id_raw[0], .len=MAX_BYTES_JOB_ID },
        .msg_body = {.len=2, .bytes="{}"},  .alias=mock_cfg.alias,
        .usr_data = NULL,  .routing_key=mock_bind_cfg.routing_key,
    };
    {
        amqp_rpc_reply_t mock_reply_ok = {.reply_type=AMQP_RESPONSE_NORMAL};
        amqp_rpc_reply_t mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
            .reply={.id=AMQP_BASIC_PUBLISH_METHOD}};
        expect(amqp_queue_declare);
        expect(amqp_get_rpc_reply, will_return(&mock_reply_ok));
        expect(amqp_basic_publish, will_return(AMQP_STATUS_TABLE_TOO_BIG));
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
    }
    ARPC_STATUS_CODE status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_MSGQ_PUBLISH_ERROR));
} // end of rpc_start_test__publish_failure


Ensure(rpc_start_test__msg_broker_down) {
    const char *msg_body_raw = "{\"rpc_field_1\": \"some_str_value\", \"rpc_field_2\": 98760}";
    size_t msg_body_raw_sz = strlen(msg_body_raw);
    char job_id_raw[MAX_BYTES_JOB_ID] = {0};
#pragma GCC diagnostic ignored "-Wint-conversion"
    void *dummy[2] = {0x123, 0x456};
#pragma GCC diagnostic pop
    amqp_socket_t  *mock_mq_sock = (amqp_socket_t  *)dummy[0];
    amqp_connection_state_t  mock_mq_conn = (amqp_connection_state_t)dummy[1];
    arpc_cfg_bind_t mock_bind_cfg = {.routing_key="rpc.media.utest_operation_1", .exchange_name="exc257",
        .reply={ .queue={.name_pattern="xyz123", .render_fn=NULL},
            .correlation_id={.render_fn=NULL, .name_pattern="beeBubble"} }};
    arpc_cfg_t  mock_cfg = {.alias="utest_mqbroker_1", .bindings={.capacity=1,
        .size=1, .entries=&mock_bind_cfg }};
    struct arpc_ctx_t mock_ctx = {.ref_cfg=&mock_cfg, .sock=mock_mq_sock, .conn=mock_mq_conn};
    struct arpc_ctx_list_t mock_ctx_lst = {.size=1, .entries=&mock_ctx};
    arpc_exe_arg_t  rpc_arg = {
        .conn=(void *)&mock_ctx_lst, .job_id = {.bytes=&job_id_raw[0], .len=MAX_BYTES_JOB_ID },
        .msg_body = {.len=msg_body_raw_sz, .bytes=msg_body_raw},  .alias=mock_cfg.alias,
        .usr_data = NULL,  .routing_key=mock_bind_cfg.routing_key,
    };
    {
        amqp_rpc_reply_t mock_reply_conn_err = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
            .library_error=AMQP_STATUS_HEARTBEAT_TIMEOUT};
        expect(amqp_queue_declare);
        expect(amqp_get_rpc_reply, will_return(&mock_reply_conn_err));
        expect(amqp_error_string2, will_return(""));
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_destroy_connection);
        expect(amqp_new_connection, will_return(mock_mq_conn));
        expect(amqp_tcp_socket_new, will_return(mock_mq_sock));
        expect(amqp_socket_open, will_return(AMQP_STATUS_SOCKET_ERROR));
    }
    assert_that(rpc_arg.job_id.bytes, is_equal_to_string(""));
    ARPC_STATUS_CODE status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_MSGQ_CONNECTION_ERROR));
} // end of rpc_start_test__msg_broker_down

TestSuite *app_rpc_core_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_core_init_test__memory_error);
    add_test(suite, rpc_core_init_test__invalid_host);
    add_test(suite, rpc_core_init_test__login_failure);
    add_test(suite, rpc_core_init_test__declare_queue_error);
    add_test(suite, rpc_core_init_test__binding_error);
    add_test(suite, rpc_start_test__cfg_missing);
    add_test(suite, rpc_start_test__reconnected_published);
    add_test(suite, rpc_start_test__publish_failure);
    add_test(suite, rpc_start_test__msg_broker_down);
    return suite;
}
