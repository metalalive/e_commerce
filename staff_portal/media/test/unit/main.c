#include <cgreen/cgreen.h>
#include "app_cfg.h"

TestSuite *app_appcfg_generic_tests(void);
TestSuite *app_cfg_parser_tests(void);
TestSuite *app_rpc_cfg_parser_tests(void);
TestSuite *app_rpc_core_tests(void);
TestSuite *app_storage_cfg_parser_tests(void);
TestSuite *app_storage_localfs_tests(void);
TestSuite *app_network_util_tests(void);
TestSuite *app_cfg_route_tests(void);
TestSuite *app_middleware_tests(void);
TestSuite *app_multipart_parsing_tests(void);
TestSuite *app_auth_tests(void);
TestSuite *app_utils_tests(void);
TestSuite *app_timer_poll_tests(void);
TestSuite *app_model_pool_tests(void);
TestSuite *app_model_connection_tests(void);
TestSuite *app_model_mariadb_tests(void);
TestSuite *app_views_common_tests(void);
TestSuite *app_model_query_tests(void);

int main(int argc, char **argv) {
    int result = 0;
    TestSuite *suite = create_named_test_suite("media_app_unit_test");
    TestReporter *reporter = create_text_reporter();
    app_global_cfg_set_exepath("./media/build/unit_test.out");
    add_suite(suite, app_cfg_parser_tests());
    add_suite(suite, app_rpc_cfg_parser_tests());
    add_suite(suite, app_rpc_core_tests());
    add_suite(suite, app_storage_cfg_parser_tests());
    add_suite(suite, app_storage_localfs_tests());
    add_suite(suite, app_network_util_tests());
    add_suite(suite, app_cfg_route_tests());
    add_suite(suite, app_middleware_tests());
    add_suite(suite, app_multipart_parsing_tests());
    add_suite(suite, app_auth_tests());
    add_suite(suite, app_utils_tests());
    add_suite(suite, app_timer_poll_tests());
    add_suite(suite, app_model_pool_tests());
    add_suite(suite, app_model_connection_tests());
    add_suite(suite, app_model_mariadb_tests());
    add_suite(suite, app_model_query_tests());
    add_suite(suite, app_views_common_tests());
    add_suite(suite, app_appcfg_generic_tests());
    if(argc > 1) {
        const char *test_name = argv[argc - 1];
        result = run_single_test(suite, test_name, reporter);
    } else {
        result = run_test_suite(suite, reporter);
    }
    destroy_test_suite(suite);
    destroy_reporter(reporter);
    return result;
}
