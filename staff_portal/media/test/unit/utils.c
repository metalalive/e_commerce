#include <cgreen/cgreen.h>
#include "utils.h"

#define  NUM_TEST_NODES 8
Ensure(app_llnode_link_test) {
    app_llnode_t nodes[NUM_TEST_NODES];
    app_llnode_t *curr, *prev, *new;
    memset(&nodes[0], 0x0, sizeof(nodes));
    {
        new = &nodes[5];
        assert_that(new->next, is_null);
        assert_that(new->prev, is_null);
        app_llnode_link(NULL, NULL, new);
        assert_that(new->next, is_null);
        assert_that(new->prev, is_null);
    }
    { // 7 --> 0
        new  = &nodes[0];
        prev = &nodes[NUM_TEST_NODES - 1];
        app_llnode_link(NULL, prev, new);
        assert_that(new->prev , is_equal_to(prev));
        assert_that(prev->next, is_equal_to(new));
    }
    { // 6 --> 1
        new  = &nodes[6];
        curr = &nodes[1];
        app_llnode_link(curr, NULL, new);
        assert_that(new->next , is_equal_to(curr));
        assert_that(curr->prev, is_equal_to(new));
    }
    { // 7 --> 0 --> 3 --> 6 --> 1
        new  = &nodes[3];
        curr = &nodes[6];
        prev = &nodes[0];
        assert_that(new->next, is_null);
        assert_that(new->prev, is_null);
        app_llnode_link(curr, prev, new);
        assert_that(new->next , is_equal_to(curr));
        assert_that(new->prev , is_equal_to(prev));
        assert_that(curr->prev, is_equal_to(new));
        assert_that(prev->next, is_equal_to(new));
    }
    { // 7 --> 0 --> 3 --> 6 --> 1 --> 4 --> 2 --> 5
        app_llnode_link(&nodes[5], &nodes[4], &nodes[2]);
        app_llnode_link(&nodes[2], &nodes[1], &nodes[4]);
        size_t expect_idx[NUM_TEST_NODES] = {7, 0, 3, 6, 1, 4, 2, 5};
        size_t idx = 0;
        for(curr = &nodes[7]; curr; curr = curr->next) {
            app_llnode_t *expect_node = & nodes[expect_idx[idx++]];
            assert_that(curr, is_equal_to(expect_node));
        }
        for(curr = &nodes[5]; curr; curr = curr->prev) {
            app_llnode_t *expect_node = & nodes[expect_idx[--idx]];
            assert_that(curr, is_equal_to(expect_node));
        }
    }
    { // 7 --> 0 --> 3 --> 6 --> 1 --> 4 --> 2
        prev = &nodes[2];
        curr = &nodes[5];
        assert_that(curr->next , is_null);
        assert_that(curr->prev , is_equal_to(prev));
        app_llnode_unlink(curr);
        assert_that(curr->next , is_null);
        assert_that(curr->prev , is_null);
        assert_that(prev->next , is_null);
    }
    { // 0 --> 3 --> 6 --> 1 --> 4 --> 2
        prev = &nodes[7];
        curr = &nodes[0];
        app_llnode_unlink(prev);
        assert_that(prev->next , is_null);
        assert_that(prev->prev , is_null);
        assert_that(curr->prev , is_null);
    }
    { // 0 --> 3 --> 1 --> 4 --> 2
        curr = &nodes[6];
        app_llnode_unlink(curr);
        assert_that(curr->prev , is_null);
        assert_that(curr->next , is_null);
        assert_that(nodes[3].next , is_equal_to(&nodes[1]));
        assert_that(nodes[1].prev , is_equal_to(&nodes[3]));
    }
} // end of app_llnode_link_test
#undef NUM_TEST_NODES

#define  NUM_ENTRIES_HASHMAP  3
Ensure(app_hashmap_access_test) {
    int err = 0;
    int value = 0;
    struct hsearch_data hmap = {0};
    hcreate_r(NUM_ENTRIES_HASHMAP, &hmap);
    err = app_save_int_to_hashmap(&hmap, "arm64", 0xacce55);
    assert_that(err, is_equal_to(1));
    err = app_save_int_to_hashmap(&hmap, "riscv", 0xa15);
    assert_that(err, is_equal_to(1));
    err = app_save_int_to_hashmap(&hmap, "avr", 0xbeef);
    err = app_save_int_to_hashmap(&hmap, "8080", 0x8080);
    err = app_save_int_to_hashmap(&hmap, "IA64", 0x1a64);
    assert_that(err, is_equal_to(1));
    err = app_save_int_to_hashmap(&hmap, "8052", 0x8052);
    assert_that(err, is_equal_to(0));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    value = (int) app_fetch_from_hashmap(&hmap, "avr");
    assert_that(value, is_equal_to(0xbeef));
    value = (int) app_fetch_from_hashmap(&hmap, "arm64");
    assert_that(value, is_equal_to(0xacce55));
    value = (int) app_fetch_from_hashmap(&hmap, "IA64");
    assert_that(value, is_equal_to(0x1a64));
#pragma GCC diagnostic pop
    hdestroy_r(&hmap);
} // end of app_hashmap_access_test
#undef  NUM_ENTRIES_HASHMAP


#define  EXPECT_NUM_ITEMS  3
#define  RAW_STRING_LEN    80
Ensure(app_url_decode_query_param_test) {
    const char *kv[EXPECT_NUM_ITEMS][2] = {
        {"cumin", "clove"},
        {"wasabi", NULL},
        {"dill", "mustard"},
    }; // pairs of query parameters expected to be in raw URI
    char raw_query_param[RAW_STRING_LEN] = {0};
    // should NOT include question mark --> `?`  symbol
    snprintf(&raw_query_param[0], (size_t)RAW_STRING_LEN, "%s=%s&%s&%s=%s&",
           kv[0][0], kv[0][1], kv[1][0], kv[2][0], kv[2][1] );
    json_t *map = json_object();
    int actual_num_items = app_url_decode_query_param(&raw_query_param[0], map);
    assert_that(actual_num_items, is_equal_to(EXPECT_NUM_ITEMS));
    for(size_t idx = 0; idx < EXPECT_NUM_ITEMS; idx++) {
        json_t *obj = json_object_get(map, kv[idx][0]);
        if(json_is_string(obj)) {
            const char *expect_val = kv[idx][1];
            const char *actual_val = json_string_value(obj);
            assert_that(actual_val, is_equal_to_string(expect_val));
        } else if(json_is_boolean(obj)) {
            uint8_t actual_val = (uint8_t) json_boolean_value(obj);
            assert_that(actual_val, is_equal_to((uint8_t)1));
        } else {
            assert_that(0, is_equal_to(1));
        }
    } // end of loop
    json_decref(map);
} // end of app_url_decode_query_param_test
#undef  RAW_STRING_LEN
#undef  EXPECT_NUM_ITEMS

TestSuite *app_utils_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_llnode_link_test);
    add_test(suite, app_hashmap_access_test);
    add_test(suite, app_url_decode_query_param_test);
    return suite;
} // end of app_utils_tests

