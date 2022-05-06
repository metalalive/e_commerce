#include <cgreen/cgreen.h>
#include "rpc/cfg_parser.h"

static void app_utest__rpc_cfg_setup(json_t *objs, size_t num_brokers, const char **data_str, uint32_t *data_uint)
{
    size_t idx = 0;
    size_t jdx = 0;
    size_t dstr_rd_ptr = 0;
    size_t dint_rd_ptr = 0;
    for(idx = 0; idx < num_brokers; idx++) {
        json_t *broker_cfg = json_object();
        json_array_append(objs, broker_cfg);
        json_t *credential = json_object();
        json_t *attributes = json_object();
        json_t *bindings   = json_array();
        json_object_set(broker_cfg, "credential", credential);
        json_object_set(broker_cfg, "attributes", attributes);
        json_object_set(broker_cfg, "bindings",   bindings  );
        { // assume the hierarchy path is only 2 levels, first level is string, the second one is integer
            json_t *filepath  = json_string(data_str[dstr_rd_ptr++]);
            json_t *hierarchy = json_array();
            json_array_append(hierarchy, json_string(data_str[dstr_rd_ptr++]));
            json_array_append(hierarchy, json_integer(data_uint[dint_rd_ptr++]));
            json_object_set(credential, "filepath" , filepath );
            json_object_set(credential, "hierarchy", hierarchy);
        }
        {
            json_t *vhost = json_string(data_str[dstr_rd_ptr++]);
            json_t *max_channels     = json_integer(data_uint[dint_rd_ptr++]);
            json_t *max_kb_per_frame = json_integer(data_uint[dint_rd_ptr++]);
            json_object_set(attributes, "vhost", vhost);
            json_object_set(attributes, "max_channels", max_channels);
            json_object_set(attributes, "max_kb_per_frame", max_kb_per_frame);
        }
        size_t num_bindings = data_uint[dint_rd_ptr++];
        for(jdx = 0; jdx < num_bindings; jdx++) {
            json_t *bind_obj = json_object();
            json_t *queue    = json_string(data_str[dstr_rd_ptr++]);
            json_t *exchange = json_string(data_str[dstr_rd_ptr++]);
            json_t *routing_key = json_string(data_str[dstr_rd_ptr++]);
            json_t *durable  = json_boolean(data_uint[dint_rd_ptr++]);
            json_object_set(bind_obj, "queue", queue);
            json_object_set(bind_obj, "exchange", exchange);
            json_object_set(bind_obj, "routing_key", routing_key);
            json_object_set(bind_obj, "durable", durable);
            json_array_append(bindings, bind_obj);
        } // end of binding configuration
    } // end of broker configuration
} // end of app_utest__rpc_cfg_setup

#define  NUM_MSG_BROKERS 2
Ensure(rpc_caller_cfg_incomplete_setting_tests) {
    const char *data_str[] = {
        // --------- broker 1 ----------
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost1234",  // attributes vhost
        "queue_01_for_rpc_operation_abc", // broker[0] bindings[0] queue 
        "exchange_02_for_rpc_operation_abc", // broker[0] bindings[0] exchange
        "rpc.media.operation_abc", // broker[0] bindings[0] routing_key
        "queue_03_for_rpc_operation_cde", // broker[0] bindings[1] queue 
        "exchange_04_for_rpc_operation_cde", // broker[0] bindings[1] exchange
        "rpc.media.operation_cde", // broker[0] bindings[1] routing_key
        // --------- broker 2 ----------
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost5678",  // attributes vhost
        "queue_07_for_rpc_operation_efg", // broker[1] bindings[0] queue 
        "exchange_08_for_rpc_operation_efg", // broker[1] bindings[0] exchange
        "rpc.media.operation_efg", // broker[1] bindings[0] routing_key
        "queue_09_for_rpc_operation_ghi", // broker[1] bindings[1] queue 
        "exchange_11_for_rpc_operation_ghi", // broker[1] bindings[1] exchange
        "rpc.media.operation_ghi", // broker[1] bindings[1] routing_key
        "queue_12_for_rpc_operation_ijk", // broker[1] bindings[2] queue 
        "exchange_13_for_rpc_operation_ijk", // broker[1] bindings[2] exchange
        NULL, // broker[1] bindings[2] routing_key , expect to report error from here
    }; // end of data_str
    uint32_t data_uint[] = {
        // --------- broker 1 ----------
        0, // credential hierarchy
        370, // attributes max_channels
        63,  // attributes max_kb_per_frame
        2, // number of bindings in current broker configuration
        0, // broker[0] bindings[0] durable
        1, // broker[0] bindings[1] durable
        // --------- broker 2 ----------
        1, // credential hierarchy
        800, // attributes max_channels
        43,  // attributes max_kb_per_frame
        3, // number of bindings in current broker configuration
        0, // broker[1] bindings[0] durable
        1, // broker[1] bindings[1] durable
        0, // broker[1] bindings[2] durable
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


static void utest_app_rpc__verify_broker_1(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[2]));
    assert_that(rpc_cfg->attributes.max_channels, is_equal_to(data_uint_ptr[1]));
    assert_that(rpc_cfg->attributes.max_kb_per_frame, is_equal_to(data_uint_ptr[2]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[1];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[6]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[7]));
    assert_that(bind_cfg->routing_key, is_equal_to_string(data_str_ptr[8]));
} // end of utest_app_rpc__verify_broker_1

