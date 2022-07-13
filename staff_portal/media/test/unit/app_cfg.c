#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"

Ensure(appcfg_parse_errlog_path_test) {
#define ERR_LOG_FILEPATH "./tmp/log/utest_app_err.log"
    int saved_stdout_fd = dup(1);
    app_cfg_t acfg = {0};
    json_t *obj = json_string(ERR_LOG_FILEPATH);
    int err = appcfg_parse_errlog_path(obj, &acfg);
    assert_that(err, is_equal_to(0));
    assert_that(acfg.error_log_fd, is_greater_than(2));
    assert_that(access(ERR_LOG_FILEPATH, F_OK), is_equal_to(0));
    dup2(saved_stdout_fd, 1);
    deinit_app_cfg(&acfg);
    assert_that(acfg.error_log_fd, is_equal_to(-1));
    unlink(ERR_LOG_FILEPATH);
    assert_that(access(ERR_LOG_FILEPATH, F_OK), is_equal_to(-1));
    json_decref(obj);
    sleep(1); // TODO, wait until messages are redirected to stdout
#undef ERR_LOG_FILEPATH
} // end of appcfg_parse_errlog_path_test

Ensure(appcfg_parse_num_workers_test) {
#define  NUM_EXPECT_WORKERS  3
    app_cfg_t acfg = {0};
    json_t *obj = json_integer(NUM_EXPECT_WORKERS);
    int err = appcfg_parse_num_workers(obj, &acfg);
    assert_that(err, is_equal_to(0));
    assert_that(acfg.workers.size, is_equal_to(NUM_EXPECT_WORKERS));
    assert_that(acfg.workers.entries, is_not_null);
    deinit_app_cfg(&acfg);
    assert_that(acfg.workers.entries, is_null);
    json_decref(obj);
#undef NUM_EXPECT_WORKERS
} // end of appcfg_parse_num_workers_test

static void utest_appcfg__worker_entry(void *data)
{
    struct worker_init_data_t *actual_data = (struct worker_init_data_t *)data;
    assert_that(actual_data->app_cfg, is_not_null);
    assert_that(actual_data->loop, is_not_null);
} // end of utest_appcfg__worker_entry

Ensure(appcfg_start_workers_test) {
#define  NUM_EXPECT_WORKERS  2
    app_cfg_t acfg = {0};
    struct worker_init_data_t expect_data[NUM_EXPECT_WORKERS + 1] = {0};
    json_t *obj = json_integer(NUM_EXPECT_WORKERS);
    appcfg_parse_num_workers(obj, &acfg);
    json_decref(obj);
    int err = appcfg_start_workers(&acfg, &expect_data[0], utest_appcfg__worker_entry);
    assert_that(err, is_equal_to(0));
    appcfg_terminate_workers(&acfg, expect_data);
    deinit_app_cfg(&acfg);
#undef NUM_EXPECT_WORKERS
} // end of appcfg_start_workers_test

TestSuite *app_appcfg_generic_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, appcfg_parse_errlog_path_test);
    add_test(suite, appcfg_parse_num_workers_test);
    add_test(suite, appcfg_start_workers_test);
    return suite;
}
