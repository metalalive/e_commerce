#include <cgreen/cgreen.h>
#include "rpc/cfg_parser.h"
#include <h2o.h>
/*
 *  Important Note for Jansson library :
 *  The nested json objects can be automatically de-alloacted when de-allocating
 *  the root object by "stealing the reference" of nested object.
 *  
 *  "Stealing" means a Jansson function creating new json object WITHOUT incrementing
 *  the reference count of the new object, which makes de-allocation easier because
 *  all you need is to call json_decref() on root object.
 *
 *  The functions stealing reference come with surfix name `_new` , e.g.
 *  json_object_set_new() , json_array_append_new() ... etc
 *  
 *  Further discussion:
 *      https://groups.google.com/g/jansson-users/c/3nkqNOFD8CU/m/2X3bxMshuxIJ
*/

static void app_utest__rpc_cfg_setup(json_t *objs, size_t num_brokers, const char **data_str, uint32_t *data_uint)
{
    size_t idx = 0;
    size_t jdx = 0;
    size_t dstr_rd_ptr = 0;
    size_t dint_rd_ptr = 0;
    for(idx = 0; idx < num_brokers; idx++) {
        json_t *broker_cfg = json_object();
        json_array_append_new(objs, broker_cfg);
        json_t *credential = json_object();
        json_t *attributes = json_object();
        json_t *bindings   = json_array();
        json_t *alias = json_string(data_str[dstr_rd_ptr++]);
        json_object_set_new(broker_cfg, "alias", alias);
        json_object_set_new(broker_cfg, "credential", credential);
        json_object_set_new(broker_cfg, "attributes", attributes);
        json_object_set_new(broker_cfg, "bindings",   bindings  );
        { // assume the hierarchy path is only 2 levels, first level is string, the second one is integer
            json_t *filepath  = json_string(data_str[dstr_rd_ptr++]);
            json_t *hierarchy = json_array();
            json_array_append_new(hierarchy, json_string(data_str[dstr_rd_ptr++]));
            json_array_append_new(hierarchy, json_integer(data_uint[dint_rd_ptr++]));
            json_object_set_new(credential, "filepath" , filepath );
            json_object_set_new(credential, "hierarchy", hierarchy);
        }
        {
            json_t *vhost = json_string(data_str[dstr_rd_ptr++]);
            json_t *max_channels     = json_integer(data_uint[dint_rd_ptr++]);
            json_t *max_kb_per_frame = json_integer(data_uint[dint_rd_ptr++]);
            json_object_set_new(attributes, "vhost", vhost);
            json_object_set_new(attributes, "max_channels", max_channels);
            json_object_set_new(attributes, "max_kb_per_frame", max_kb_per_frame);
        }
        size_t num_bindings = data_uint[dint_rd_ptr++];
        for(jdx = 0; jdx < num_bindings; jdx++) {
            json_t *bind_obj = json_object();
            json_t *queue    = json_string(data_str[dstr_rd_ptr++]);
            json_t *exchange = json_string(data_str[dstr_rd_ptr++]);
            json_t *routing_key = json_string(data_str[dstr_rd_ptr++]);
            json_t *durable  = json_boolean(data_uint[dint_rd_ptr++]);
            json_object_set_new(bind_obj, "queue", queue);
            json_object_set_new(bind_obj, "exchange", exchange);
            json_object_set_new(bind_obj, "routing_key", routing_key);
            json_object_set_new(bind_obj, "durable", durable);
            {
                json_t *reply_obj = json_object();
                json_t *qname_obj = json_object();
                json_t *corr_id_obj = json_object();
                json_object_set_new(qname_obj, "pattern",   json_string(data_str[dstr_rd_ptr++]));
                json_object_set_new(qname_obj, "render_fn", json_string(data_str[dstr_rd_ptr++]));
                json_object_set_new(corr_id_obj, "pattern",   json_string(data_str[dstr_rd_ptr++]));
                json_object_set_new(corr_id_obj, "render_fn", json_string(data_str[dstr_rd_ptr++]));
                json_object_set_new(reply_obj, "queue", qname_obj);
                json_object_set_new(reply_obj, "correlation_id", corr_id_obj);
                json_object_set_new(reply_obj, "task_handler_fn", json_string(data_str[dstr_rd_ptr++]));
                json_object_set_new(reply_obj, "durable", json_boolean(data_uint[dint_rd_ptr++]));
                json_object_set_new(reply_obj, "ttl_sec", json_integer(data_uint[dint_rd_ptr++]));
                json_object_set_new(bind_obj, "reply", reply_obj);
            }
            json_array_append_new(bindings, bind_obj);
        } // end of binding configuration
    } // end of broker configuration
} // end of app_utest__rpc_cfg_setup


