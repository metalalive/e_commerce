#include "../test/integration/test.h"

#define  EXPECT_RESOURCE_ID  "bMrI8f"
#define  SECONDARY_RESOURCE_ID  "';t1kT0'"

typedef struct {
    uint32_t expect_resp_code;
    const char *err_field;
    const char *err_msg;
    json_t *upld_req_ref;
} usrarg_t;

extern json_t *_app_itest_active_upload_requests;

static void test_verify__app_server_response(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    usrarg_t  *_usr_arg = (usrarg_t *)usr_arg;
    CURLcode res;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(_usr_arg->expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    if(actual_resp_code < 300) {
        const char *valid_resource_id = json_string_value(json_object_get(resp_obj, "resource_id"));
        // assert_that(expect_upld_id, is_equal_to_string(actual_upld_id));
        json_t *stats = _usr_arg->upld_req_ref;
        assert_that(valid_resource_id , is_not_null);
        if(valid_resource_id) {
            json_object_set_new(stats, "resource_id", json_string(valid_resource_id));
        }
    } else {
        const char *actual_err_msg = json_string_value(json_object_get(resp_obj, _usr_arg->err_field));
        if(_usr_arg->err_msg) {
            assert_that(actual_err_msg, is_equal_to_string(_usr_arg->err_msg));
        }
    }
    json_decref(resp_obj);
} // end of test_verify__app_server_response


#define  MAX_BYTES_REQ_BODY  128
static void _api_commit_upload_req__success_common (json_t *upld_req, const char *resource_id,
        uint32_t expect_resp_code, const char *err_field, const char *err_msg)
{
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/complete");
    json_t *header_kv_serials = json_array();
    json_t *quota = json_array();
    {
        uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id" ));
        const char *codename_list[2] = {"upload_files", NULL};
        json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
        add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    }
    const char req_body_raw[MAX_BYTES_REQ_BODY] = {0};
    {
        uint32_t req_seq = json_integer_value(json_object_get(upld_req, "req_seq"));
        json_t *req_body = json_object();
        json_object_set_new(req_body, "resource_id", json_string(resource_id));
        json_object_set_new(req_body, "req_seq", json_integer(req_seq));
        size_t nwrite = json_dumpb((const json_t *)req_body, &req_body_raw[0],  MAX_BYTES_REQ_BODY, JSON_COMPACT);
        json_decref(req_body);
        assert(MAX_BYTES_REQ_BODY > nwrite);
    }
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url = &url[0], .headers = header_kv_serials,
        .req_body = {.serial_txt=&req_body_raw[0], .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
    };
    usrarg_t  cb_arg = {.upld_req_ref=upld_req, .expect_resp_code=expect_resp_code,
        .err_field=err_field, .err_msg=err_msg };
    run_client_request(&setup_data, test_verify__app_server_response, (void *)&cb_arg);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of _api_commit_upload_req__success_common
#undef  MAX_BYTES_REQ_BODY


Ensure(api_commit_upload_req__missing_auth_token) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/complete");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    api_test_common_auth_token_fail(&setup_data);
    json_decref(header_kv_serials);
} // end of api_commit_upload_req__missing_auth_token


Ensure(api_commit_upload_req__invalid_resource_id) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, "' OR 1;'", 400, "resource_id", "invalid format");
    }
} // end of api_commit_upload_req__invalid_resource_id

Ensure(api_commit_upload_req__nonexistent_req) {
    json_t *upld_req_src = json_array_get(_app_itest_active_upload_requests, 0);
    json_t *upld_req_dst = json_object();
    {
        uint32_t usr_id  = json_integer_value(json_object_get(upld_req_src, "usr_id"));
        uint32_t req_seq = 0xffffffff;
        json_object_set_new(upld_req_dst, "usr_id" , json_integer(usr_id ));
        json_object_set_new(upld_req_dst, "req_seq", json_integer(req_seq));
    }
    _api_commit_upload_req__success_common(upld_req_dst, EXPECT_RESOURCE_ID, 400,
            "req_seq", "request not exists");
    json_decref(upld_req_dst);
} // end of api_commit_upload_req__nonexistent_req


Ensure(api_commit_upload_req__incomplete_chunks) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 1);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, EXPECT_RESOURCE_ID, 400,
                "req_seq", "part numbers of file chunks are not adjacent");
    }
} // end of api_commit_upload_req__incomplete_chunks


Ensure(api_commit_upload_req__add_new_resource_id) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 2);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, EXPECT_RESOURCE_ID, 201, NULL, NULL);
        _api_commit_upload_req__success_common(upld_req, SECONDARY_RESOURCE_ID, 400, "req_seq", "request not exists");
    } // the same upload request cannot be applied to `differeent resource`
} // end of api_commit_upload_req__add_new_resource_id


Ensure(api_commit_upload_req__resource_id_not_allowed) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 3);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, EXPECT_RESOURCE_ID, 403,
                "resource_id", "NOT allowed to use the ID");
    } // the resource id was claimed by another user, different users are NOT allowed to use it
} // end of api_commit_upload_req__resource_id_not_allowed


Ensure(api_commit_upload_req__use_existing_resource_id) {
    // pre-condition: req-seq at idx=2 and idx=4 should be generated by the same user
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 4);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, EXPECT_RESOURCE_ID, 200, NULL, NULL);
    }
    upld_req = json_array_get(_app_itest_active_upload_requests, 2);
    assert_that(upld_req, is_not_equal_to(NULL));
    if(upld_req) {
        _api_commit_upload_req__success_common(upld_req, SECONDARY_RESOURCE_ID, 201, NULL, NULL);
    } // at this point, req-seq at idx=2 returns to `uncommitted state`, so it can be committed to another `resource`
} // end of api_commit_upload_req__use_existing_resource_id


TestSuite *api_complete_multipart_upload_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_commit_upload_req__missing_auth_token);
    add_test(suite, api_commit_upload_req__invalid_resource_id);
    add_test(suite, api_commit_upload_req__nonexistent_req);
    add_test(suite, api_commit_upload_req__incomplete_chunks);
    add_test(suite, api_commit_upload_req__add_new_resource_id);
    add_test(suite, api_commit_upload_req__resource_id_not_allowed);
    add_test(suite, api_commit_upload_req__use_existing_resource_id);
    return suite;
}
