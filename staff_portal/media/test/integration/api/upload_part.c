#include "views.h"
#include "../test/integration/test.h"

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    uint32_t resp_code;
    uint32_t part;
    const char *f_chksum;
    const char *filepath;
} upldpart_usrarg_t;

static void test_verify__upload_part_ok(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    upldpart_usrarg_t  *_usr_arg = (upldpart_usrarg_t *)usr_arg;
    CURLcode res;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code , is_equal_to(_usr_arg->resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    int actual_part = (int)json_integer_value(json_object_get(resp_obj, "part"));
    if(actual_part > 0) {
        assert_that(actual_part, is_equal_to(_usr_arg->part));
    }
    const char *checksum = json_string_value(json_object_get(resp_obj, "checksum"));
    if(checksum) {
        assert_that(checksum, is_equal_to_string(_usr_arg->f_chksum));
    }
    json_decref(resp_obj);
} // end of test_verify__upload_part_ok

#define EXPECT_PART  3
#define CHUNK_FILE_PATH  "./tmp/test_file_chunk_0"
#define CHUNK_FILE_CHKSUM  "c618d7709f63b3e2cc11f799f3d1a7edb53b5bc0"
Ensure(api_test_upload_part_ok) {
    char url[128] = {0};
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(!upld_req) { return; }
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id" ));
    uint32_t req_seq = json_integer_value(json_object_get(upld_req, "req_seq"));
    sprintf(&url[0], "https://%s:%d%s?req_seq=%d&part=%d", "localhost",
            8010, "/upload/multipart/part", req_seq, EXPECT_PART);
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    {
        json_t *item = json_object();
        json_object_set(item, "app_code", json_integer(APP_CODE));
        json_object_set(item, "mat_code", json_integer(QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER));
        json_object_set(item, "maxnum", json_integer(200));
        json_array_append(quota, item);
    }
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0, .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .headers = header_kv_serials
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.entries[0] = CHUNK_FILE_PATH;
    setup_data.upload_filepaths.size = 1;
    upldpart_usrarg_t  cb_arg = {.resp_code=200, .part=EXPECT_PART, .f_chksum=CHUNK_FILE_CHKSUM};
    run_client_request(&setup_data, test_verify__upload_part_ok, (void *)&cb_arg);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_upload_part_ok
#undef EXPECT_PART
#undef CHUNK_FILE_PATH
#undef CHUNK_FILE_CHKSUM


#define EXPECT_PART  3
Ensure(api_test_upload_part__missing_auth_token) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?req_seq=%s&part=%d", "localhost",
            8010, "/upload/multipart/part", "1c037a57581e", EXPECT_PART);
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
} // end of api_test_upload_part__missing_auth_token


#define EXPECT_PART  3
static void  test_verify__upload_part_uri_error(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 400;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code , is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *err_msg = json_string_value(json_object_get(resp_obj, "req_seq"));
    assert_that(err_msg, is_equal_to_string("missing request ID"));
    json_decref(resp_obj);
} // end of test_verify__upload_part_uri_error

Ensure(api_test_upload_part__uri_error) {
    char url[128] = {0};
    uint32_t usr_id  = 123;
    uint32_t req_seq = 0xffffff; // invalid upload request
    sprintf(&url[0], "https://%s:%d%s?req_id=%d&part=%d", "localhost",
            8010, "/upload/multipart/part", req_seq, EXPECT_PART);
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0, .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    run_client_request(&setup_data, test_verify__upload_part_uri_error, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_upload_part__uri_error


static void  test_verify__upload_part_invalid_req(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 400;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code , is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *err_msg = json_string_value(json_object_get(resp_obj, "req_seq"));
    assert_that(err_msg, is_equal_to_string("request not exists"));
    json_decref(resp_obj);
} // end of test_verify__upload_part_invalid_req

Ensure(api_test_upload_part__invalid_req) {
    char url[128] = {0};
    uint32_t usr_id  = 123;
    uint32_t req_seq = 0xffffff; // invalid upload request
    sprintf(&url[0], "https://%s:%d%s?req_seq=%d&part=%d", "localhost",
            8010, "/upload/multipart/part", req_seq, EXPECT_PART);
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    {
        json_t *item = json_object();
        json_object_set(item, "app_code", json_integer(APP_CODE));
        json_object_set(item, "mat_code", json_integer(QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER));
        json_object_set(item, "maxnum", json_integer(1));
        json_array_append(quota, item);
    }
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0, .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    run_client_request(&setup_data, test_verify__upload_part_invalid_req, NULL);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_upload_part__invalid_req


#define  NUM_PARTS  3
#define  CHUNK_FILE_PATH_1    "./tmp/test_file_chunk_0"
#define  CHUNK_FILE_PATH_2    "./tmp/test_file_chunk_1"
#define  CHUNK_FILE_PATH_3    "./tmp/test_file_chunk_2"
#define  CHUNK_FILE_CHKSUM_1  "c618d7709f63b3e2cc11f799f3d1a7edb53b5bc0"
#define  CHUNK_FILE_CHKSUM_2  "95e2ea5f466fa1bf99e32781f9c2a273f005adb4"
#define  CHUNK_FILE_CHKSUM_3  "5a1a019e84295cd75f7d78752650ac9d5dd54432"
Ensure(api_test_upload_part__quota_exceed) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 1);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(!upld_req) { return; }
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id" ));
    uint32_t req_seq = json_integer_value(json_object_get(upld_req, "req_seq"));
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    {
        size_t  max_upld_kbytes = 2;
        json_t *item = json_object();
        json_object_set(item, "app_code", json_integer(APP_CODE));
        json_object_set(item, "mat_code", json_integer(QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER));
        json_object_set(item, "maxnum", json_integer(max_upld_kbytes));
        json_array_append(quota, item);
    }
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.size = 1;
    upldpart_usrarg_t  cb_args[NUM_PARTS] = {
        {.resp_code=200, .part=1, .f_chksum=CHUNK_FILE_CHKSUM_1, .filepath=CHUNK_FILE_PATH_1},
        {.resp_code=200, .part=2, .f_chksum=CHUNK_FILE_CHKSUM_2, .filepath=CHUNK_FILE_PATH_2},
        {.resp_code=403, .part=3, .f_chksum=CHUNK_FILE_CHKSUM_3, .filepath=CHUNK_FILE_PATH_3},
    };
    for (size_t idx = 0; idx < NUM_PARTS; idx++) {
        char url[128] = {0};
        sprintf(&url[0], "https://%s:%d%s?req_seq=%d&part=%d", "localhost", 8010, "/upload/multipart/part",
                req_seq, cb_args[idx].part );
        setup_data.url = &url[0];
        setup_data.upload_filepaths.entries[0] = cb_args[idx].filepath;
        run_client_request(&setup_data, test_verify__upload_part_ok, (void *)&cb_args[idx]);
        sleep(1);
    } // end of loop
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_upload_part__quota_exceed
#undef  CHUNK_FILE_PATH_1  
#undef  CHUNK_FILE_PATH_2  
#undef  CHUNK_FILE_PATH_3  
#undef  CHUNK_FILE_CHKSUM_1
#undef  CHUNK_FILE_CHKSUM_2
#undef  CHUNK_FILE_CHKSUM_3
#undef  NUM_PARTS


TestSuite *api_upload_part_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_upload_part__missing_auth_token);
    add_test(suite, api_test_upload_part_ok);
    add_test(suite, api_test_upload_part__uri_error);
    add_test(suite, api_test_upload_part__invalid_req);
    add_test(suite, api_test_upload_part__quota_exceed);
    return suite;
}
