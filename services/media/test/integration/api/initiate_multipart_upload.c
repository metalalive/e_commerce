#include "../test/integration/test.h"
#define MAX_NUM_ACTIVE_UPLOAD_REQUESTS 3

json_t  *_app_itest_active_upload_requests = NULL;

static int _itest_num_original_files = 0;

static void test_verify__initiate_multipart_upload_ok(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to((long)privdata->expect_resp_code));
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
                        json_object_set_new(item, "usr_id",  json_integer(usr_id));
                        json_object_set_new(item, "req_seq", json_integer(req_seq));
                        json_object_set_new(item, "part", json_array());
                        json_object_set_new(item, "resource_id", json_null());
                        json_object_set_new(item, "async_job_ids", json_array()); // num of ongoing async jobs
                        json_object_set_new(item, "type"  , json_null()); // file type, will be referenced when transcoding files
                        json_object_set_new(item, "broken", json_null()); // is the file broken
                        json_array_append_new(_app_itest_active_upload_requests, item);
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
    size_t idx = 0, NUM_USERS = ITEST_NUM_UPLD_REQS__FOR_ERR_CHK + _itest_num_original_files;
    // the resource id client wants to claim, server may return auth failure if the user doesn't
    //  have access to modify the resource pointed by this ID
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
    const char *codename_list[3] = {"upload_files", "edit_file_access_control", NULL};
    uint32_t *usr_prof_ids = malloc(NUM_USERS * sizeof(uint32_t));
    for(idx = 0; idx < NUM_USERS; idx++)
        usr_prof_ids[idx] = 125 + idx;
    // assume one of the users sent multiple upload requests
    usr_prof_ids[ITEST_UPLD_REQ__SAME_USER__IDX_2] = usr_prof_ids[ITEST_UPLD_REQ__SAME_USER__IDX_1];
    json_t *header_kv_serials = json_array(), *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL}, .headers = header_kv_serials,
        .expect_resp_code = 201
    };
    for(idx = 0; idx < (NUM_USERS - 1); idx++) {
        add_auth_token_to_http_header(header_kv_serials, usr_prof_ids[idx], codename_list, quota);
        run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
        sleep(1); // delay to prevent users from sending too many requests at a time
        // then clean previous auth token and create new one
        json_array_remove(header_kv_serials, (json_array_size(header_kv_serials) - 1));
    } { // subcase : number of initiated updaate requests exceeded
        add_auth_token_to_http_header(header_kv_serials, usr_prof_ids[NUM_USERS - 1], codename_list, quota);
        setup_data.expect_resp_code = 201;
        for(idx = 0; idx < MAX_NUM_ACTIVE_UPLOAD_REQUESTS; idx++) {
            run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
            sleep(1);
        } // app server does NOT allow users to send several valid requests in one second
        setup_data.expect_resp_code = 400;
        for(idx = 0; idx < 6; idx++)
            run_client_request(&setup_data, test_verify__initiate_multipart_upload_ok, NULL);
    }
    json_decref(header_kv_serials);
    json_decref(quota);
    free(usr_prof_ids);
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


TestSuite *api_initiate_multipart_upload_tests(json_t *root_cfg)
{
    json_t  *fchunk_cfg = json_object_get(json_object_get(root_cfg, "test"), "file_chunk");
    json_t  *file_list = json_object_get(fchunk_cfg, "files");
    if(file_list && json_is_array(file_list)) {
        _itest_num_original_files = json_array_size(file_list);
        int required = ITEST_UPLD_REQ__SAME_USER__IDX_2 - ITEST_NUM_UPLD_REQS__FOR_ERR_CHK + 1;
        if(_itest_num_original_files < required)
            fprintf(stderr, "[itest][api][init_upld_req] line:%d, required:%d, _itest_num_original_files:%d \n",
                    __LINE__, required, _itest_num_original_files);
    } else {
        fprintf(stderr, "[itest][api][init_upld_req] line:%d, no test file list in config object \n",
                __LINE__);
    }
    _app_itest_active_upload_requests = json_array();
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_initiate_multipart_upload_auth_token_fail);
    add_test(suite, api_test_initiate_multipart_upload_insufficient_permission);
    add_test(suite, api_test_initiate_multipart_upload_ok);
    return suite;
} // end of  api_initiate_multipart_upload_tests

void api_deinitiate_multipart_upload_tests(void) {
    if(_app_itest_active_upload_requests) {
        json_decref(_app_itest_active_upload_requests);
        _app_itest_active_upload_requests = NULL;
    }
}
