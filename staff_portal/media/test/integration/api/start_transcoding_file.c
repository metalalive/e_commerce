#include "../test/integration/test.h"
extern json_t *_app_itest_active_upload_requests;

static void test_verify__start_transcoding_file(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 202;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *actual_job_id = json_string_value(json_object_get(resp_obj, "job"));
    assert_that(actual_job_id, is_not_null);
    json_decref(resp_obj);
}

Ensure(api_transcoding_file_test_invalid_format) {
} // end of api_transcoding_file_test_invalid_format

Ensure(api_transcoding_file_test_accepted) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/file/transcode");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath="./media/test/integration/examples/transcode_req_body.json"},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__start_transcoding_file, NULL);
    { // TODO:wait until connection timeout, consume API again, the app server should reconnect the AMQP broker
        //// app_cfg_t *acfg = app_get_global_cfg();
        //// arpc_cfg_t *rpc_cfg = &acfg->rpc.entries[0];
        //// size_t delay_secs = 4 * rpc_cfg->attributes.timeout_secs;
        //// sleep(delay_secs);
        //// setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_2.json";
        //// run_client_request(&setup_data, test_verify__complete_multipart_upload, NULL);
    }
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_transcoding_file_test_accepted

TestSuite *api_start_transcoding_file_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_transcoding_file_test_invalid_format);
    add_test(suite, api_transcoding_file_test_accepted);
    return suite;
}
