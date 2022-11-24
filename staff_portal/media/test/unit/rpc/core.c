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
    amqp_connection_state_t mock_conn_state = malloc(45);
    // assume entry #1 has connection error
    expect(amqp_new_connection, will_return(NULL));
    // assume entry #2 has socket error
    expect(amqp_new_connection, will_return(mock_conn_state));
    expect(amqp_tcp_socket_new, will_return(NULL));
    expect(amqp_connection_close, when(conn_state, is_equal_to(mock_conn_state)));
    expect(amqp_destroy_connection, will_return(0), when(conn_state, is_equal_to(mock_conn_state)));
    ctx = app_rpc_conn_init(&cfgs[0], NUM_RPC_CFGS);
    assert_that(ctx, is_not_null);
    if(ctx) {
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
        .msg_body = {.len=msg_body_raw_sz, .bytes=(void *)msg_body_raw},  .alias=mock_cfg.alias,
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
    assert_that(status, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
} // end of rpc_start_test__publish_failure


Ensure(rpc_start_test__publish_msg_broker_down) {
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
        .msg_body = {.len=msg_body_raw_sz, .bytes=(void *)msg_body_raw},  .alias=mock_cfg.alias,
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
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_destroy_connection, will_return(0), when(conn_state, is_equal_to(mock_mq_conn)));
    }
    assert_that(rpc_arg.job_id.bytes, is_equal_to_string(""));
    ARPC_STATUS_CODE status = app_rpc_start(&rpc_arg);
    assert_that(status, is_equal_to(APPRPC_RESP_MSGQ_CONNECTION_ERROR));
} // end of rpc_start_test__publish_msg_broker_down

Ensure(rpc_ctx_lookup_test) {
#define  NUM_RPC_CFGS  3
    arpc_cfg_t  mock_cfgs[NUM_RPC_CFGS] = {
        {.alias="utest_mqbroker_1"}, {.alias="utest_mqbroker_2"}, {.alias="utest_mqbroker_3"}
    };
    struct arpc_ctx_t      mock_ctxs[NUM_RPC_CFGS] = {
        {.ref_cfg=&mock_cfgs[0]}, {.ref_cfg=&mock_cfgs[1]}, {.ref_cfg=&mock_cfgs[2]}
    };
    struct arpc_ctx_list_t mock_ctx_lst = {.size=NUM_RPC_CFGS, .entries=&mock_ctxs[0]};
    void *chosen_ctx = NULL;
    chosen_ctx = app_rpc_context_lookup((void *)&mock_ctx_lst, "utest_mqbroker_6789");
    assert_that(chosen_ctx, is_null);
    chosen_ctx = app_rpc_context_lookup((void *)&mock_ctx_lst, "utest_mqbroker_2");
    assert_that(chosen_ctx, is_equal_to(&mock_ctxs[1]));
#undef  NUM_RPC_CFGS
} // end of rpc_ctx_lookup_test

Ensure(rpc_consume_test__cfg_error) {
    char dummy[2] = {0};
    arpc_cfg_bind_t bindcfg = {.q_name="utest_rpc_method_queue"};
    arpc_cfg_t  mock_cfg = {.bindings={.entries=&bindcfg, .size=1}};
    struct arpc_ctx_t mock_ctxs = {.consumer_setup_done=0, .ref_cfg=&mock_cfg,
        .conn = (void *)&dummy[0]};
    amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
            .reply={.id=AMQP_BASIC_CONSUME_METHOD}};
    expect(amqp_basic_consume, when(conn_state, is_equal_to(&dummy[0])),
            when(q_name, is_equal_to("utest_rpc_method_queue")) );
    expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
    expect(amqp_destroy_envelope);
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[1]);
    assert_that(res, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
} // end of rpc_consume_test__cfg_error

