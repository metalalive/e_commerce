#include <h2o.h>
#include <h2o/serverutil.h>
#include <cgreen/cgreen.h>
#include "storage/cfg_parser.h"

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_mkdir_fn(asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_rmdir_fn(asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_open_fn(asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_close_fn(asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_seek_fn( asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_write_fn(asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_read_fn( asa_op_base_cfg_t *cfg)
{ return ASTORAGE_RESULT_ACCEPT; }


Ensure(storage_cfg_incomplete_setting_tests) {
    json_t *objs = json_array();
    json_t *obj0 = json_object();
    json_t *ops = json_object();
    app_cfg_t  app_cfg = {0};
    int err = 0;
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(-1));
    assert_that(app_cfg.storages.entries, is_null);
    {
        app_cfg.exe_path = "/path/to/unknown/image";
        json_array_append(objs, obj0);
        json_object_set(obj0, "alias", json_string("storage_dst_1"));
        json_object_set(obj0, "base_path", json_string("/path/to/file/store"));
    }
    { // lacking part of function names in config json object
        json_object_set(ops, "open" , json_string("utest_storage_open_fn"));
        json_object_set(ops, "close", json_string("utest_storage_close_fn"));
        json_object_set(obj0, "ops", ops);
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(-1));
    assert_that(app_cfg.storages.entries, is_null);
    { // incorrect image path, failed to parse functions from image
        json_object_set(ops, "write", json_string("utest_storage_write_fn"));
        json_object_set(ops, "read", json_string("utest_storage_read_fn"));
        json_object_set(ops, "seek", json_string("utest_storage_seek_fn"));
        json_object_set(ops, "mkdir", json_string("utest_storage_mkdir_fn"));
        json_object_set(ops, "rmdir", json_string("utest_storage_rmdir_fn"));
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(-1));
    assert_that(app_cfg.storages.entries, is_null);
    json_decref(objs);
} // end of storage_cfg_incomplete_setting_tests


Ensure(storage_cfg_missing_operation_fn_tests) {
    json_t *objs = json_array();
    json_t *obj0 = json_object();
    json_t *ops = json_object();
    app_cfg_t  app_cfg = {.exe_path = "media/build/unit_test.out"};
    int err = 0;
    {
        json_array_append(objs, obj0);
        json_object_set(obj0, "alias", json_string("storage_dst_1"));
        json_object_set(obj0, "base_path", json_string("/path/to/file/store"));
        json_object_set(ops, "open" , json_string("utest_storage_open_fn"));
        json_object_set(ops, "close", json_string("utest_storage_close_fn"));
        json_object_set(ops, "write", json_string("utest_storage_write_fn_assume_non_existent"));
        json_object_set(ops, "read", json_string("utest_storage_read_fn"));
        json_object_set(ops, "seek", json_string("utest_storage_seek_fn"));
        json_object_set(obj0, "ops", ops);
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(-1));
    assert_that(app_cfg.storages.entries, is_null);
    {
        json_object_set(ops, "write", json_string("utest_storage_write_fn"));
        json_object_set(ops, "mkdir", json_string("utest_storage_mkdir_fn"));
        json_object_set(ops, "rmdir", json_string("utest_storage_rmdir_fn"));
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(0));
    assert_that(app_cfg.storages.entries, is_not_null);
    assert_that(app_cfg.storages.size, is_equal_to(1));
    if(app_cfg.storages.entries && app_cfg.storages.size == 1) {
        asa_cfg_ops_t *parsed_ops = &app_cfg.storages.entries[0].ops;
        assert_that(parsed_ops->fn_open,  is_equal_to(utest_storage_open_fn));
        assert_that(parsed_ops->fn_close, is_equal_to(utest_storage_close_fn));
        assert_that(parsed_ops->fn_write, is_equal_to(utest_storage_write_fn));
        assert_that(parsed_ops->fn_read,  is_equal_to(utest_storage_read_fn));
        assert_that(parsed_ops->fn_seek,  is_equal_to(utest_storage_seek_fn));
        assert_that(parsed_ops->fn_mkdir, is_equal_to(utest_storage_mkdir_fn));
        assert_that(parsed_ops->fn_rmdir, is_equal_to(utest_storage_rmdir_fn));
    }
    json_decref(objs);
} // end of storage_cfg_missing_operation_fn_tests


TestSuite *app_storage_cfg_parser_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, storage_cfg_incomplete_setting_tests);
    add_test(suite, storage_cfg_missing_operation_fn_tests);
    return suite;
} // end of app_storage_cfg_parser_tests
