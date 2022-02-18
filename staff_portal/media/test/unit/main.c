#include <cgreen/cgreen.h>

TestSuite *app_cfg_parser_tests(void);
TestSuite *app_network_util_tests(void);
TestSuite *app_cfg_route_tests(void);
TestSuite *app_middleware_tests(void);
TestSuite *app_auth_tests(void);

int main(int argc, char **argv) {
    int result = 0;
    TestSuite *suite = create_named_test_suite("media_app_unit_test");
    TestReporter *reporter = create_text_reporter();
    add_suite(suite, app_cfg_parser_tests());
    add_suite(suite, app_network_util_tests());
    add_suite(suite, app_cfg_route_tests());
    add_suite(suite, app_middleware_tests());
    add_suite(suite, app_auth_tests());
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
