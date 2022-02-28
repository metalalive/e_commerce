#include "../test/integration/test.h"

static void test_verify__initiate_multipart_upload_ok(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    // analyza response body
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    assert_that(resp_obj, is_not_equal_to(NULL));
    if(resp_obj) { // should return short-term token for upload request
        const char *access_token = json_string_value(json_object_get(resp_obj, "upld_id"));
        assert_that(access_token, is_not_null);
    }
    json_decref(resp_obj);
}


Ensure(api_test_initiate_multipart_upload_ok) {
    char url[128] = {0};
    // the resource id client wants to claim, server may return auth failure if the user doesn't
    //  have access to modify the resource pointed by this ID
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
    const char *codename_list[3] = {"upload_files", "edit_file_access_control", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    add_auth_token_to_http_header(header_kv_serials, codename_list);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok);
    json_decref(header_kv_serials);
}


Ensure(api_test_initiate_multipart_upload_auth_token_fail) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
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
} // end of api_test_initiate_multipart_upload_auth_token_fail


Ensure(api_test_initiate_multipart_upload_insufficient_permission) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
    const char *codename_list[3] = {"can_do_sth_else", "can_do_that", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    add_auth_token_to_http_header(header_kv_serials, codename_list);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    api_test_common_permission_check_fail(&setup_data);
    json_decref(header_kv_serials);
} // end of api_test_initiate_multipart_upload_insufficient_permission


TestSuite *api_initiate_multipart_upload_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_initiate_multipart_upload_auth_token_fail);
    add_test(suite, api_test_initiate_multipart_upload_insufficient_permission);
    add_test(suite, api_test_initiate_multipart_upload_ok);
    return suite;
}