#define DECLARE_RPC_REPLY_RENDER_FN(fn_name) \
static ARPC_STATUS_CODE fn_name \
    (arpc_cfg_bind_reply_t *cfg, arpc_exe_arg_t *arg, char *wr_buf, size_t wr_sz)

DECLARE_RPC_REPLY_RENDER_FN(utset_operation_efg__rpc_corr_id_render)
{ return APPRPC_RESP_ACCEPTED; }

DECLARE_RPC_REPLY_RENDER_FN(utset_operation_cde__rpc_qname_render)
{
    h2o_error_printf("[DEBUG][rpc][cfg-parser] utset_operation_cde__rpc_qname_render hit \n");
    return APPRPC_RESP_ACCEPTED;
}

DECLARE_RPC_REPLY_RENDER_FN(utset_operation_ijk__rpc_qname_render)
{ return APPRPC_RESP_ACCEPTED; }

#define  NUM_MSG_BROKERS 2
Ensure(rpc_caller_cfg_incomplete_setting_tests) {
    const char *data_str[] = {
        // --------- broker 1 ----------
        "utest_mqbroker_1", // alias
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost1234",  // attributes vhost
        "queue_01_for_rpc_operation_abc", // broker[0] bindings[0] queue 
        "exchange_02_for_rpc_operation_abc", // broker[0] bindings[0] exchange
        "rpc.media.operation_abc", // broker[0] bindings[0] routing_key
            "rpc_replyq_operation_abc__usrprof", // broker[0] bindings[0] reply.queue.pattern
            NULL,                    // broker[0] bindings[0] reply.queue.render_fn
            "rpc.media.abc.corr_id", // broker[0] bindings[0] reply.correlation_id.pattern
            NULL,  // broker[0] bindings[0] reply.correlation_id.render_fn
            NULL,  // broker[0] bindings[0] reply.task_handler_fn
        "queue_03_for_rpc_operation_cde", // broker[0] bindings[1] queue 
        "exchange_04_for_rpc_operation_cde", // broker[0] bindings[1] exchange
        "rpc.media.operation_cde", // broker[0] bindings[1] routing_key
            "rpc_replyq_operation_cde__usrgrp_%u",   // broker[0] bindings[1] reply.queue.pattern
            "utset_operation_cde__rpc_qname_render", // broker[0] bindings[1] reply.queue.render_fn
            "rpc.media.cde.corr_id",                 // broker[0] bindings[1] reply.correlation_id.pattern
            NULL, // broker[0] bindings[1] reply.correlation_id.render_fn
            NULL, // broker[0] bindings[1] reply.task_handler_fn
        // --------- broker 2 ----------
        "utest_mqbroker_2", // alias
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost5678",  // attributes vhost
        "queue_07_for_rpc_operation_efg", // broker[1] bindings[0] queue 
        "exchange_08_for_rpc_operation_efg", // broker[1] bindings[0] exchange
        "rpc.media.operation_efg", // broker[1] bindings[0] routing_key
            "rpc_replyq_operation_efg", // broker[1] bindings[0] reply.queue.pattern
            NULL, // broker[1] bindings[0] reply.queue.render_fn
            "rpc.media.efg.corr_id.%lu.%x", // broker[1] bindings[0] reply.correlation_id.pattern
            "utset_operation_efg__rpc_corr_id_render", // broker[1] bindings[0] reply.correlation_id.render_fn
            NULL, // broker[1] bindings[0] reply.task_handler_fn
        "queue_09_for_rpc_operation_ghi", // broker[1] bindings[1] queue 
        "exchange_11_for_rpc_operation_ghi", // broker[1] bindings[1] exchange
        "rpc.media.operation_ghi",  // broker[1] bindings[1] routing_key
            "rpc_replyq_operation_ghi", // broker[1] bindings[1] reply.queue.pattern
            NULL, // broker[1] bindings[1] reply.queue.render_fn
            "rpc.media.ghi.corr_id", // broker[1] bindings[1] reply.correlation_id.pattern
            NULL, // broker[1] bindings[1] reply.correlation_id.render_fn
            NULL, // broker[1] bindings[1] reply.task_handler_fn
        "queue_12_for_rpc_operation_ijk", // broker[1] bindings[2] queue 
        "exchange_13_for_rpc_operation_ijk", // broker[1] bindings[2] exchange
        "rpc.media.operation_ijk", // broker[1] bindings[2] routing_key
            "rpc_replyq_operation_ijk__oid_%u", // broker[1] bindings[2] reply.queue.pattern
            "utset_operation_ijk__rpc_qname_render", // broker[1] bindings[2] reply.queue.render_fn
            "rpc.media.ijk.corr_id.%llu.%s.%04x", // broker[1] bindings[2] reply.correlation_id.pattern
            "utset_operation_ijk__rpc_corr_id_render", // broker[1] bindings[2] reply.correlation_id.render_fn , expect to report error from here
            NULL, // broker[1] bindings[2] reply.task_handler_fn
    }; // end of data_str
    uint32_t data_uint[] = {
        // --------- broker 1 ----------
        0, // credential hierarchy
        370, // attributes max_channels
        63,  // attributes max_kb_per_frame
        2, // number of bindings in current broker configuration
        0,   // broker[0] bindings[0] durable
            1,   // broker[0] bindings[0] reply.durable
            300, // broker[0] bindings[0] reply.ttl_sec
        1,   // broker[0] bindings[1] durable
            0,   // broker[0] bindings[1] reply.durable
            343, // broker[0] bindings[1] reply.ttl_sec
        // --------- broker 2 ----------
        1, // credential hierarchy
        800, // attributes max_channels
        43,  // attributes max_kb_per_frame
        3, // number of bindings in current broker configuration
        0,   // broker[1] bindings[0] durable
            1,   // broker[1] bindings[0] reply.durable
            278, // broker[1] bindings[0] reply.ttl_sec
        1,   // broker[1] bindings[1] durable
            0,   // broker[1] bindings[1] reply.durable
            19,  // broker[1] bindings[1] reply.ttl_sec
        0,   // broker[1] bindings[2] durable
            1,   // broker[1] bindings[2] reply.durable
            120, // broker[1] bindings[2] reply.ttl_sec
    }; // end of data_uint
    const char **data_str_ptr = (const char **)data_str;
    uint32_t *data_uint_ptr = &data_uint[0];
    app_cfg_t  mock_app_cfg = {0};
    json_t *objs = json_array();
    app_utest__rpc_cfg_setup(objs, NUM_MSG_BROKERS, data_str_ptr, data_uint_ptr);
    int err = 0;
    assert_that(mock_app_cfg.rpc.entries, is_null);
    err = parse_cfg_rpc_caller(objs, &mock_app_cfg);
    assert_that(err, is_not_equal_to(0));
    assert_that(mock_app_cfg.rpc.entries, is_null);
    json_decref(objs);
} // end of rpc_caller_cfg_incomplete_setting_tests
#undef  NUM_BINDINGS


