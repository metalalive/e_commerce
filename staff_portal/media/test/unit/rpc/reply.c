#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <uv.h>
#include "rpc/reply.h"

#define  UTEST_USER_ID     135
#define  UTEST_TIMEOUT_MS  20
#define  UTEST_NUM_RPC_BINDINGS  3
#define  UTEST_Q_NAME_PREFIX    "utest.myapp.user."
#define  UTEST_Q_NAME_PATTERN   UTEST_Q_NAME_PREFIX "%s"
#define  UTEST_FUNC1_CORR_ID_PREFIX    "utest.myapp.funct123.corr."
#define  UTEST_FUNC1_CORR_ID_PATTERN   UTEST_FUNC1_CORR_ID_PREFIX "%s.end"
#define  UTEST_FUNC2_CORR_ID_PREFIX    "utest.myapp.functABC.corr."
#define  UTEST_FUNC2_CORR_ID_PATTERN   UTEST_FUNC2_CORR_ID_PREFIX "%s"
#define  UTEST_FUNC3_CORR_ID_PREFIX    "utest.myapp.functOne.corr."
#define  UTEST_FUNC3_CORR_ID_PATTERN   UTEST_FUNC3_CORR_ID_PREFIX "%s"

typedef struct {
    struct {
        size_t len;
        const char *data;
    } corr_id;
    struct {
        size_t len;
        const char *data;
    } msg;
    uint64_t _ts;
} ut_recv_msg_t;

#define  UT_MSG_ITEM_INIT(_corr_id_str, _msg_str, _timestamp)   { \
    .corr_id = {.data=_corr_id_str, .len=strlen(_corr_id_str)}, \
    .msg = {.data=_msg_str,  .len=strlen(_msg_str)}, \
    ._ts =_timestamp \
}


static  void  utest_rpc_replytimer_err_cb (arpc_reply_cfg_t *cfg, ARPC_STATUS_CODE result)
{
    mock(cfg, result);
    uint8_t *work_done_flag = cfg->usr_data;
    *work_done_flag = 1;
}


static  uint8_t  utest_rpc_replytimer_updated_cb (arpc_reply_cfg_t *cfg, json_t *actual_info, ARPC_STATUS_CODE result)
{
    json_t *expect_info = NULL, **expect_info_p = &expect_info;
    uint8_t _continue = mock(result, expect_info_p);
    if(expect_info) {
        json_t  *expect_recv = NULL, *actual_recv = NULL, *packed = NULL;
        const char *corr_id_pattern = NULL;
        uint8_t idx = 0;
        json_object_foreach(actual_info, corr_id_pattern, actual_recv) {
            if(!json_is_array(actual_recv)) // might be internal info e.g. usr_id
                continue;
            expect_recv = json_object_get(expect_info, corr_id_pattern);
            if(expect_recv)
                assert_that(json_array_size(actual_recv), is_equal_to(json_array_size(expect_recv)));
            json_array_foreach(actual_recv, idx, packed) {
                json_t *corr_id_item = json_object_get(packed, "corr_id");
                json_t *msg_item = json_object_get(packed, "msg");
                uint64_t  ts_done = json_integer_value(json_object_get(packed, "timestamp"));
                const char * corr_id = json_string_value(json_object_get(corr_id_item, "data"));
                size_t  corr_id_sz   = json_integer_value(json_object_get(corr_id_item, "size"));
                const char *msg = json_string_value(json_object_get(msg_item, "data"));
                size_t  msg_sz  = json_integer_value(json_object_get(msg_item, "size"));
                ut_recv_msg_t  *expect_data = (ut_recv_msg_t *) json_integer_value(json_array_get(expect_recv, idx));
                assert_that(expect_data, is_not_null);
                if(!expect_data)
                    break;
                assert_that(corr_id,    is_equal_to_string(expect_data->corr_id.data));
                assert_that(corr_id_sz, is_equal_to(expect_data->corr_id.len));
                assert_that(msg,    is_equal_to_string(expect_data->msg.data));
                assert_that(msg_sz, is_equal_to(expect_data->msg.len));
                assert_that(ts_done, is_equal_to(expect_data->_ts));
            } // end of loop
        } // end of loop
    } // end of expect_info exists
    if(_continue) {
        void *ctx = apprpc_recv_reply_restart ((void *)cfg);
        assert_that(ctx, is_equal_to(cfg));
        _continue = ctx == cfg;
    }
    uint8_t *work_done_flag = cfg->usr_data;
    *work_done_flag = _continue == 0;
    return _continue;
} // end of utest_rpc_replytimer_updated_cb