Ensure(rpc_consume_test__empty_queue) {
    char dummy[2] = {0};
    arpc_cfg_t  mock_cfg = {0};
    struct arpc_ctx_t mock_ctxs = {.consumer_setup_done=1, .ref_cfg=&mock_cfg,
        .conn = (void *)&dummy[0]};
    amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
            .library_error=AMQP_STATUS_TIMEOUT};
    expect(amqp_maybe_release_buffers);
    expect(amqp_consume_message, will_return(&mock_reply_err));
    expect(amqp_destroy_envelope);
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[1]);
    assert_that(res, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_TIMEOUT));
} // end of rpc_consume_test__empty_queue

Ensure(rpc_consume_test__unknown_route) {
    char dummy[2] = {0};
    const char *mock_route_key = "abcde";
    size_t mock_route_key_sz = strlen(mock_route_key);
    arpc_cfg_t  mock_cfg = {0};
    struct arpc_ctx_t mock_ctxs = {.consumer_setup_done=1, .ref_cfg=&mock_cfg,
        .conn = (void *)&dummy[0]};
    amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
    expect(amqp_maybe_release_buffers);
    expect(amqp_consume_message,  will_return(&mock_reply_ok),
            will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
            will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
        );
    expect(amqp_destroy_envelope);
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[1]);
    assert_that(res, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
} // end of rpc_consume_test__unknown_route

Ensure(rpc_consume_test__missing_handler) {
#define EXPECT_NUM_BINDINGS 3
    char dummy[2] = {0};
    arpc_cfg_bind_t bindcfg[EXPECT_NUM_BINDINGS] = {
        {.routing_key="utest.rpc.operation1"},
        {.routing_key="utest.rpc.operation2"},
        {.routing_key="utest.rpc.operation3"},
    };
    const char *mock_route_key = bindcfg[2].routing_key;
    size_t mock_route_key_sz = strlen(mock_route_key);
    arpc_cfg_t  mock_cfg = {.bindings={.entries=&bindcfg[0], .size=EXPECT_NUM_BINDINGS}};
    struct arpc_ctx_t mock_ctxs = {.consumer_setup_done=1, .ref_cfg=&mock_cfg, .conn = (void *)&dummy[0]};
    amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
    expect(amqp_maybe_release_buffers);
    expect(amqp_consume_message,  will_return(&mock_reply_ok),
            will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
            will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
        );
    expect(amqp_destroy_envelope);
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[1]);
    assert_that(res, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__missing_handler


static void utest_mock_consumer_handler(arpc_receipt_t *r)
{
    size_t recv_msg_sz = r->msg_body.len;
    char  *recv_msg    = r->msg_body.bytes;
    char  *middle_body   =  NULL;
    char **middle_body_p = &middle_body;
    char  *return_body   =  "qwertyuh";
    char **return_body_p = &return_body;
    mock(r, recv_msg_sz, recv_msg, return_body_p, middle_body_p);
    if(middle_body)
        r->send_fn(r, middle_body, strlen(middle_body));
    r->return_fn(r, return_body, strlen(return_body));
} // end of utest_mock_consumer_handler


#define  UTEST_RPC_CONSUME__HANDLER_DONE_SETUP \
    char dummy[3] = {0}; \
    arpc_cfg_bind_t bindcfg[EXPECT_NUM_BINDINGS] = { \
        {.routing_key="utest.rpc.operation.1234", .reply={.task_handler=utest_mock_consumer_handler}}, \
    }; \
    const char *mock_route_key = bindcfg[0].routing_key; \
    size_t mock_route_key_sz = strlen(mock_route_key); \
    arpc_cfg_t  mock_cfg = {.bindings={.entries=&bindcfg[0], .size=EXPECT_NUM_BINDINGS}}; \
    struct arpc_ctx_t mock_ctxs = {.consumer_setup_done=1, .ref_cfg=&mock_cfg, .conn=(void *)&dummy[0] \
        , .sock=(void *)&dummy[1]};



Ensure(rpc_consume_test__handler_done__broker_down) {
#define EXPECT_NUM_BINDINGS 1
    UTEST_RPC_CONSUME__HANDLER_DONE_SETUP;
    amqp_bytes_t expect_recv_msg = {.len=11, .bytes="Wuhan virus"};
    {
        amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
        amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
                .library_error=AMQP_STATUS_SOCKET_ERROR};
        expect(amqp_maybe_release_buffers);
        expect(amqp_consume_message,  will_return(&mock_reply_ok),
                will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
                will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
                will_set_contents_of_parameter(evp_msg_body, &expect_recv_msg, sizeof(amqp_bytes_t)),
            );
        expect(utest_mock_consumer_handler,   when(recv_msg_sz, is_equal_to(expect_recv_msg.len)),
                when(recv_msg, is_equal_to_string(expect_recv_msg.bytes))
            );
        expect(amqp_basic_publish, will_return(AMQP_STATUS_SOCKET_ERROR));
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
        expect(amqp_connection_close);
        expect(amqp_destroy_connection);
        expect(amqp_new_connection, will_return(&dummy[0]));
        expect(amqp_tcp_socket_new, will_return(&dummy[1]));
        expect(amqp_socket_open, will_return(AMQP_STATUS_TCP_ERROR));
        expect(amqp_connection_close);
        expect(amqp_destroy_connection);
        expect(amqp_destroy_envelope);
    }
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[2]);
    assert_that(res, is_equal_to(APPRPC_RESP_OK));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__handler_done__broker_down