static void utest_rpc_dellocate_app_cfg(app_cfg_t *app_cfg)
{
    size_t idx = 0;
    for(idx = 0; idx < app_cfg->rpc.size; idx++) {
        app_rpc_cfg_deinit(&app_cfg->rpc.entries[idx]);
    }
    app_cfg->rpc.size = 0;
} // end of utest_rpc_dellocate_app_cfg


static void utest_app_rpc__verify_broker_1(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->alias, is_equal_to_string(data_str_ptr[0]));
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[3]));
    assert_that(rpc_cfg->attributes.max_channels, is_equal_to(data_uint_ptr[1]));
    assert_that(rpc_cfg->attributes.max_kb_per_frame, is_equal_to(data_uint_ptr[2]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[1];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[12]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[13]));
    assert_that(bind_cfg->routing_key, is_equal_to_string(data_str_ptr[14]));
    assert_that(bind_cfg->reply.queue.name_pattern         , is_equal_to_string(data_str_ptr[15]));
    assert_that(bind_cfg->reply.correlation_id.name_pattern, is_equal_to_string(data_str_ptr[17]));
} // end of utest_app_rpc__verify_broker_1

static void utest_app_rpc__verify_broker_2(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->alias, is_equal_to_string(data_str_ptr[0]));
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[3]));
    assert_that(rpc_cfg->attributes.max_channels, is_equal_to(data_uint_ptr[1]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[0];
    assert_that(bind_cfg->q_name     , is_equal_to_string(data_str_ptr[4]));
    assert_that(bind_cfg->routing_key, is_equal_to_string(data_str_ptr[6]));
    assert_that(bind_cfg->reply.queue.name_pattern         , is_equal_to_string(data_str_ptr[7]));
    assert_that(bind_cfg->reply.correlation_id.name_pattern, is_equal_to_string(data_str_ptr[9]));
    assert_that(bind_cfg->flags.durable, is_equal_to(data_uint_ptr[4]));
    bind_cfg = &rpc_cfg->bindings.entries[1];
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[13]));
    assert_that(bind_cfg->reply.queue.name_pattern         , is_equal_to_string(data_str_ptr[15]));
    assert_that(bind_cfg->reply.correlation_id.name_pattern, is_equal_to_string(data_str_ptr[17]));
    assert_that(bind_cfg->flags.durable, is_equal_to(data_uint_ptr[7]));
    bind_cfg = &rpc_cfg->bindings.entries[2];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[20]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[21]));
    assert_that(bind_cfg->reply.queue.name_pattern         , is_equal_to_string(data_str_ptr[23]));
    assert_that(bind_cfg->reply.correlation_id.name_pattern, is_equal_to_string(data_str_ptr[25]));
} // end of utest_app_rpc__verify_broker_2