static  ARPC_STATUS_CODE  utest_rpc_replytimer_lowlvl_fn (arpc_exe_arg_t *mock_exearg,
        size_t max_nread,  arpc_reply_corr_identify_fn  identify_fn)
{
    arpc_cfg_t  rpc_cfg = {0}, *_rpc_cfg_p = &rpc_cfg;
    uint8_t  idx = 0, num_msg_recv = 0, *num_msg_recv_p = &num_msg_recv;
    ARPC_STATUS_CODE result = mock(_rpc_cfg_p, num_msg_recv_p);
    for(idx = 0; idx < num_msg_recv; idx++) {
        char  **routekey_p = &mock_exearg->routing_key;
        char  **msg_p     = &mock_exearg->msg_body.bytes;
        size_t *msg_sz_p  = &mock_exearg->msg_body.len;
        char  **jobid_p     = &mock_exearg->job_id.bytes;
        size_t *jobid_sz_p  = &mock_exearg->job_id.len;
        uint64_t *_ts_p  = &mock_exearg->_timestamp;
        mock(routekey_p, msg_p, msg_sz_p, jobid_p, jobid_sz_p, _ts_p);
        identify_fn(_rpc_cfg_p, mock_exearg);
    } // end of loop
    return result;
} // end of  utest_rpc_replytimer_lowlvl_fn


#define  UTEST_RPC_REPLYTIMER__SETUP \
    int fake_mq_conn = 0, work_done = 0; \
    uv_loop_t *loop = uv_default_loop(); \
    arpc_cfg_bind_t  mock_rpc_bindings[UTEST_NUM_RPC_BINDINGS] = { \
        {.reply={.queue={.name_pattern=UTEST_Q_NAME_PATTERN}, .correlation_id={.name_pattern=UTEST_FUNC1_CORR_ID_PATTERN}}}, \
        {.reply={.queue={.name_pattern=UTEST_Q_NAME_PATTERN}, .correlation_id={.name_pattern=UTEST_FUNC2_CORR_ID_PATTERN}}}, \
        {.reply={.queue={.name_pattern=UTEST_Q_NAME_PATTERN}, .correlation_id={.name_pattern=UTEST_FUNC3_CORR_ID_PATTERN}}}, \
    }; \
    arpc_cfg_t  mock_rpc_cfg = {.bindings = {.size=UTEST_NUM_RPC_BINDINGS, .entries=&mock_rpc_bindings[0]}}; \
    arpc_reply_cfg_t  mock_cfg = {.loop=loop, .conn=(void *)&fake_mq_conn, \
        .usr_id=UTEST_USER_ID,  .timeout_ms=UTEST_TIMEOUT_MS, .usr_data=&work_done, \
        .on_error=utest_rpc_replytimer_err_cb, .on_update=utest_rpc_replytimer_updated_cb, \
        .get_reply_fn=utest_rpc_replytimer_lowlvl_fn \
    };

#define  UTEST_RPC_REPLYTIMER__TEARDOWN \
    uv_run(loop, UV_RUN_ONCE);