Ensure(rpc_consume_test__handler_finalize_reply_error) {
#define EXPECT_NUM_BINDINGS 1
    UTEST_RPC_CONSUME__HANDLER_DONE_SETUP;
    {
        amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
        amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
                .reply={.id=AMQP_BASIC_PUBLISH_METHOD} };
        expect(amqp_maybe_release_buffers);
        expect(amqp_consume_message,  will_return(&mock_reply_ok),
                will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
                will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
            );
        expect(utest_mock_consumer_handler);
        expect(amqp_basic_publish, will_return(AMQP_STATUS_BAD_AMQP_DATA));
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
        expect(amqp_destroy_envelope);
    }
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[2]);
    assert_that(res, is_equal_to(APPRPC_RESP_OK));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__handler_finalize_reply_error


Ensure(rpc_consume_test__handler_finalize_reply_ok) {
#define EXPECT_NUM_BINDINGS 1
    UTEST_RPC_CONSUME__HANDLER_DONE_SETUP;
    amqp_bytes_t expect_recv_msg = {.len=11, .bytes="cyberattack"};
    const char *expect_return_body = "you are almost there, keep digging";
    {
        amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
        expect(amqp_maybe_release_buffers);
        expect(amqp_consume_message,  will_return(&mock_reply_ok),
                will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
                will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
                will_set_contents_of_parameter(evp_msg_body, &expect_recv_msg, sizeof(amqp_bytes_t)),
            );
        expect(utest_mock_consumer_handler,  when(recv_msg_sz, is_equal_to(expect_recv_msg.len)),
                when(recv_msg, is_equal_to_string(expect_recv_msg.bytes)),
                will_set_contents_of_parameter(return_body_p, &expect_return_body, sizeof(char *))  );
        expect(amqp_basic_publish, will_return(AMQP_STATUS_OK),
                when(raw_body, is_equal_to_string(expect_return_body)));
        expect(amqp_destroy_envelope);
    }
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[2]);
    assert_that(res, is_equal_to(APPRPC_RESP_OK));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__handler_finalize_reply_ok