static void utest_app_rpc__verify_broker_2(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[2]));
    assert_that(rpc_cfg->attributes.max_channels, is_equal_to(data_uint_ptr[1]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[0];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[3]));
    assert_that(bind_cfg->routing_key, is_equal_to_string(data_str_ptr[5]));
    assert_that(bind_cfg->flags.durable, is_equal_to(data_uint_ptr[4]));
    bind_cfg = &rpc_cfg->bindings.entries[1];
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[7]));
    assert_that(bind_cfg->flags.durable, is_equal_to(data_uint_ptr[5]));
    bind_cfg = &rpc_cfg->bindings.entries[2];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[9]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[10]));
} // end of utest_app_rpc__verify_broker_2

static void utest_app_rpc__verify_broker_3(arpc_cfg_t *rpc_cfg, const char **data_str_ptr, uint32_t *data_uint_ptr)
{
    arpc_cfg_bind_t *bind_cfg = NULL;
    assert_that(rpc_cfg->attributes.vhost, is_equal_to_string(data_str_ptr[2]));
    assert_that(rpc_cfg->attributes.max_kb_per_frame, is_equal_to(data_uint_ptr[2]));
    assert_that(rpc_cfg->bindings.size, is_equal_to(data_uint_ptr[3]));
    bind_cfg = &rpc_cfg->bindings.entries[0];
    assert_that(bind_cfg->q_name, is_equal_to_string(data_str_ptr[3]));
    assert_that(bind_cfg->exchange_name, is_equal_to_string(data_str_ptr[4]));
} // end of utest_app_rpc__verify_broker_3