#define  MSG_SET1_ITEM1      UT_MSG_ITEM_INIT(UTEST_FUNC2_CORR_ID_PREFIX "uGzF89e", "Blacktea", 1928301)
#define  MSG_SET1_ITEM2      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "Hazard5", "MieGorang", 6410063)
#define  MSG_SET1_ITEM3      UT_MSG_ITEM_INIT(UTEST_FUNC3_CORR_ID_PREFIX "oIY85gw", "phak moo", 4098272)
#define  MSG_SET1_ITEM4      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "tenguol", "wonderin", 9850273)
#define  MSG_SET1_ITEM5      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "tangoEr", "mushroom", 8502712)
#define  MSG_SET2_ITEM1      UT_MSG_ITEM_INIT(UTEST_FUNC3_CORR_ID_PREFIX "igujr8R", "Blacktea", 1782301)
#define  MSG_SET2_ITEM2      UT_MSG_ITEM_INIT(UTEST_FUNC2_CORR_ID_PREFIX "zaRd50P", "antisocial", 1006305)
#define  MSG_SET3_ITEM1      UT_MSG_ITEM_INIT(UTEST_FUNC2_CORR_ID_PREFIX "028urkU", "Blacktea", 80714972)
#define  MSG_SET3_ITEM2      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "rEw823J", "ieGorang", 50318337)
#define  MSG_SET3_ITEM3      UT_MSG_ITEM_INIT(UTEST_FUNC3_CORR_ID_PREFIX "e09rMlK", "phak moo", 93084721)
#define  MSG_SET3_ITEM4      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "eui38gC", "wonderin", 11095187)
#define  MSG_SET3_ITEM5      UT_MSG_ITEM_INIT(UTEST_FUNC2_CORR_ID_PREFIX "rDhngwI", "wonderin", 28047373)
#define  MSG_SET3_ITEM6      UT_MSG_ITEM_INIT(UTEST_FUNC1_CORR_ID_PREFIX "bullet",  "fastapi", 65010807)