Ensure(rpc_consume_test__handler_middle_reply_error) {
#define EXPECT_NUM_BINDINGS 1
    UTEST_RPC_CONSUME__HANDLER_DONE_SETUP;
    const char *expect_middle_body = "handler has processed and sending message in the middle";
    const char *expect_return_body = "handler has done the task, returing the message";
    {
        amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
        amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
                .reply={.id=AMQP_BASIC_PUBLISH_METHOD} };
        expect(amqp_maybe_release_buffers);
        expect(amqp_consume_message,  will_return(&mock_reply_ok),
                will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
                will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
            );
        expect(utest_mock_consumer_handler,
                will_set_contents_of_parameter(return_body_p, &expect_return_body, sizeof(char *)),
                will_set_contents_of_parameter(middle_body_p, &expect_middle_body, sizeof(char *))
              );
        // assume the first message failed to deliver
        expect(amqp_basic_publish, will_return(AMQP_STATUS_BAD_AMQP_DATA),
                when(raw_body, is_equal_to_string(expect_middle_body)));
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
        // assume the second message delivered successfully
        expect(amqp_basic_publish, will_return(AMQP_STATUS_OK),
                when(raw_body, is_equal_to_string(expect_return_body)));
        expect(amqp_destroy_envelope);
    }
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[2]);
    assert_that(res, is_equal_to(APPRPC_RESP_OK));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__handler_middle_reply_error


Ensure(rpc_consume_test__handler_middle_reply_ok) {
#define EXPECT_NUM_BINDINGS 1
    UTEST_RPC_CONSUME__HANDLER_DONE_SETUP;
    amqp_bytes_t expect_recv_msg = {.len=6, .bytes="Wu-Mao"};
    const char *expect_middle_body = "handler has processed and sending message in the middle";
    const char *expect_return_body = "handler has done the task, returing the message";
    {
        amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
        expect(amqp_maybe_release_buffers);
        expect(amqp_consume_message,  will_return(&mock_reply_ok),
                will_set_contents_of_parameter(evp_routekey, (void **)&mock_route_key, sizeof(void *)),
                will_set_contents_of_parameter(evp_routekey_sz, &mock_route_key_sz, sizeof(size_t)),
                will_set_contents_of_parameter(evp_msg_body, &expect_recv_msg, sizeof(amqp_bytes_t)),
            );
        expect(utest_mock_consumer_handler, when(recv_msg_sz, is_equal_to(expect_recv_msg.len)),
                when(recv_msg, is_equal_to_string(expect_recv_msg.bytes)),
                will_set_contents_of_parameter(return_body_p, &expect_return_body, sizeof(char *)),
                will_set_contents_of_parameter(middle_body_p, &expect_middle_body, sizeof(char *))
              );
        expect(amqp_basic_publish, will_return(AMQP_STATUS_OK),
                when(raw_body, is_equal_to_string(expect_middle_body)));
        expect(amqp_basic_publish, will_return(AMQP_STATUS_OK),
                when(raw_body, is_equal_to_string(expect_return_body)));
        expect(amqp_destroy_envelope);
    }
    ARPC_STATUS_CODE res = app_rpc_consume_message((void *)&mock_ctxs, (void *)&dummy[2]);
    assert_that(res, is_equal_to(APPRPC_RESP_OK));
#undef EXPECT_NUM_BINDINGS
} // end of rpc_consume_test__handler_middle_reply_ok




typedef struct {
    amqp_bytes_t   corr_id;
    amqp_bytes_t   msg;
} ut_consume_evp_t;

#define  UT_EVP_ITEM_INIT(_corr_id_str, _msg_str)   { \
    .corr_id = {.bytes=_corr_id_str, .len=strlen(_corr_id_str)}, \
    .msg = {.bytes=_msg_str,  .len=strlen(_msg_str)} \
}

