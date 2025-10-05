#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include "middleware.h"

#define MANDATORY_EXE_FLG 1

#define GEN_TEST_HANDLER_FUNC(num) \
    static int test_handler_node_##num(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) { \
        ENTRY  e = {.key = "sum", .data = NULL}; \
        ENTRY *e_ret = NULL; \
        hsearch_r(e, FIND, &e_ret, node->data); \
        int curr_sum = *(int *)e_ret->data; \
        curr_sum = curr_sum * (num); \
        int mock_status = (int)mock(curr_sum); \
        if (mock_status > 0) \
            req->res.status = mock_status; \
        *(int *)e_ret->data = curr_sum; \
        app_run_next_middleware(self, req, node); \
        return 0; \
    }

GEN_TEST_HANDLER_FUNC(1)
GEN_TEST_HANDLER_FUNC(2)
GEN_TEST_HANDLER_FUNC(3)
GEN_TEST_HANDLER_FUNC(4)
GEN_TEST_HANDLER_FUNC(5)
GEN_TEST_HANDLER_FUNC(6)
GEN_TEST_HANDLER_FUNC(7)

Ensure(gen_middleware_tests) {
#define NUM_FNS  5
#define NUM_ARGS (NUM_FNS << 1)
    ENTRY             e = {.key = NULL, .data = NULL};
    ENTRY            *e_ret = NULL;
    app_middleware_fn expect_fn_order[] = {
        test_handler_node_2, test_handler_node_5, test_handler_node_1, test_handler_node_7,
        test_handler_node_4
    };
    app_middleware_node_t *head = app_gen_middleware_chain(
        NUM_ARGS, test_handler_node_2, MANDATORY_EXE_FLG, test_handler_node_5, MANDATORY_EXE_FLG,
        test_handler_node_1, MANDATORY_EXE_FLG, test_handler_node_7, MANDATORY_EXE_FLG, test_handler_node_4,
        MANDATORY_EXE_FLG
    );
    app_middleware_node_t *curr = head;
    int                    idx = 0;
    for (idx = 0; idx < NUM_FNS && curr; idx++) {
        assert_that(curr->fn, is_equal_to(expect_fn_order[idx]));
        assert_that(curr->flags.mandatory, is_equal_to(MANDATORY_EXE_FLG));
        curr = curr->next;
    } // end of loop
    char       *test_keys[] = {"j83", "4t9", "34u", "9ut", "4yu", "wrf", "xin", "mqf", NULL};
    const char *test_data = "qwertyuiopasdfghjklzxcvbnm";
    for (idx = 0; test_keys[idx]; idx++) {
        e.key = test_keys[idx];
        e.data = (void *)&test_data[idx];
        assert_that(hsearch_r(e, ENTER, &e_ret, head->data), is_not_equal_to(0));
    } // end of loop
    for (idx = 0; test_keys[idx]; idx++) {
        e.key = test_keys[idx];
        e.data = NULL;
        e_ret = NULL;
        hsearch_r(e, FIND, &e_ret, head->data);
        assert_that(e_ret, is_not_null);
        assert_that(e_ret->data, is_equal_to(&test_data[idx]));
    } // end of loop
    e.key = "middleware_chain_head";
    e.data = NULL;
    e_ret = NULL;
    hsearch_r(e, FIND, &e_ret, head->data);
    assert_that(e_ret, is_not_null);
    assert_that(e_ret->data, is_equal_to(head));
    app_cleanup_middlewares(head);
#undef NUM_ARGS
#undef NUM_FNS
} // end of gen_middleware_tests

Ensure(run_all_middlewares_tests) {
#define EXPECT_FN_ORDER \
    test_handler_node_2, MANDATORY_EXE_FLG, test_handler_node_5, MANDATORY_EXE_FLG, test_handler_node_1, \
        MANDATORY_EXE_FLG, test_handler_node_7, MANDATORY_EXE_FLG, test_handler_node_4, MANDATORY_EXE_FLG, \
        test_handler_node_3, MANDATORY_EXE_FLG, test_handler_node_6, MANDATORY_EXE_FLG
#define NUM_FNS  7
#define NUM_ARGS (NUM_FNS << 1)
    app_middleware_node_t *head = app_gen_middleware_chain(NUM_ARGS, EXPECT_FN_ORDER);
    h2o_handler_t          mock_hdlr = {0};
    h2o_req_t              mock_req = {0};
    int                    expect_sum = 1, actual_sum = 1;
    ENTRY                  e = {.key = "sum", .data = (void *)&actual_sum};
    ENTRY                 *e_ret = NULL;
    hsearch_r(e, ENTER, &e_ret, head->data);
    expect_sum = expect_sum * 2;
    expect(test_handler_node_2, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 5;
    expect(test_handler_node_5, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 1;
    expect(test_handler_node_1, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 7;
    expect(test_handler_node_7, will_return(201), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 4;
    expect(test_handler_node_4, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 3;
    expect(test_handler_node_3, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 6;
    expect(test_handler_node_6, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    head->fn(&mock_hdlr, &mock_req, head);
    assert_that(mock_req.res.status, is_equal_to(201));
#undef EXPECT_FN_ORDER
#undef NUM_FNS
#undef NUM_ARGS
} // end of run_all_middlewares_tests

Ensure(run_mandatory_middlewares_tests) {
#define EXPECT_FN_ORDER \
    test_handler_node_2, 0, test_handler_node_5, 0, test_handler_node_1, 0, test_handler_node_7, 0, \
        test_handler_node_4, 1, test_handler_node_3, 0, test_handler_node_6, 1
#define NUM_FNS  7
#define NUM_ARGS (NUM_FNS << 1)
    app_middleware_node_t *head = app_gen_middleware_chain(NUM_ARGS, EXPECT_FN_ORDER);
    h2o_handler_t          mock_hdlr = {0};
    h2o_req_t              mock_req = {0};
    int                    expect_sum = 1, actual_sum = 1;
    ENTRY                  e = {.key = "sum", .data = (void *)&actual_sum};
    ENTRY                 *e_ret = NULL;
    hsearch_r(e, ENTER, &e_ret, head->data);
    expect_sum = expect_sum * 2;
    expect(test_handler_node_2, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 5;
    expect(test_handler_node_5, will_return(409), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 4;
    expect(test_handler_node_4, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    expect_sum = expect_sum * 6;
    expect(test_handler_node_6, will_return(0), when(curr_sum, is_equal_to(expect_sum)));
    head->fn(&mock_hdlr, &mock_req, head);
    assert_that(mock_req.res.status, is_equal_to(409));
#undef EXPECT_FN_ORDER
#undef NUM_FNS
#undef NUM_ARGS
} // end of run_mandatory_middlewares_tests

TestSuite *app_middleware_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, gen_middleware_tests);
    add_test(suite, run_all_middlewares_tests);
    add_test(suite, run_mandatory_middlewares_tests);
    return suite;
}
