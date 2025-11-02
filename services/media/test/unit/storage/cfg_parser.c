#include <sysexits.h>
#include <h2o.h>
#include <h2o/serverutil.h>
#include <cgreen/cgreen.h>
#include "storage/cfg_parser.h"

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_mkdir_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_rmdir_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_scandir_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE
utest_storage_scandir_next_fn(asa_op_base_cfg_t *cfg, asa_dirent_t *ent) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_rename_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_unlink_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_open_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_close_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_seek_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_write_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) ASA_RES_CODE utest_storage_read_fn(asa_op_base_cfg_t *cfg) {
    return ASTORAGE_RESULT_ACCEPT;
}

static __attribute__((optimize("O0"))) size_t utest_storage_typesize_fn(void) { return 123; }

Ensure(storage_cfg_incomplete_setting_tests) {
    json_t   *objs = json_array(), *obj0 = json_object(), *ops = json_object();
    app_cfg_t app_cfg = {0};
    int       err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(EX_CONFIG));
    assert_that(app_cfg.storages.entries, is_null);
    {
        app_cfg.exe_path = "/path/to/unknown/image";
        json_array_append_new(objs, obj0);
        json_object_set_new(obj0, "alias", json_string("storage_dst_1"));
        json_object_set_new(obj0, "base_path", json_string("/path/to/file/store"));
    }
    { // lacking part of function names in config json object
        json_object_set_new(ops, "open", json_string("utest_storage_open_fn"));
        json_object_set_new(ops, "close", json_string("utest_storage_close_fn"));
        json_object_set_new(obj0, "ops", ops);
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(EX_CONFIG));
    assert_that(app_cfg.storages.entries, is_null);
    { // incorrect image path, failed to parse functions from image
        json_object_set_new(ops, "write", json_string("utest_storage_write_fn"));
        json_object_set_new(ops, "read", json_string("utest_storage_read_fn"));
        json_object_set_new(ops, "seek", json_string("utest_storage_seek_fn"));
        json_object_set_new(ops, "mkdir", json_string("utest_storage_mkdir_fn"));
        json_object_set_new(ops, "rmdir", json_string("utest_storage_rmdir_fn"));
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(EX_CONFIG));
    assert_that(app_cfg.storages.entries, is_null);
    json_decref(objs);
} // end of storage_cfg_incomplete_setting_tests

Ensure(storage_cfg_missing_operation_fn_tests) {
    json_t   *objs = json_array(), *obj0 = json_object(), *ops = json_object();
    app_cfg_t app_cfg = {.exe_path = "media/build/unit_test.out"};
    app_load_envvars(&app_cfg.env_vars);
    {
        json_array_append_new(objs, obj0);
        json_object_set_new(obj0, "alias", json_string("storage_dst_1"));
        json_object_set_new(obj0, "base_path", json_string("/path/to/file/store"));
        json_object_set_new(ops, "open", json_string("utest_storage_open_fn"));
        json_object_set_new(ops, "close", json_string("utest_storage_close_fn"));
        json_object_set_new(ops, "write", json_string("utest_storage_write_fn_assume_non_existent"));
        json_object_set_new(ops, "read", json_string("utest_storage_read_fn"));
        json_object_set_new(ops, "seek", json_string("utest_storage_seek_fn"));
        json_object_set_new(ops, "mkdir", json_string("utest_storage_mkdir_fn"));
        json_object_set_new(ops, "rmdir", json_string("utest_storage_rmdir_fn"));
        json_object_set_new(obj0, "ops", ops);
    }
    int err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(EX_CONFIG));
    assert_that(app_cfg.storages.entries, is_null);
    {
        json_object_set_new(ops, "write", json_string("utest_storage_write_fn"));
        json_object_set_new(ops, "unlink", json_string("utest_storage_unlink_fn"));
        json_object_set_new(ops, "rename", json_string("utest_storage_rename_fn"));
        json_object_set_new(ops, "scandir", json_string("utest_storage_scandir_fn"));
        json_object_set_new(ops, "scandir_next", json_string("utest_storage_scandir_next_fn"));
        json_object_set_new(ops, "typesize", json_string("utest_storage_typesize_fn"));
    }
    err = parse_cfg_storages(objs, &app_cfg);
    assert_that(err, is_equal_to(EX_OK));
    assert_that(app_cfg.storages.entries, is_not_null);
    assert_that(app_cfg.storages.size, is_equal_to(1));
    if (app_cfg.storages.entries && app_cfg.storages.size == 1) {
        asa_cfg_ops_t *parsed_ops = &app_cfg.storages.entries[0].ops;
        assert_that(parsed_ops->fn_open, is_equal_to(utest_storage_open_fn));
        assert_that(parsed_ops->fn_close, is_equal_to(utest_storage_close_fn));
        assert_that(parsed_ops->fn_write, is_equal_to(utest_storage_write_fn));
        assert_that(parsed_ops->fn_read, is_equal_to(utest_storage_read_fn));
        assert_that(parsed_ops->fn_seek, is_equal_to(utest_storage_seek_fn));
        assert_that(parsed_ops->fn_mkdir, is_equal_to(utest_storage_mkdir_fn));
        assert_that(parsed_ops->fn_rmdir, is_equal_to(utest_storage_rmdir_fn));
        assert_that(parsed_ops->fn_unlink, is_equal_to(utest_storage_unlink_fn));
        assert_that(parsed_ops->fn_rename, is_equal_to(utest_storage_rename_fn));
        assert_that(parsed_ops->fn_scandir, is_equal_to(utest_storage_scandir_fn));
        assert_that(parsed_ops->fn_scandir_next, is_equal_to(utest_storage_scandir_next_fn));
    }
    app_storage_cfg_deinit(&app_cfg);
    json_decref(objs);
} // end of storage_cfg_missing_operation_fn_tests

TestSuite *app_storage_cfg_parser_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, storage_cfg_incomplete_setting_tests);
    add_test(suite, storage_cfg_missing_operation_fn_tests);
    return suite;
} // end of app_storage_cfg_parser_tests
