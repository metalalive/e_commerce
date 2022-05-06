#include <cgreen/cgreen.h>
#include "rpc/core.h"

Ensure(rpc_core_test_start) {
} // end of rpc_core_test_start

TestSuite *app_rpc_core_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, rpc_core_test_start);
    return suite;
}