#define  NUM_SETS    3
#define  NUM_MSG_RECV_SET1   5
#define  NUM_MSG_RECV_SET2   2
#define  NUM_MSG_RECV_SET3   6
#define  TOT_NUM_MSG_RECV    (NUM_MSG_RECV_SET1 + NUM_MSG_RECV_SET2 + NUM_MSG_RECV_SET3)
Ensure(rpc_replytimer__msg_batch)
{
    UTEST_RPC_REPLYTIMER__SETUP
    uint8_t  idx = 0, jdx = 0, proc_msg_idx = 0, mock_continue_flag = 0;
    uint8_t  expect_num_msg_recv[NUM_SETS] = {NUM_MSG_RECV_SET1, NUM_MSG_RECV_SET2, NUM_MSG_RECV_SET3};
    ut_recv_msg_t  expect_msg_sequence[TOT_NUM_MSG_RECV] = {
        MSG_SET1_ITEM1, MSG_SET1_ITEM2, MSG_SET1_ITEM3, MSG_SET1_ITEM4, MSG_SET1_ITEM5,
        MSG_SET2_ITEM1, MSG_SET2_ITEM2,
        MSG_SET3_ITEM1, MSG_SET3_ITEM2, MSG_SET3_ITEM3, MSG_SET3_ITEM4, MSG_SET3_ITEM5, MSG_SET3_ITEM6
    };
    json_t *expect_classified_msgs[NUM_SETS] = {0};
#define  BUILD_CLASSIFIED_MSGSET(set_idx, bind_idx, num_msg, ...) { \
    if(!expect_classified_msgs[set_idx]) \
        expect_classified_msgs[set_idx] = json_object(); \
    json_t *item1 = expect_classified_msgs[set_idx]; \
    const char *corr_id_patt = mock_rpc_bindings[bind_idx].reply.correlation_id.name_pattern; \
    if(!json_object_get(item1, corr_id_patt)) \
        json_object_set_new(item1, corr_id_patt, json_array()); \
    json_t *item2 = json_object_get(item1, corr_id_patt); \
    ut_recv_msg_t  *_msg_list_p[num_msg] = {__VA_ARGS__}; \
    for(jdx = 0; jdx < num_msg; jdx++) \
        json_array_append_new(item2, json_integer((uint64_t)_msg_list_p[jdx])); \
}
    BUILD_CLASSIFIED_MSGSET(0, 0, 3, &expect_msg_sequence[1], &expect_msg_sequence[3], &expect_msg_sequence[4])
    BUILD_CLASSIFIED_MSGSET(0, 1, 1, &expect_msg_sequence[0])
    BUILD_CLASSIFIED_MSGSET(0, 2, 1, &expect_msg_sequence[2])
    BUILD_CLASSIFIED_MSGSET(1, 1, 1, &expect_msg_sequence[6])
    BUILD_CLASSIFIED_MSGSET(1, 2, 1, &expect_msg_sequence[5])
    BUILD_CLASSIFIED_MSGSET(2, 0, 3, &expect_msg_sequence[8], &expect_msg_sequence[10], &expect_msg_sequence[12])
    BUILD_CLASSIFIED_MSGSET(2, 1, 2, &expect_msg_sequence[7], &expect_msg_sequence[11])
    BUILD_CLASSIFIED_MSGSET(2, 2, 1, &expect_msg_sequence[9])
    const char *mock_replyq_name = UTEST_Q_NAME_PREFIX "1928";
    void *ctx = apprpc_recv_reply_start(&mock_cfg);
    assert_that(ctx, is_not_equal_to(NULL));
    for(idx = 0; idx < NUM_SETS; idx++) {
        expect( utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OK),
            will_set_contents_of_parameter(_rpc_cfg_p, &mock_rpc_cfg, sizeof(arpc_cfg_t)),
            will_set_contents_of_parameter(num_msg_recv_p, &expect_num_msg_recv[idx], sizeof(uint8_t))
        );
        for(jdx = 0; jdx < expect_num_msg_recv[idx]; jdx++) {
            ut_recv_msg_t *exp_msg = &expect_msg_sequence[proc_msg_idx++];
            expect( utest_rpc_replytimer_lowlvl_fn,
                will_set_contents_of_parameter(routekey_p, &mock_replyq_name, sizeof(char *)),
                will_set_contents_of_parameter(msg_p,    &exp_msg->msg.data, sizeof(char *)),
                will_set_contents_of_parameter(msg_sz_p, &exp_msg->msg.len,  sizeof(size_t)),
                will_set_contents_of_parameter(jobid_p,  &exp_msg->corr_id.data,  sizeof(char *)),
                will_set_contents_of_parameter(jobid_sz_p, &exp_msg->corr_id.len, sizeof(size_t)),
                will_set_contents_of_parameter(_ts_p, &exp_msg->_ts, sizeof(uint64_t)),
            );
        } // end of loop
        mock_continue_flag = (idx + 1) < NUM_SETS;
        expect(utest_rpc_replytimer_updated_cb, will_return(mock_continue_flag),
                will_set_contents_of_parameter(expect_info_p, &expect_classified_msgs[idx], sizeof(json_t *)),
                when(result, is_equal_to(APPRPC_RESP_OK))  );
    } // end of loop
    while(!work_done)
        uv_run(loop, UV_RUN_ONCE);
    for(idx = 0; idx < NUM_SETS; idx++)
        json_decref(expect_classified_msgs[idx]);
    UTEST_RPC_REPLYTIMER__TEARDOWN
#undef   BUILD_CLASSIFIED_MSGSET
} // end of  rpc_replytimer__msg_batch
#undef   TOT_NUM_MSG_RECV 
#undef   NUM_MSG_RECV_SET1
#undef   NUM_MSG_RECV_SET2
#undef   NUM_MSG_RECV_SET3
#undef   NUM_SETS


