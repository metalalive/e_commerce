#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"
#include "models/datatypes.h"
#include "models/cfg_parser.h"

static  __attribute__((optimize("O0")))  void  utest_app_model__db_ops_cfg(db_3rdparty_ops_t *cfg)
{ mock(cfg); }

Ensure(cfg_databases_missing_alias) {
    json_t *objs = json_array();
    json_t *obj = json_object();
    json_array_append_new(objs, obj);
    json_object_set_new(obj, "max_connections", json_integer(123));
    json_object_set_new(obj, "idle_timeout", json_integer(456));
    int result = parse_cfg_databases(objs, NULL);
    assert_that(result, is_not_equal_to(0));
    json_decref(objs);
} // end of cfg_databases_missing_alias

Ensure(cfg_databases_missing_ops_cfg) {
    json_t *objs = json_array();
    json_t *obj = json_object();
    json_t *credential = json_object();
    json_array_append_new(objs, obj);
    json_object_set_new(obj, "max_connections", json_integer(123));
    json_object_set_new(obj, "idle_timeout", json_integer(456));
    json_object_set_new(obj, "bulk_query_limit_kb", json_integer(7));
    json_object_set_new(obj, "alias",   json_string("service_1_primary"));
    json_object_set_new(obj, "db_name", json_string("service_1_db_primary"));
    json_object_set_new(obj, "init_cfg_ops", json_string("utest_app_model__db_nonexistent_ops_cfg"));
    json_object_set_new(obj, "credential", credential);
    json_object_set_new(credential, "hierarchy", json_array());
    json_object_set_new(credential, "filepath", json_string("invalid/path/to/credential_file"));
    app_cfg_t *global_appcfg = app_get_global_cfg();
    app_cfg_t  mock_app_cfg = {.exe_path=global_appcfg->exe_path};
    int result = parse_cfg_databases(objs, &mock_app_cfg);
    assert_that(result, is_not_equal_to(0));
    json_decref(objs);
} // end of cfg_databases_missing_ops_cfg


Ensure(cfg_databases_missing_credential) {
    json_t *objs = json_array();
    json_t *obj = json_object();
    json_t *credential = json_object();
    json_array_append_new(objs, obj);
    json_object_set_new(obj, "max_connections", json_integer(123));
    json_object_set_new(obj, "idle_timeout", json_integer(456));
    json_object_set_new(obj, "bulk_query_limit_kb", json_integer(7));
    json_object_set_new(obj, "alias",   json_string("service_1_primary"));
    json_object_set_new(obj, "db_name", json_string("service_1_db_primary"));
    json_object_set_new(obj, "init_cfg_ops", json_string("utest_app_model__db_ops_cfg"));
    json_object_set_new(obj, "credential", credential);
    json_object_set_new(credential, "hierarchy", json_array());
    json_object_set_new(credential, "filepath", json_string("invalid/path/to/credential_file"));
    app_cfg_t *global_appcfg = app_get_global_cfg();
    app_cfg_t  mock_app_cfg = {.exe_path=global_appcfg->exe_path};
    expect(utest_app_model__db_ops_cfg);
    int result = parse_cfg_databases(objs, &mock_app_cfg);
    assert_that(result, is_not_equal_to(0));
    json_decref(objs);
} // end of cfg_databases_missing_credential

#pragma GCC push_options
#pragma GCC optimize ("O0")
Ensure(cfg_databases_invalid_credential) {
    int credential_fd = -1;
    char credential_filepath[] = "./tmp/unittest_credential_XXXXXX";
    json_t *objs = json_array();
    json_t *obj = json_object();
    json_t *credential = json_object();
    json_array_append_new(objs, obj);
    json_object_set_new(obj, "max_connections", json_integer(123));
    json_object_set_new(obj, "idle_timeout", json_integer(456));
    json_object_set_new(obj, "bulk_query_limit_kb", json_integer(7));
    json_object_set_new(obj, "alias",   json_string("service_1_primary"));
    json_object_set_new(obj, "db_name", json_string("service_1_db_primary"));
    json_object_set_new(obj, "init_cfg_ops", json_string("utest_app_model__db_ops_cfg"));
    json_object_set_new(obj, "credential", credential);
    app_cfg_t *global_appcfg = app_get_global_cfg();
    app_cfg_t  mock_app_cfg = {.exe_path=global_appcfg->exe_path};
    {
        const char *file_content = "{\"hier1\":{\"hier2\":{\"hier3\":{}}}}";
        credential_fd = mkstemp(&credential_filepath[0]);
        write(credential_fd, file_content, strlen(file_content));
        json_object_set_new(credential, "filepath", json_string(&credential_filepath[0]));
    } {
        json_t *credential_hier_keys = json_array();
        // TODO, Valgrind complained `Address is 4 bytes inside a block of size 6 alloc'd` , WHY should it be error ?
        json_array_append_new(credential_hier_keys, json_stringn("hier1", 7));
        json_array_append_new(credential_hier_keys, json_stringn("hier2", 7));
        json_array_append_new(credential_hier_keys, json_stringn("hier3", 7));
        json_object_set_new(credential, "hierarchy", credential_hier_keys);
    }
    expect(utest_app_model__db_ops_cfg);
    int result = parse_cfg_databases(objs, &mock_app_cfg);
    assert_that(result, is_not_equal_to(0));
    json_decref(objs);
    unlink(&credential_filepath[0]);
    close(credential_fd);
} // end of cfg_databases_invalid_credential
#pragma GCC pop_options

TestSuite *app_model_cfg_parser_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, cfg_databases_missing_alias);
    add_test(suite, cfg_databases_missing_ops_cfg);
    add_test(suite, cfg_databases_missing_credential);
    add_test(suite, cfg_databases_invalid_credential);
    return suite;
}
