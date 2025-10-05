#include <jansson.h>
#include "../test/integration/test.h"

#define URL_PATTERN "https://localhost:8010/job?id=%s"

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    json_t *job_item; // for recording specific job progress / error
    int     expect_resp_code;
} itest_usrarg_t;

//  __attribute__((optimize("O0")))
static void itest_verify__monitor_job_progress(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
    itest_usrarg_t *usr_arg = (itest_usrarg_t *)_usr_arg;
    CURLcode        res;
    long            expect_resp_code = usr_arg->expect_resp_code;
    long            actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_not_equal_to(0));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
#if 1
    if (actual_resp_code != expect_resp_code)
        fprintf(stderr, "[itest][monitor_job_progress] line:%d, response mismatch \n", __LINE__);
#endif
    if (actual_resp_code > 0 && actual_resp_code < 400) { // ok
        json_t          *fn_verify_item = json_object_get(usr_arg->job_item, "fn_verify_job");
        test_verify_cb_t custom_fn = (test_verify_cb_t)json_integer_value(fn_verify_item);
        assert_that(custom_fn, is_not_null);
        if (custom_fn)
            custom_fn(handle, privdata, usr_arg->job_item);
    }
} // end of itest_verify__monitor_job_progress

static int api_test_monitor_job_progress__update(uint32_t usr_id, json_t *job_item, int expect_resp_code) {
    uint8_t done_flag = (uint8_t)json_boolean_value(json_object_get(job_item, "done"));
    if (!done_flag) {
        const char *job_id = json_string_value(json_object_get(job_item, "job_id"));
        size_t      URL_TOT_SZ = sizeof(URL_PATTERN) + strlen(job_id) + 1;
        char        url[URL_TOT_SZ];
        size_t      nwrite = snprintf(&url[0], URL_TOT_SZ, URL_PATTERN, job_id);
        url[nwrite++] = 0;
        assert_that((URL_TOT_SZ >= nwrite), is_equal_to(1));
        const char *codename_list[2] = {"upload_files", NULL};
        json_t     *header_kv_serials = json_array();
        json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
        json_t *quota = json_array();
        add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
        itest_usrarg_t   usr_arg = {.job_item = job_item, .expect_resp_code = expect_resp_code};
        test_setup_pub_t setup_data = {
            .method = "GET",
            .verbose = 0,
            .url = &url[0],
            .http_timeout_sec = 7,
            .upload_filepaths = {.size = 0, .capacity = 0, .entries = NULL},
            .headers = header_kv_serials
        };
        run_client_request(&setup_data, itest_verify__monitor_job_progress, &usr_arg);
        done_flag = (uint8_t)json_boolean_value(json_object_get(job_item, "done"));
        json_decref(header_kv_serials);
        json_decref(quota);
    } // reduce unecessary assertions stored in cgreen internal queue (implemented using Linux pipe)
    int still_processing = done_flag ? 0 : 1;
    return still_processing;
} // end of api_test_monitor_job_progress__update

Ensure(api_test__monitor_job_progress__nonexist_id) {
    json_t  *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    uint32_t usr_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    json_t  *mock_job_item = json_object();
#define JOB_ID "non_exist_id_123456abc"
    json_object_set_new(mock_job_item, "job_id", json_string(JOB_ID));
    api_test_monitor_job_progress__update(usr_id, mock_job_item, 400);
    json_decref(mock_job_item);
#undef JOB_ID
} // end of api_test__monitor_job_progress__nonexist_id

Ensure(api_test__monitor_job_progress__ok) {
    json_t *jobs_flatten = json_array();
    json_t *upld_req = NULL, *async_job_ids_item = NULL, *job_item = NULL;
    int     idx = 0, jdx = 0, num_processing = 0;
    sleep(80); // TODO, wait util first progress update are sent back
    do {
        num_processing = 0; // reset
        json_array_foreach(_app_itest_active_upload_requests, idx, upld_req) {
            uint32_t usr_id = json_integer_value(json_object_get(upld_req, "usr_id"));
            async_job_ids_item = json_object_get(upld_req, "async_job_ids");
            json_array_foreach(async_job_ids_item, jdx, job_item) {
                num_processing += api_test_monitor_job_progress__update(usr_id, job_item, 200);
            } // end of iteeration on processing jobs
        } // end of iteeration on upload request
        if (num_processing > 0)
            sleep(15);
    } while (num_processing > 0);
    json_decref(jobs_flatten);
} // end of api_test__monitor_job_progress__ok

TestSuite *api_monitor_job_progress_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__monitor_job_progress__nonexist_id);
    add_test(suite, api_test__monitor_job_progress__ok);
    return suite;
}
#undef URL_PATTERN