#define  NUM_BIND_CFG   2
#define  UTEST_FETCH_REPLYQ__SETUP  \
    char dummy[2] = {0x12, 0x45}; \
    amqp_socket_t  *mock_mq_sock = (amqp_socket_t  *)&dummy[0]; \
    amqp_connection_state_t  mock_mq_conn = (amqp_connection_state_t)&dummy[1]; \
    arpc_cfg_bind_t  mock_bind_cfgs[NUM_BIND_CFG] = { \
        {.reply={.queue={.name_pattern="app.op.xyz123"}}}, \
        {.reply={.queue={.name_pattern="app.op.uvw345"}}}, \
    }; \
    arpc_cfg_t  mock_cfg = {.alias="utest_mqbroker_1", .bindings={.capacity=NUM_BIND_CFG, \
        .size=NUM_BIND_CFG, .entries=&mock_bind_cfgs[0] }}; \
    struct arpc_ctx_t  mock_ctx = {.ref_cfg=&mock_cfg, .sock=mock_mq_sock, .conn=mock_mq_conn}; \
    struct arpc_ctx_list_t  mock_ctx_lst = {.size=1, .entries=&mock_ctx}; \
    arpc_exe_arg_t  mock_rpc_arg = {.conn=(void *)&mock_ctx_lst, .alias=mock_cfg.alias, .usr_data=NULL};

static void  utest_rpc_fetch_from_replyq_cb (arpc_cfg_t *cfg, arpc_exe_arg_t *args)
{
    ut_consume_evp_t  *expect_evps = args->usr_data;
    assert_that(args->msg_body.bytes, is_equal_to_string(expect_evps[0].msg.bytes));
    assert_that(args->job_id.bytes, is_equal_to_string(expect_evps[0].corr_id.bytes));
    mock(cfg, args);
    args->usr_data = expect_evps += 1; // advanced to next object in array
} // end of utest_rpc_fetch_from_replyq_cb

#define  NUM_EVPS_REPLYQ1  3
#define  NUM_EVPS_REPLYQ2  4
#define  TOTAL_NUM_MSGS   (NUM_EVPS_REPLYQ1 + NUM_EVPS_REPLYQ2)
Ensure(rpc_replyq_test__get_all_msgs)
{
    UTEST_FETCH_REPLYQ__SETUP
    amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_rpc_reply_t  mock_reply_timeout = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
                .library_error=AMQP_STATUS_TIMEOUT};
    int  chosen_evp_idx = 0;
    ut_consume_evp_t  expect_evps[TOTAL_NUM_MSGS + 1] = {
        UT_EVP_ITEM_INIT("bike", "msg001"),      UT_EVP_ITEM_INIT("freight", "msg002"),
        UT_EVP_ITEM_INIT("chip", "msg003"),      UT_EVP_ITEM_INIT("cream", "msg004"),
        UT_EVP_ITEM_INIT("cracket", "msg005"),   UT_EVP_ITEM_INIT("corn", "msg006"),
        UT_EVP_ITEM_INIT("pie", "msg007"),       UT_EVP_ITEM_INIT("", ""),
    };
    mock_rpc_arg.usr_data = &expect_evps[0];