Ensure(rpc_caller_cfg_reconfig_tests) {
    const char *data_str[] = {
        // --------- broker 1 ----------
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost1234",  // attributes vhost
        "queue_01_for_rpc_operation_abc", // broker[0] bindings[0] queue 
        "exchange_02_for_rpc_operation_abc", // broker[0] bindings[0] exchange
        "rpc.media.operation_abc", // broker[0] bindings[0] routing_key
        "queue_03_for_rpc_operation_cde", // broker[0] bindings[1] queue 
        "exchange_04_for_rpc_operation_cde", // broker[0] bindings[1] exchange
        "rpc.media.operation_cde", // broker[0] bindings[1] routing_key
        // --------- broker 2 ----------
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost5678",  // attributes vhost
        "queue_07_for_rpc_operation_efg", // broker[1] bindings[0] queue 
        "exchange_08_for_rpc_operation_efg", // broker[1] bindings[0] exchange
        "rpc.media.operation_efg", // broker[1] bindings[0] routing_key
        "queue_09_for_rpc_operation_ghi", // broker[1] bindings[1] queue 
        "exchange_11_for_rpc_operation_ghi", // broker[1] bindings[1] exchange
        "rpc.media.operation_ghi", // broker[1] bindings[1] routing_key
        "queue_12_for_rpc_operation_ijk", // broker[1] bindings[2] queue 
        "exchange_13_for_rpc_operation_ijk", // broker[1] bindings[2] exchange
        "rpc.media.operation_ijk", // broker[1] bindings[2] routing_key
        // --------- broker 3 ----------
        "common/data/secrets.json", // credential filepath
        "amqp_broker", // credential hierarchy
        "/vhost_practice_tdd",  // attributes vhost
        "queue_12_for_rpc_operation_kmn", // broker[2] bindings[0] queue 
        "exchange_13_for_rpc_operation_kmn", // broker[2] bindings[0] exchange
        "rpc.media.operation_kmn", // broker[2] bindings[0] routing_key
    }; // end of data_str
    uint32_t data_uint[] = {
        // --------- broker 1 ----------
        0, // credential hierarchy
        370, // attributes max_channels
        63,  // attributes max_kb_per_frame
        2, // number of bindings in current broker configuration
        0, // broker[0] bindings[0] durable
        1, // broker[0] bindings[1] durable
        // --------- broker 2 ----------
        1, // credential hierarchy
        800, // attributes max_channels
        43,  // attributes max_kb_per_frame
        3, // number of bindings in current broker configuration
        0, // broker[1] bindings[0] durable
        1, // broker[1] bindings[1] durable
        0, // broker[1] bindings[2] durable
        // --------- broker 3 ----------
        0, // credential hierarchy
        626, // attributes max_channels
        85,  // attributes max_kb_per_frame
        1, // number of bindings in current broker configuration
        1, // broker[2] bindings[0] durable
    }; // end of data_uint
    const char **data_str_ptr  = NULL;
    uint32_t    *data_uint_ptr = NULL;
    app_cfg_t  mock_app_cfg = {0};
    json_t *objs = json_array();
    int err = 0;
    { // parse configuration for broker #2 and #3
        size_t num_msg_brokers = 2;
        data_str_ptr = (const char **)&data_str[9];
        data_uint_ptr = &data_uint[6];
        app_utest__rpc_cfg_setup(objs, num_msg_brokers, data_str_ptr, data_uint_ptr);
        assert_that(mock_app_cfg.rpc.entries, is_null);
        err = parse_cfg_rpc_caller(objs, &mock_app_cfg);
        assert_that(err, is_equal_to(0));
        assert_that(mock_app_cfg.rpc.size, is_equal_to(num_msg_brokers));
        assert_that(mock_app_cfg.rpc.entries, is_not_null);
        utest_app_rpc__verify_broker_2(&mock_app_cfg.rpc.entries[0], data_str_ptr, data_uint_ptr);
        utest_app_rpc__verify_broker_3(&mock_app_cfg.rpc.entries[1],
                (const char **)&data_str[21], &data_uint[13]);
    }
    { // parse configuration for all three brokers
        size_t num_msg_brokers = 3;
        data_str_ptr = (const char **)&data_str[0];
        data_uint_ptr = &data_uint[0];
        json_array_clear(objs);
        app_utest__rpc_cfg_setup(objs, num_msg_brokers, data_str_ptr, data_uint_ptr);
        err = parse_cfg_rpc_caller(objs, &mock_app_cfg);
        assert_that(err, is_equal_to(0));
        assert_that(mock_app_cfg.rpc.size, is_equal_to(num_msg_brokers));
        assert_that(mock_app_cfg.rpc.entries, is_not_null);
        utest_app_rpc__verify_broker_1(&mock_app_cfg.rpc.entries[0], data_str_ptr, data_uint_ptr);
        utest_app_rpc__verify_broker_2(&mock_app_cfg.rpc.entries[1],
                (const char **)&data_str[9], &data_uint[6]);
        utest_app_rpc__verify_broker_3(&mock_app_cfg.rpc.entries[2],
                (const char **)&data_str[21], &data_uint[13]);
    }
} // end of rpc_caller_cfg_reconfig_tests


TestSuite *app_rpc_cfg_parser_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_caller_cfg_incomplete_setting_tests);
    add_test(suite, rpc_caller_cfg_reconfig_tests);
    return suite;
}