#define  NUM_SETS    3
Ensure(rpc_replytimer__start_empty) 
{
    UTEST_RPC_REPLYTIMER__SETUP
    uint8_t  idx = 0, mock_continue_flag = 0;
    void *ctx = apprpc_recv_reply_start(&mock_cfg);
    assert_that(ctx, is_not_equal_to(NULL));
    uint8_t expect_num_msg_recv = 0;
    for(idx = 0; idx < NUM_SETS; idx++) {
        expect( utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OK),
            will_set_contents_of_parameter(_rpc_cfg_p, &mock_rpc_cfg, sizeof(arpc_cfg_t)),
            will_set_contents_of_parameter(num_msg_recv_p, &expect_num_msg_recv, sizeof(uint8_t))
        );
        mock_continue_flag = (idx + 1) < NUM_SETS;
        expect(utest_rpc_replytimer_updated_cb, will_return(mock_continue_flag),
                when(result, is_equal_to(APPRPC_RESP_OK))  );
    } // end of loop
    while(!work_done)
        uv_run(loop, UV_RUN_ONCE);
    UTEST_RPC_REPLYTIMER__TEARDOWN
} // end of rpc_replytimer__start_empty
#undef   NUM_SETS


Ensure(rpc_replytimer__missing_corr_id)
{
    UTEST_RPC_REPLYTIMER__SETUP
    uint8_t expect_num_msg_recv = 1;
    void *ctx = apprpc_recv_reply_start(&mock_cfg);
    assert_that(ctx, is_not_equal_to(NULL));
    { // the received message will be discarded due to lack of correlation ID
        expect(utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OK),
            will_set_contents_of_parameter(_rpc_cfg_p, &mock_rpc_cfg, sizeof(arpc_cfg_t)),
            will_set_contents_of_parameter(num_msg_recv_p, &expect_num_msg_recv, sizeof(uint8_t))
        );
        expect(utest_rpc_replytimer_lowlvl_fn);
        expect(utest_rpc_replytimer_updated_cb, will_return(0),  when(result, is_equal_to(APPRPC_RESP_OK)));
    }
    while(!work_done)
        uv_run(loop, UV_RUN_ONCE);
    UTEST_RPC_REPLYTIMER__TEARDOWN
} // end of rpc_replytimer__missing_corr_id


Ensure(rpc_replytimer__recv_junk_msg)
{
    UTEST_RPC_REPLYTIMER__SETUP
    const char *mock_replyq_name = UTEST_Q_NAME_PREFIX "65535";
    uint8_t expect_num_msg_recv = 1;
    ut_recv_msg_t  exp_msg = UT_MSG_ITEM_INIT("myapp.unknown.correlation_id.pattern", "message1240394", 270219);
    void *ctx = apprpc_recv_reply_start(&mock_cfg);
    assert_that(ctx, is_not_equal_to(NULL));
    { // the received message will be discarded due to lack of correlation ID
        expect( utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OK),
            will_set_contents_of_parameter(_rpc_cfg_p, &mock_rpc_cfg, sizeof(arpc_cfg_t)),
            will_set_contents_of_parameter(num_msg_recv_p, &expect_num_msg_recv, sizeof(uint8_t))
        );
        expect( utest_rpc_replytimer_lowlvl_fn,
            will_set_contents_of_parameter(routekey_p, &mock_replyq_name, sizeof(char *)),
            will_set_contents_of_parameter(msg_p,    &exp_msg.msg.data, sizeof(char *)),
            will_set_contents_of_parameter(msg_sz_p, &exp_msg.msg.len,  sizeof(size_t)),
            will_set_contents_of_parameter(jobid_p,  &exp_msg.corr_id.data,  sizeof(char *)),
            will_set_contents_of_parameter(jobid_sz_p, &exp_msg.corr_id.len, sizeof(size_t)),
            will_set_contents_of_parameter(_ts_p, &exp_msg._ts, sizeof(uint64_t)),
        );
        expect(utest_rpc_replytimer_updated_cb, will_return(0),  when(result, is_equal_to(APPRPC_RESP_OK)));
    }
    while(!work_done)
        uv_run(loop, UV_RUN_ONCE);
} // end of rpc_replytimer__recv_junk_msg