#define RUN_CODE(cfg_idx, num_evps, timeout_detection) { \
    const char *expect_q_name = mock_bind_cfgs[cfg_idx].reply.queue.name_pattern; \
    expect(amqp_basic_consume, when(conn_state, is_equal_to(mock_mq_conn)), \
        when(q_name, is_equal_to_string(expect_q_name)) ); \
    expect(amqp_get_rpc_reply, will_return(&mock_reply_ok)); \
    for(int jdx = 0; jdx < num_evps; jdx++) { \
        expect(amqp_maybe_release_buffers, when(conn_state, is_equal_to(mock_mq_conn))); \
        expect(amqp_consume_message,  will_return(&mock_reply_ok), \
            will_set_contents_of_parameter(evp_msg_body, &expect_evps[chosen_evp_idx].msg, sizeof(amqp_bytes_t)), \
            will_set_contents_of_parameter(evp_corr_id,  &expect_evps[chosen_evp_idx].corr_id, sizeof(amqp_bytes_t)), \
        ); \
        expect(utest_rpc_fetch_from_replyq_cb); \
        expect(amqp_destroy_envelope); \
        chosen_evp_idx++; \
    } \
    if(timeout_detection) { \
        expect(amqp_maybe_release_buffers, when(conn_state, is_equal_to(mock_mq_conn))); \
        expect(amqp_consume_message,  will_return(&mock_reply_timeout)); \
    } \
    expect(amqp_basic_cancel, when(tag, is_equal_to_string(expect_q_name))); \
}
    RUN_CODE(0, NUM_EVPS_REPLYQ1, 1)
    RUN_CODE(1, NUM_EVPS_REPLYQ2, 1)
    ARPC_STATUS_CODE  result =  app_rpc_fetch_replies(&mock_rpc_arg, UINT32_MAX, utest_rpc_fetch_from_replyq_cb);
    assert_that(result, is_equal_to(APPRPC_RESP_OK));
    assert_that(mock_rpc_arg.usr_data, is_equal_to(&expect_evps[TOTAL_NUM_MSGS]));
} // end of rpc_replyq_test__get_all_msgs
#undef  NUM_EVPS_REPLYQ1
#undef  NUM_EVPS_REPLYQ2
#undef  TOTAL_NUM_MSGS


#define  NUM_EVPS_REPLYQ1__PART1  3
#define  NUM_EVPS_REPLYQ1__PART2  2
#define  NUM_EVPS_REPLYQ2__PART1  2
#define  NUM_EVPS_REPLYQ2__PART2  5
#define  TOTAL_NUM_MSGS   (NUM_EVPS_REPLYQ1__PART1 + NUM_EVPS_REPLYQ1__PART2 + NUM_EVPS_REPLYQ2__PART1 + NUM_EVPS_REPLYQ2__PART2)
Ensure(rpc_replyq_test__limited_num_msgs) 
{
    UTEST_FETCH_REPLYQ__SETUP
    amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_rpc_reply_t  mock_reply_timeout = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
                .library_error=AMQP_STATUS_TIMEOUT};
    int  chosen_evp_idx = 0;
    ut_consume_evp_t  expect_evps[TOTAL_NUM_MSGS + 1] = {
        UT_EVP_ITEM_INIT("Rust", "msg001"),      UT_EVP_ITEM_INIT("Gopher", "msg002"),
        UT_EVP_ITEM_INIT("Shellfish", "msg003"), UT_EVP_ITEM_INIT("GraphQL", "msg004"),
        UT_EVP_ITEM_INIT("Postgre", "msg005"),   UT_EVP_ITEM_INIT("Nginx", "msg006"),
        UT_EVP_ITEM_INIT("AWS3", "msg007"),    UT_EVP_ITEM_INIT("gRPC", "msg008"),
        UT_EVP_ITEM_INIT("Redis", "msg009"),   UT_EVP_ITEM_INIT("Kafka", "msg010"),
        UT_EVP_ITEM_INIT("Docker","msg011"),   UT_EVP_ITEM_INIT("Scala", "msg012"),
        UT_EVP_ITEM_INIT("", ""),
    };
    mock_rpc_arg.usr_data = &expect_evps[0];
    ARPC_STATUS_CODE  result; size_t num_msg_checked = 0;
    {
        RUN_CODE(0, NUM_EVPS_REPLYQ1__PART1, 0)
        result =  app_rpc_fetch_replies(&mock_rpc_arg, NUM_EVPS_REPLYQ1__PART1, utest_rpc_fetch_from_replyq_cb);
        num_msg_checked +=  NUM_EVPS_REPLYQ1__PART1;
        assert_that(result, is_equal_to(APPRPC_RESP_OK));
        assert_that(mock_rpc_arg.usr_data, is_equal_to(&expect_evps[num_msg_checked]));
    } {
        RUN_CODE(0, NUM_EVPS_REPLYQ1__PART2, 1)
        RUN_CODE(1, NUM_EVPS_REPLYQ2__PART1, 0)
        uint8_t num_processed = NUM_EVPS_REPLYQ1__PART2 + NUM_EVPS_REPLYQ2__PART1;
        result =  app_rpc_fetch_replies(&mock_rpc_arg, num_processed, utest_rpc_fetch_from_replyq_cb);
        num_msg_checked += num_processed;
        assert_that(result, is_equal_to(APPRPC_RESP_OK));
        assert_that(mock_rpc_arg.usr_data, is_equal_to(&expect_evps[num_msg_checked]));
    } {
        RUN_CODE(0, 0, 1)
        RUN_CODE(1, NUM_EVPS_REPLYQ2__PART2, 0)
        result =  app_rpc_fetch_replies(&mock_rpc_arg, NUM_EVPS_REPLYQ2__PART2, utest_rpc_fetch_from_replyq_cb);
        num_msg_checked += NUM_EVPS_REPLYQ2__PART2;
        assert_that(result, is_equal_to(APPRPC_RESP_OK));
        assert_that(mock_rpc_arg.usr_data, is_equal_to(&expect_evps[num_msg_checked]));
    }
} // end of rpc_replyq_test__limited_num_msgs
#undef  TOTAL_NUM_MSGS
#undef  NUM_EVPS_REPLYQ1__PART1
#undef  NUM_EVPS_REPLYQ1__PART2
#undef  NUM_EVPS_REPLYQ2__PART1
#undef  NUM_EVPS_REPLYQ2__PART2


