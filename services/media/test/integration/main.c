#include "../test/integration/test.h"
#include "app_server.h"

static void test_verify__abort_multipart_upload(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_abort_multipart_upload_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?upload_id=%s", "localhost",
            8010, "/upload/multipart/abort", "1c037a57581e");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__abort_multipart_upload, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
}


static void test_verify__single_chunk_upload(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 201;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *item = NULL;
    int idx = 0;
    json_array_foreach(resp_obj, idx, item) {
        const char *actual_resource_id = json_string_value(json_object_get(item, "resource_id"));
        const char *actual_file_name   = json_string_value(json_object_get(item, "file_name"));
        assert_that(actual_resource_id, is_not_null);
        assert_that(actual_file_name  , is_not_null);
    }
    json_decref(resp_obj);
}

Ensure(api_single_chunk_upload_test) {
    // this API endpoint accept multiple files in one flight
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?resource_id=%s,%s", "localhost",
            8010, "/upload", "bMerI8f", "8fQwhBj");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .headers = header_kv_serials
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 2);
    setup_data.upload_filepaths.entries[0] = "media/test/integration/examples/test_file_chunk_0";
    setup_data.upload_filepaths.entries[1] = "media/test/integration/examples/test_file_chunk_1";
    setup_data.upload_filepaths.size = 2;
    run_client_request(&setup_data, test_verify__single_chunk_upload, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
}


static void test_verify__discard_ongoing_job(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_discard_ongoing_job_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/job", "1b2934ad4e2c9");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, 123, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__discard_ongoing_job, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
}

TestSuite *app_api_tests(json_t *root_cfg)
{
    TestSuite *suite = create_test_suite();
    add_suite(suite, api_initiate_multipart_upload_tests(root_cfg));
    add_suite(suite, api_upload_part_tests(root_cfg));
    add_suite(suite, api_complete_multipart_upload_tests());
    add_suite(suite, api_file_acl_tests());
    add_suite(suite, api_start_transcoding_file_tests());
    add_suite(suite, api_monitor_job_progress_tests());
    add_suite(suite, api_file_streaming_init_tests());
    add_suite(suite, api_file_nonstream_init_tests());
    add_suite(suite, api_file_stream_seek_elm_tests());
    add_suite(suite, api_start_transcoding_file_v2_tests());
    add_suite(suite, api_monitor_job_progress_tests());
    add_suite(suite, api_discard_committed_file_tests());
    add_test(suite, api_abort_multipart_upload_test);
    add_test(suite, api_single_chunk_upload_test);
    add_test(suite, api_discard_ongoing_job_test);
    return suite;
}

static void run_app_server(void *data) {
    test_init_app_data_t *data1 = (test_init_app_data_t *)data;
    start_application(data1->cfg_file_path, data1->exe_path);
} // end of run_app_server()


int main(int argc, char **argv) {
    assert(argc > 1);
    test_init_app_data_t  init_app_data = {
        .cfg_file_path = argv[argc - 1], // "./media/settings/test.json",
        .exe_path = argv[argc - 2] // "./media/build/integration_test.out"
    };
    int result = 0;
    uv_thread_t app_tid = 0;
    json_error_t  j_err = {0};
    json_t  *root_cfg = json_load_file( init_app_data.cfg_file_path, (size_t)0, &j_err);
    if (!json_is_object(root_cfg)) {
        fprintf(stderr, "[test] failed to parse config file\n");
        goto done;
    }
    result = uv_thread_create( &app_tid, run_app_server, (void *)&init_app_data );
    assert(result == 0);
    assert(app_tid > 0);
    TestSuite *suite = create_named_test_suite("media_app_integration_test");
    TestReporter *reporter = create_text_reporter();
    add_suite(suite, app_api_tests(root_cfg));
    curl_global_init(CURL_GLOBAL_DEFAULT);
    init_mock_auth_server("./tmp/cache/test/jwks/media_test_jwks_pubkey_XXXXXX");
    while(1) {
        if(app_server_ready()) {
            break;
        } else {
            result = pthread_tryjoin_np(app_tid, NULL);
            if(result == 0) {
                fprintf(stderr, "[test] app server thread terminated due to some error\n");
                goto done;
            } // unexpected early thread terminated
            sleep(3);
        }
    }
    fprintf(stdout, "[test] curl version : %s \n", curl_version());
    fprintf(stdout, "[test] app server is ready, start integration test cases ...\n");
    // const char *test_name = argv[argc - 1];
    // result = run_single_test(suite, test_name, reporter);
    result = run_test_suite(suite, reporter);
    pthread_kill(app_tid, SIGTERM);
    result = ETIMEDOUT;
    for (int idx = 0; (result != 0) && (idx < 10); idx++) {
        result = pthread_tryjoin_np(app_tid, NULL);
        if(result != 0)
            sleep(1);
    }
    if(result != 0) {
        fprintf(stderr, "[test] app server failed to terminate, error:%d \n", result);
        pthread_kill(app_tid, SIGTERM); // forced shutdown
    }
done:
    api_deinitiate_multipart_upload_tests();
    deinit_mock_auth_server();
    curl_global_cleanup();
    destroy_test_suite(suite);
    destroy_reporter(reporter);
    if (!root_cfg)
        json_decref(root_cfg);
    return result;
} // end of main()
