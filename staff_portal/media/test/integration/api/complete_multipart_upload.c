#include "../test/integration/test.h"

static void test_verify__complete_multipart_upload(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 202;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code ));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    // const char *expect_upld_id = "1c037a57581e";
    const char *actual_job_id = json_string_value(json_object_get(resp_obj, "job_id"));
    assert_that(actual_job_id, is_not_null);
    // assert_that(expect_upld_id, is_equal_to_string(actual_upld_id));
    json_decref(resp_obj);
}

Ensure(api_complete_multipart_upload_test_accepted) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/complete");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    const char *req_body_raw = "{\"resource_id\":\"bMrI8f\", \"req_seq\":9801746}";
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=&req_body_raw[0], .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__complete_multipart_upload, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
}

TestSuite *api_complete_multipart_upload_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_complete_multipart_upload_test_accepted);
    return suite;
}
