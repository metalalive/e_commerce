#include "../test/integration/test.h"
#define MAX_NUM_ACTIVE_UPLOAD_REQUESTS 3

json_t  *_app_itest_active_upload_requests = NULL;

static void test_verify__initiate_multipart_upload_ok(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that((long)privdata->expect_resp_code, is_equal_to(actual_resp_code));
    // analyza response body
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    assert_that(resp_obj, is_not_equal_to(NULL));
    if(resp_obj) { // should return short-term token for upload request
        switch(privdata->expect_resp_code) {
            case 201:
                {
                    unsigned int req_seq = json_integer_value(json_object_get(resp_obj, "req_seq"));
                    unsigned int usr_id = json_integer_value(json_object_get(resp_obj, "usr_id"));
                    assert_that(req_seq, is_greater_than(0));
                    assert_that(usr_id, is_greater_than(0));
                    if(req_seq > 0 && usr_id > 0) {
                        json_t *item = json_object();
                        json_object_set(item, "usr_id",  json_integer(usr_id));
                        json_object_set(item, "req_seq", json_integer(req_seq));
                        json_array_append(_app_itest_active_upload_requests, item);
                    }
                }
                break;
            case 400:
                {
                    unsigned int num_active = json_integer_value(json_object_get(resp_obj, "num_active"));
                    unsigned int max_limit = json_integer_value(json_object_get(resp_obj, "max_limit"));
                    assert_that(max_limit, is_greater_than(0));
                    assert_that(num_active, is_greater_than(0));
                    assert_that(num_active, is_equal_to(max_limit));
                }
                break;
            case 503:
            default:
                break;
        }
    }
    json_decref(resp_obj);
} // end of test_verify__initiate_multipart_upload_ok


Ensure(api_test_initiate_multipart_upload_ok) {
    char url[128] = {0};
    size_t idx = 0;
    // the resource id client wants to claim, server may return auth failure if the user doesn't
    //  have access to modify the resource pointed by this ID
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
    const char *codename_list[3] = {"upload_files", "edit_file_access_control", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 125, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL}, .headers = header_kv_serials,
        .expect_resp_code = 201
    };
    run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
    { // clean previous auth token and create new one
        sleep(1);
        json_array_remove(header_kv_serials, (json_array_size(header_kv_serials) - 1));
        add_auth_token_to_http_header(header_kv_serials, 127, codename_list, quota);
        run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
    }
    json_array_remove(header_kv_serials, (json_array_size(header_kv_serials) - 1));
    add_auth_token_to_http_header(header_kv_serials, 130, codename_list, quota);
    setup_data.expect_resp_code = 201;
    for(idx = 0; idx < MAX_NUM_ACTIVE_UPLOAD_REQUESTS; idx++) {
        sleep(1);
        run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
    } // app server does NOT allow users to send several valid requests in one second
    setup_data.expect_resp_code = 400;
    sleep(1);
    for(idx = 0; idx < 6; idx++) {
        run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
    }
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_initiate_multipart_upload_ok


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
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    api_test_common_permission_check_fail(&setup_data);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_initiate_multipart_upload_insufficient_permission


TestSuite *api_initiate_multipart_upload_tests(void)
{
    _app_itest_active_upload_requests = json_array();
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_initiate_multipart_upload_auth_token_fail);
    add_test(suite, api_test_initiate_multipart_upload_insufficient_permission);
    add_test(suite, api_test_initiate_multipart_upload_ok);
    return suite;
}

void api_deinitiate_multipart_upload_tests(void) {
    if(_app_itest_active_upload_requests) {
        json_decref(_app_itest_active_upload_requests);
        _app_itest_active_upload_requests = NULL;
    }
}
