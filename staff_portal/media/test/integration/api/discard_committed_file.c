#include "../test/integration/test.h"

#define  ITEST_HOST  "localhost:8010"
#define  ITEST_URL_PATTERN  "https://" ITEST_HOST "/file?" API_QPARAM_LABEL__RESOURCE_ID "=%s"

static void test_verify__discard_file(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_test__discard_committed_video__ok_1) {
    const char *mock_resource_id = "1b2934ad4e2c9";
    size_t  url_sz = sizeof(ITEST_URL_PATTERN) + strlen(mock_resource_id) + 1;
    char url[url_sz];
    sprintf(&url[0], ITEST_URL_PATTERN, mock_resource_id);
    const char *codename_list[3] = {"upload_files", "edit_file_access_control", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__discard_file, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
}

Ensure(api_test__discard_committed_video__ok_2) {
} // end of  api_test__discard_committed_video__ok_2

Ensure(api_test__discard_committed_image__ok_1) {
} // end of  api_test__discard_committed_image__ok_1

Ensure(api_test__discard_committed_image__ok_2) {
} // end of  api_test__discard_committed_image__ok_2

Ensure(api_test__discard_committed_file__nonexist) {
} // end of  api_test__discard_committed_file__nonexist

Ensure(api_test__discard_committed_file__denied) {
} // end of  api_test__discard_committed_file__denied

TestSuite *api_discard_committed_file_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__discard_committed_file__nonexist);
    add_test(suite, api_test__discard_committed_file__denied);
    add_test(suite, api_test__discard_committed_video__ok_1);
    add_test(suite, api_test__discard_committed_video__ok_2);
    add_test(suite, api_test__discard_committed_image__ok_1);
    add_test(suite, api_test__discard_committed_image__ok_2);
    return suite;
}