static void utest_app_rpc__verify_broker_3(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->alias, is_equal_to_string(data_str_ptr[0]));
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[3]));
    assert_that(rpc_cfg->attributes.max_kb_per_frame, is_equal_to(data_uint_ptr[2]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[0];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[4]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[5]));
    assert_that(bind_cfg->routing_key, is_equal_to_string(data_str_ptr[6]));
    assert_that(bind_cfg->reply.queue.name_pattern         , is_equal_to_string(data_str_ptr[7]));
    assert_that(bind_cfg->reply.correlation_id.name_pattern, is_equal_to_string(data_str_ptr[9]));
} // end of utest_app_rpc__verify_broker_3


Ensure(rpc_caller_cfg_reconfig_tests) {
    const char *data_str[] = {
        // --------- broker 1 ----------
        "utest_mqbroker_1", // alias
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost1234",  // attributes vhost
        "queue_01_for_rpc_operation_abc", // broker[0] bindings[0] queue 
        "exchange_02_for_rpc_operation_abc", // broker[0] bindings[0] exchange
        "rpc.media.operation_abc", // broker[0] bindings[0] routing_key
            "rpc_replyq_operation_abc", // broker[0] bindings[0] reply.queue.pattern
            NULL, // broker[0] bindings[0] reply.queue.render_fn
            "rpc.media.abc.corr_id", // broker[0] bindings[0] reply.correlation_id.pattern
            NULL, // broker[0] bindings[0] reply.correlation_id.render_fn
            NULL, // broker[0] bindings[0] reply.task_handler_fn
        "queue_03_for_rpc_operation_cde", // broker[0] bindings[1] queue 
        "exchange_04_for_rpc_operation_cde", // broker[0] bindings[1] exchange
        "rpc.media.operation_cde", // broker[0] bindings[1] routing_key
            "rpc_replyq_operation_cde_oid_%lu", // broker[0] bindings[1] reply.queue.pattern
            "utset_operation_cde__rpc_qname_render", // broker[0] bindings[1] reply.queue.render_fn
            "rpc.media.cde.corr_id", // broker[0] bindings[1] reply.correlation_id.pattern
            NULL, // broker[0] bindings[1] reply.correlation_id.render_fn
            NULL, // broker[0] bindings[1] reply.task_handler_fn
        // --------- broker 2 ----------
        "utest_mqbroker_2", // alias
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost5678",  // attributes vhost
        "queue_07_for_rpc_operation_efg", // broker[1] bindings[0] queue 
        "exchange_08_for_rpc_operation_efg", // broker[1] bindings[0] exchange
        "rpc.media.operation_efg", // broker[1] bindings[0] routing_key
            "rpc_replyq_operation_efg", // broker[1] bindings[0] reply.queue.pattern
            NULL, // broker[1] bindings[0] reply.queue.render_fn
            "rpc.media.efg.corr_id", // broker[1] bindings[0] reply.correlation_id.pattern
            "utset_operation_efg__rpc_corr_id_render", // broker[1] bindings[0] reply.correlation_id.render_fn
            NULL, // broker[1] bindings[0] reply.task_handler_fn
        "queue_09_for_rpc_operation_ghi", // broker[1] bindings[1] queue 
        "exchange_11_for_rpc_operation_ghi", // broker[1] bindings[1] exchange
        "rpc.media.operation_ghi", // broker[1] bindings[1] routing_key
            "rpc_replyq_operation_ghi", // broker[1] bindings[1] reply.queue.pattern
            NULL, // broker[1] bindings[1] reply.queue.render_fn
            "rpc.media.ghi.corr_id", // broker[1] bindings[1] reply.correlation_id.pattern
            NULL, // broker[1] bindings[1] reply.correlation_id.render_fn
            NULL, // broker[1] bindings[1] reply.task_handler_fn
        "queue_12_for_rpc_operation_ijk", // broker[1] bindings[2] queue 
        "exchange_13_for_rpc_operation_ijk", // broker[1] bindings[2] exchange
        "rpc.media.operation_ijk", // broker[1] bindings[2] routing_key
            "rpc_replyq_operation_ijk", // broker[1] bindings[2] reply.queue.pattern
            NULL, // broker[1] bindings[2] reply.queue.render_fn
            "rpc.media.ijk.corr_id", // broker[1] bindings[2] reply.correlation_id.pattern
            NULL, // broker[1] bindings[2] reply.correlation_id.render_fn
            NULL, // broker[1] bindings[2] reply.task_handler_fn
        // --------- broker 3 ----------
        "utest_mqbroker_3", // alias
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost_practice_tdd",  // attributes vhost
        "queue_12_for_rpc_operation_kmn", // broker[2] bindings[0] queue 
        "exchange_13_for_rpc_operation_kmn", // broker[2] bindings[0] exchange
        "rpc.media.operation_kmn", // broker[2] bindings[0] routing_key
            "rpc_replyq_operation_kmn", // broker[2] bindings[0] reply.queue.pattern
            NULL, // broker[2] bindings[0] reply.queue.render_fn
            "rpc.media.kmn.corr_id", // broker[2] bindings[0] reply.correlation_id.pattern
            NULL, // broker[2] bindings[0] reply.correlation_id.render_fn
            NULL, // broker[2] bindings[0] reply.task_handler_fn
    }; // end of data_str
    uint32_t data_uint[] = {
        // --------- broker 1 ----------
        0, // credential hierarchy
        370, // attributes max_channels
        63,  // attributes max_kb_per_frame
        2, // number of bindings in current broker configuration
        0, // broker[0] bindings[0] durable
            1,   // broker[0] bindings[0] reply.durable
            350, // broker[0] bindings[0] reply.ttl_sec
        1, // broker[0] bindings[1] durable
            0,  // broker[0] bindings[1] reply.durable
            78, // broker[0] bindings[1] reply.ttl_sec
        // --------- broker 2 ----------
        1, // credential hierarchy
        800, // attributes max_channels
        43,  // attributes max_kb_per_frame
        3, // number of bindings in current broker configuration
        0, // broker[1] bindings[0] durable
            1,  // broker[1] bindings[0] reply.durable
            36, // broker[1] bindings[0] reply.ttl_sec
        1, // broker[1] bindings[1] durable
            0,  // broker[1] bindings[1] reply.durable
            18, // broker[1] bindings[1] reply.ttl_sec
        0, // broker[1] bindings[2] durable
            1,  // broker[1] bindings[2] reply.durable
            80, // broker[1] bindings[2] reply.ttl_sec
        // --------- broker 3 ----------
        0, // credential hierarchy
        626, // attributes max_channels
        85,  // attributes max_kb_per_frame
        1, // number of bindings in current broker configuration
        1, // broker[2] bindings[0] durable
            0,  // broker[2] bindings[0] reply.durable
            47, // broker[2] bindings[0] reply.ttl_sec
    }; // end of data_uint
    const char **data_str_ptr  = NULL;
    uint32_t    *data_uint_ptr = NULL;
    app_cfg_t  mock_app_cfg = {0};
    json_t *objs = json_array();
    int err = 0;
    { // parse configuration for broker #2 and #3
        size_t num_msg_brokers = 2;
        data_str_ptr = (const char **)&data_str[20];
        data_uint_ptr = &data_uint[10];
        app_utest__rpc_cfg_setup(objs, num_msg_brokers, data_str_ptr, data_uint_ptr);
        assert_that(mock_app_cfg.rpc.entries, is_null);
        err = parse_cfg_rpc_caller(objs, &mock_app_cfg);
        assert_that(err, is_equal_to(0));
        assert_that(mock_app_cfg.rpc.size, is_equal_to(num_msg_brokers));
        assert_that(mock_app_cfg.rpc.entries, is_not_null);
        utest_app_rpc__verify_broker_2(&mock_app_cfg.rpc.entries[0], data_str_ptr, data_uint_ptr);
        utest_app_rpc__verify_broker_3(&mock_app_cfg.rpc.entries[1], (const char **)&data_str[48],
                &data_uint[23]);
    }
    utest_rpc_dellocate_app_cfg(&mock_app_cfg);
    json_decref(objs);
    objs = json_array();
    { // parse configuration for all three brokers
        size_t num_msg_brokers = 3;
        data_str_ptr = (const char **)&data_str[0];
        data_uint_ptr = &data_uint[0];
        app_utest__rpc_cfg_setup(objs, num_msg_brokers, data_str_ptr, data_uint_ptr);
        err = parse_cfg_rpc_caller(objs, &mock_app_cfg);
        assert_that(err, is_equal_to(0));
        assert_that(mock_app_cfg.rpc.size, is_equal_to(num_msg_brokers));
        assert_that(mock_app_cfg.rpc.entries, is_not_null);
        utest_app_rpc__verify_broker_1(&mock_app_cfg.rpc.entries[0], data_str_ptr, data_uint_ptr);
        utest_app_rpc__verify_broker_2(&mock_app_cfg.rpc.entries[1],
                (const char **)&data_str[20], &data_uint[10]);
        utest_app_rpc__verify_broker_3(&mock_app_cfg.rpc.entries[2],
                (const char **)&data_str[48], &data_uint[23]);
    }
    {
        arpc_cfg_t  *rpc_cfg = NULL;
        arpc_cfg_bind_reply_t *reply_cfg = NULL;
        rpc_cfg   = &mock_app_cfg.rpc.entries[0];
        reply_cfg = &rpc_cfg->bindings.entries[1].reply;
        reply_cfg->queue.render_fn(NULL, NULL, NULL, 0);
        assert_that(reply_cfg->queue.render_fn, is_equal_to(utset_operation_cde__rpc_qname_render));
        rpc_cfg   = &mock_app_cfg.rpc.entries[1];
        reply_cfg = &rpc_cfg->bindings.entries[0].reply;
        assert_that(reply_cfg->correlation_id.render_fn, is_equal_to(utset_operation_efg__rpc_corr_id_render));
    }
    json_decref(objs);
    utest_rpc_dellocate_app_cfg(&mock_app_cfg);
    free(mock_app_cfg.rpc.entries);
} // end of rpc_caller_cfg_reconfig_tests


TestSuite *app_rpc_cfg_parser_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_caller_cfg_incomplete_setting_tests);
    add_test(suite, rpc_caller_cfg_reconfig_tests);
    return suite;
}