Ensure(rpc_replytimer__lowlvl_unknown_error)
{
    UTEST_RPC_REPLYTIMER__SETUP
    uint8_t expect_num_msg_recv = 0;
    void *ctx = apprpc_recv_reply_start(&mock_cfg);
    assert_that(ctx, is_not_equal_to(NULL));
    {
        expect(utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OK),
            will_set_contents_of_parameter(_rpc_cfg_p, &mock_rpc_cfg, sizeof(arpc_cfg_t)),
            will_set_contents_of_parameter(num_msg_recv_p, &expect_num_msg_recv, sizeof(uint8_t))
        );
        expect(utest_rpc_replytimer_updated_cb, will_return(1),  when(result, is_equal_to(APPRPC_RESP_OK)));
        expect(utest_rpc_replytimer_lowlvl_fn, will_return(APPRPC_RESP_OS_ERROR));
        expect(utest_rpc_replytimer_err_cb, when(result, is_equal_to(APPRPC_RESP_OS_ERROR)));
    }
    while(!work_done)
        uv_run(loop, UV_RUN_ONCE);
    UTEST_RPC_REPLYTIMER__TEARDOWN
} // end of rpc_replytimer__lowlvl_unknown_error


Ensure(rpc_pycelery_extract_reply__start_ok)
{
#define  DISCARDED_REPLY    "{\"app123\":\"put down the great firewall\"}"
#define  PYCELERY_RAW_MSG  "{\"status\":\"STARTED\",\"result\":" DISCARDED_REPLY "}"
#define  UTEST_MSG_PATTERN   "[{\"msg\":{\"data\":null,\"size\":0}}]"
    json_t *mock_msgs_in = json_loadb(UTEST_MSG_PATTERN, sizeof(UTEST_MSG_PATTERN) - 1, 0, NULL);
    {
        json_t *item = json_object_get(json_array_get(mock_msgs_in, 0),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG) - 1));
    }
    json_t *valid_reply = NULL;
    ARPC_STATUS_CODE result = app_rpc__pycelery_extract_replies(mock_msgs_in, &valid_reply);
    assert_that(result, is_equal_to(APPRPC_RESP_OK));
    assert_that(valid_reply, is_equal_to(NULL));
    json_decref(mock_msgs_in);
#undef   UTEST_MSG_PATTERN
#undef   PYCELERY_RAW_MSG
#undef   DISCARDED_REPLY
} // end of rpc_pycelery_extract_reply__start_ok


Ensure(rpc_pycelery_extract_reply__return_ok) 
{
#define  EXPECT_APP_KEY     "var456"
#define  EXPECT_APP_VALUE   "redis oauth quic"
#define  DISCARDED_REPLY    "{\"var123\":\"pipeapple\"}"
#define  EXTRACTED_REPLY    "{\""EXPECT_APP_KEY"\":\""EXPECT_APP_VALUE"\"}"
#define  PYCELERY_RAW_MSG_1  "{\"status\":\"STARTED\",\"result\":" DISCARDED_REPLY "}"
#define  PYCELERY_RAW_MSG_2  "{\"status\":\"SUCCESS\",\"result\":" EXTRACTED_REPLY "}"
#define  UTEST_MSG_PATTERN   "[{\"msg\":{\"data\":null,\"size\":0}}, {\"msg\":{\"data\":null,\"size\":0}}]"
    json_t *mock_msgs_in = json_loadb(UTEST_MSG_PATTERN, sizeof(UTEST_MSG_PATTERN) - 1, 0, NULL);
    {
        json_t *item = json_object_get(json_array_get(mock_msgs_in, 0),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG_1));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG_1) - 1));
        item = json_object_get(json_array_get(mock_msgs_in, 1),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG_2));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG_2) - 1));
    }
    json_t *valid_reply = NULL;
    ARPC_STATUS_CODE result = app_rpc__pycelery_extract_replies(mock_msgs_in, &valid_reply);
    assert_that(result, is_equal_to(APPRPC_RESP_OK));
    assert_that(valid_reply, is_not_equal_to(NULL));
    if(valid_reply) {
        const char *actual = json_string_value(json_object_get(valid_reply,EXPECT_APP_KEY));
        assert_that(actual, is_equal_to_string(EXPECT_APP_VALUE));
        json_decref(valid_reply);
    }
    json_decref(mock_msgs_in);
