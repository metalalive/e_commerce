#include "../test/integration/test.h"


static void test_verify__upload_part_ok(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    int expect_part = 3;
    int actual_part = (int)json_integer_value(json_object_get(resp_obj, "part"));
    assert_that(expect_part, is_equal_to(actual_part));
    json_decref(resp_obj);
}


Ensure(api_test_upload_part_ok) {
    char url[128] = {0};
    int expect_part = 3;
    sprintf(&url[0], "https://%s:%d%s?upload_id=%s&part=%d", "localhost",
            8010, "/upload/multipart/part", "1c037a57581e", expect_part);
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0, .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .headers = header_kv_serials
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.entries[0] = "./tmp/test_file_chunk_0";
    setup_data.upload_filepaths.size = 1;
    run_client_request(&setup_data, test_verify__upload_part_ok);
    json_decref(header_kv_serials);
}


Ensure(api_test_upload_part_auth_token_fail) {
    char url[128] = {0};
    int expect_part = 3;
    sprintf(&url[0], "https://%s:%d%s?upload_id=%s&part=%d", "localhost",
            8010, "/upload/multipart/part", "1c037a57581e", expect_part);
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    api_test_common_auth_token_fail(&setup_data);
    json_decref(header_kv_serials);
} // end of api_test_upload_part_auth_token_fail


TestSuite *api_upload_part_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_upload_part_auth_token_fail);
    add_test(suite, api_test_upload_part_ok);
    return suite;
}