Ensure(rpc_replyq_test__empty_queue) {
    UTEST_FETCH_REPLYQ__SETUP;
    amqp_rpc_reply_t  mock_reply_ok  = {.reply_type=AMQP_RESPONSE_NORMAL};
    amqp_rpc_reply_t  mock_reply_timeout = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
                .library_error=AMQP_STATUS_TIMEOUT};
    int  chosen_evp_idx = 0;
    ut_consume_evp_t  expect_evps[1] = {UT_EVP_ITEM_INIT("", ""),};
    mock_rpc_arg.usr_data = &expect_evps[0];
    RUN_CODE(0, 0, 1)
    RUN_CODE(1, 0, 1)
    ARPC_STATUS_CODE  result = app_rpc_fetch_replies (&mock_rpc_arg, UINT32_MAX, utest_rpc_fetch_from_replyq_cb);
    assert_that(result, is_equal_to(APPRPC_RESP_OK));
    assert_that(mock_rpc_arg.usr_data, is_equal_to(&expect_evps[0]));
} // end of rpc_replyq_test__empty_queue
#undef  RUN_CODE


Ensure(rpc_replyq_test__connection_error) {
    UTEST_FETCH_REPLYQ__SETUP;
    amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_LIBRARY_EXCEPTION,
           .library_error=AMQP_STATUS_SOCKET_ERROR };
    {
        const char *expect_q_name = mock_bind_cfgs[0].reply.queue.name_pattern;
        expect(amqp_basic_consume, when(conn_state, is_equal_to(mock_mq_conn)),
            when(q_name, is_equal_to_string(expect_q_name)) );
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_destroy_connection, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_new_connection, will_return(mock_mq_conn));
        expect(amqp_tcp_socket_new, will_return(mock_mq_sock));
        expect(amqp_socket_open, will_return(AMQP_STATUS_TCP_ERROR));
        expect(amqp_connection_close, when(conn_state, is_equal_to(mock_mq_conn)));
        expect(amqp_destroy_connection, when(conn_state, is_equal_to(mock_mq_conn)));
    }
    ARPC_STATUS_CODE  result =  app_rpc_fetch_replies (&mock_rpc_arg, UINT32_MAX, utest_rpc_fetch_from_replyq_cb);
    assert_that(result, is_equal_to(APPRPC_RESP_MSGQ_CONNECTION_ERROR));
} // end of rpc_replyq_test__connection_error


