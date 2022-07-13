#include <cgreen/cgreen.h>
#include "middleware.h"

#define  GEN_TEST_HANDLER_FUNC(num) \
    static int test_handler_node_##num(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) \
    { \
        ENTRY  e = {.key="sum", .data = NULL}; \
        ENTRY *e_ret = NULL; \
        hsearch_r(e, FIND,  &e_ret, node->data);  \
        int curr_sum = *(int *)e_ret->data; \
        *(int *)e_ret->data = curr_sum * (num); \
        if(!node->next) { \
            assert_after_run_middleware(node->data); \
        } \
        app_run_next_middleware(self, req, node); \
        return 0; \
    }


static void assert_after_run_middleware(struct hsearch_data *htab);
GEN_TEST_HANDLER_FUNC(1)
GEN_TEST_HANDLER_FUNC(2)
GEN_TEST_HANDLER_FUNC(3)
GEN_TEST_HANDLER_FUNC(4)
GEN_TEST_HANDLER_FUNC(5)
GEN_TEST_HANDLER_FUNC(6)
GEN_TEST_HANDLER_FUNC(7)

Ensure(gen_middleware_tests) {
#define EXPECT_FN_ORDER  test_handler_node_2,  test_handler_node_5, test_handler_node_1, test_handler_node_7, test_handler_node_4
#define NUM_FNS  5
    ENTRY  e = {.key = NULL, .data = NULL };
    ENTRY *e_ret = NULL;
    app_middleware_fn  expect_fn_order[] = {EXPECT_FN_ORDER};
    app_middleware_node_t *head = app_gen_middleware_chain(NUM_FNS, EXPECT_FN_ORDER);
    app_middleware_node_t *curr = head;
    int idx = 0;
    for(idx = 0; idx < NUM_FNS && curr; idx++) {
        assert_that(curr->fn, is_equal_to(expect_fn_order[idx]));
        curr = curr->next;
    } // end of loop
    char *test_keys[] = {"j83","4t9","34u","9ut","4yu","wrf","xin","mqf", NULL};
    const char *test_data = "qwertyuiopasdfghjklzxcvbnm";
    for(idx = 0; test_keys[idx]; idx++) {
        e.key = test_keys[idx];
        e.data = (void *)&test_data[idx];
        assert_that( hsearch_r(e, ENTER, &e_ret, head->data) , is_not_equal_to(0) );
    } // end of loop
    for(idx = 0; test_keys[idx]; idx++) {
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
#undef EXPECT_FN_ORDER
#undef NUM_FNS
} // end of gen_middleware_tests


#define NUM_FNS  7
#define EXPECT_FN_ORDER  test_handler_node_2,  test_handler_node_5, test_handler_node_1, \
    test_handler_node_7, test_handler_node_4, test_handler_node_3, test_handler_node_6
static int multiply_recursive(int n) {
    assert(n >= 0);
    return (n < 2 ? 1: n * multiply_recursive(n - 1));
}

static void assert_after_run_middleware(struct hsearch_data *htab)
{
    ENTRY  e = {.key="sum", .data=NULL};
    ENTRY *e_ret = NULL;
    hsearch_r(e, FIND, &e_ret, htab);
    assert_that(e_ret, is_not_null);
    int expect_sum = multiply_recursive(NUM_FNS);
    int actual_sum = *(int*)e_ret->data;
    assert_that(actual_sum, is_equal_to(expect_sum));
}

Ensure(run_next_middleware_tests) {
    app_middleware_node_t *head = app_gen_middleware_chain(NUM_FNS, EXPECT_FN_ORDER);
    h2o_handler_t  mock_hdlr = {0};
    h2o_req_t      mock_req  = {0};
    int actual_sum = 1;
    ENTRY  e = {.key="sum" , .data=(void *)&actual_sum};
    ENTRY *e_ret = NULL;
    hsearch_r(e, ENTER, &e_ret, head->data);
    head->fn(&mock_hdlr, &mock_req, head);
} // end of run_next_middleware_tests
#undef EXPECT_FN_ORDER
#undef NUM_FNS


TestSuite *app_middleware_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, gen_middleware_tests);
    add_test(suite, run_next_middleware_tests);
    return suite;
}