#undef   UTEST_MSG_PATTERN
#undef   PYCELERY_RAW_MSG_2
#undef   PYCELERY_RAW_MSG_1
#undef   EXTRACTED_REPLY
#undef   DISCARDED_REPLY
#undef   EXPECT_APP_VALUE
#undef   EXPECT_APP_KEY
} // end of rpc_pycelery_extract_reply__return_ok


Ensure(rpc_pycelery_extract_reply__invalid)
{
#define  PYCELERY_RAW_MSG    "{\"random\":\"can not be recognized\"}"
#define  UTEST_MSG_PATTERN   "[{\"msg\":{\"data\":null,\"size\":0}}, {\"msg\":{\"data\":null,\"size\":0}}]"
    json_t *mock_msgs_in = json_loadb(UTEST_MSG_PATTERN, sizeof(UTEST_MSG_PATTERN) - 1, 0, NULL);
    {
        json_t *item = json_object_get(json_array_get(mock_msgs_in, 0),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG) - 1));
    }
    json_t *valid_reply = NULL;
    ARPC_STATUS_CODE result = app_rpc__pycelery_extract_replies(mock_msgs_in, &valid_reply);
    assert_that(result, is_equal_to(APPRPC_RESP_ARG_ERROR));
    assert_that(valid_reply, is_equal_to(NULL));
    json_decref(mock_msgs_in);
#undef   UTEST_MSG_PATTERN
#undef   PYCELERY_RAW_MSG
} // end of rpc_pycelery_extract_reply__invalid


Ensure(rpc_pycelery_extract_reply__remote_error)
{
#define  PYCELERY_RAW_MSG_1  "{\"status\":\"STARTED\",\"result\":{\"var123\":\"pipeapple\"}}"
#define  PYCELERY_RAW_MSG_2  "{\"status\":\"ERROR\",\"result\":{\"var456\":\"heyhey\"}}"
#define  UTEST_MSG_PATTERN   "[{\"msg\":{\"data\":null,\"size\":0}}, {\"msg\":{\"data\":null,\"size\":0}}]"
    json_t *mock_msgs_in = json_loadb(UTEST_MSG_PATTERN, sizeof(UTEST_MSG_PATTERN) - 1, 0, NULL);
    {
        json_t *item = json_object_get(json_array_get(mock_msgs_in, 0),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG_1));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG_1) - 1));
        item = json_object_get(json_array_get(mock_msgs_in, 1),"msg");
        json_object_set_new(item, "data", json_string(PYCELERY_RAW_MSG_2));
        json_object_set_new(item, "size", json_integer(sizeof(PYCELERY_RAW_MSG_2) - 1));
    }
    json_t *valid_reply = NULL;
    ARPC_STATUS_CODE result = app_rpc__pycelery_extract_replies(mock_msgs_in, &valid_reply);
    assert_that(result, is_equal_to(APPRPC_RESP_ARG_ERROR));
    assert_that(valid_reply, is_equal_to(NULL));
    json_decref(mock_msgs_in);
#undef   UTEST_MSG_PATTERN
#undef   PYCELERY_RAW_MSG_2
#undef   PYCELERY_RAW_MSG_1
} // end of rpc_pycelery_extract_reply__remote_error


TestSuite *app_rpc_replytimer_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_replytimer__msg_batch);
    add_test(suite, rpc_replytimer__start_empty);
    add_test(suite, rpc_replytimer__missing_corr_id);
    add_test(suite, rpc_replytimer__recv_junk_msg);
    add_test(suite, rpc_replytimer__lowlvl_unknown_error);
    add_test(suite, rpc_pycelery_extract_reply__start_ok);
    add_test(suite, rpc_pycelery_extract_reply__return_ok);
    add_test(suite, rpc_pycelery_extract_reply__invalid);
    add_test(suite, rpc_pycelery_extract_reply__remote_error);
    return suite;
} // end of  app_rpc_replytimer_tests