Ensure(rpc_replyq_test__consume_error) {
    UTEST_FETCH_REPLYQ__SETUP;
    amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
           .reply={.id=AMQP_BASIC_CONSUME_METHOD}};
    {
        const char *expect_q_name = mock_bind_cfgs[0].reply.queue.name_pattern;
        expect(amqp_basic_consume, when(conn_state, is_equal_to(mock_mq_conn)),
            when(q_name, is_equal_to_string(expect_q_name)) );
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
    }
    ARPC_STATUS_CODE  result =  app_rpc_fetch_replies (&mock_rpc_arg, UINT32_MAX, utest_rpc_fetch_from_replyq_cb);
    assert_that(result, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
} // end of rpc_replyq_test__consume_error


Ensure(rpc_replyq_test__nonexist_error) {
    UTEST_FETCH_REPLYQ__SETUP;
    uint16_t expect_channel_id = 12;
    uint16_t next_channel_id = expect_channel_id + 1;
    mock_ctx.curr_channel_id = expect_channel_id;
    amqp_channel_close_t  err_detail = {.reply_code=AMQP_NOT_FOUND};
    amqp_rpc_reply_t  mock_reply_err = {.reply_type=AMQP_RESPONSE_SERVER_EXCEPTION,
           .reply={.id=AMQP_CHANNEL_CLOSE_METHOD, .decoded=&err_detail}};
    amqp_channel_open_ok_t   mock_chn_result = {.channel_id = {.len=next_channel_id, .bytes=(void*)"RTYU"}};
    { // assume channel-level exception happened and the broker automatically closed the channel
        const char *expect_q_name = mock_bind_cfgs[0].reply.queue.name_pattern;
        expect(amqp_basic_consume, when(conn_state, is_equal_to(mock_mq_conn)),
            when(q_name, is_equal_to_string(expect_q_name)) );
        expect(amqp_get_rpc_reply, will_return(&mock_reply_err));
        expect(amqp_channel_close, when(conn_state, is_equal_to(mock_mq_conn)),
            when(channel, is_equal_to(expect_channel_id)) );
        expect(amqp_channel_open, will_return(&mock_chn_result), when(conn_state, is_equal_to(mock_mq_conn)),
            when(channel, is_equal_to(next_channel_id)) );
    }
    ARPC_STATUS_CODE  result =  app_rpc_fetch_replies (&mock_rpc_arg, UINT32_MAX, utest_rpc_fetch_from_replyq_cb);
    assert_that(result, is_equal_to(APPRPC_RESP_MSGQ_OPERATION_ERROR));
    assert_that(mock_ctx.curr_channel_id, is_equal_to(next_channel_id));
} // end of  rpc_replyq_test__nonexist_error


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
    add_test(suite, rpc_start_test__publish_msg_broker_down);
    add_test(suite, rpc_ctx_lookup_test);
    add_test(suite, rpc_consume_test__cfg_error);
    add_test(suite, rpc_consume_test__empty_queue);
    add_test(suite, rpc_consume_test__unknown_route);
    add_test(suite, rpc_consume_test__missing_handler);
    add_test(suite, rpc_consume_test__handler_done__broker_down);
    add_test(suite, rpc_consume_test__handler_finalize_reply_error);
    add_test(suite, rpc_consume_test__handler_finalize_reply_ok);
    add_test(suite, rpc_consume_test__handler_middle_reply_error);
    add_test(suite, rpc_consume_test__handler_middle_reply_ok);
    add_test(suite, rpc_replyq_test__get_all_msgs);
    add_test(suite, rpc_replyq_test__limited_num_msgs);
    add_test(suite, rpc_replyq_test__empty_queue);
    add_test(suite, rpc_replyq_test__connection_error);
    add_test(suite, rpc_replyq_test__consume_error);
    add_test(suite, rpc_replyq_test__nonexist_error);
    return suite;
}
