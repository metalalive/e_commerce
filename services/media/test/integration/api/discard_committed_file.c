#include "../test/integration/test.h"

#define ITEST_URL_PATTERN "/file?" API_QPARAM_LABEL__RESOURCE_ID "=%s"

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    json_t *upld_req;
    int     expect_resp_code;
} itest_usrarg_t;

static int _available_resource_lookup(json_t **upld_req, const char *ftype_in) {
    json_t *req = NULL, *resource_id_item = NULL;
    int     idx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, req) {
        resource_id_item = json_object_get(req, "resource_id");
        const char *ftype_saved = json_string_value(json_object_get(req, "type"));
        if (!ftype_saved) {
            resource_id_item = NULL;
            continue;
        }
        const char *_res_id = json_string_value(resource_id_item);
        uint8_t     type_matched = strncmp(ftype_saved, ftype_in, strlen(ftype_saved)) == 0;
        if (_res_id && type_matched) {
            break;
        } else {
            resource_id_item = NULL;
        }
    }
    if (req && resource_id_item) {
        *upld_req = req;
    } else {
        *upld_req = NULL;
        fprintf(
            stderr,
            "[itest][discard_committed_file] no more ressource"
            " with the type:%s \n",
            ftype_in
        );
    }
    return idx;
} // end of _available_resource_lookup

static void itest_verify__discarded_file_cb(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
    itest_usrarg_t *usrarg = _usr_arg;
    long            actual_resp_code = 0;
    CURLcode        res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usrarg->expect_resp_code));
}

static void _api_test__discard_committed_file__common(itest_usrarg_t *usrarg) {
    json_t     *_upld_req = usrarg->upld_req;
    uint32_t    res_owner_id = json_integer_value(json_object_get(_upld_req, "usr_id"));
    const char *resource_id = json_string_value(json_object_get(_upld_req, "resource_id"));
    size_t      url_sz = sizeof(ITEST_URL_PATTERN) + strlen(resource_id) + 1;
    char        url[url_sz];
    sprintf(&url[0], ITEST_URL_PATTERN, resource_id);
    const char *codename_list[3] = {"upload_files", "edit_file_access_control", NULL};
    json_t     *header_kv_serials = json_array();
    json_t     *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "DELETE",
        .verbose = 0, // no need to inspect curl verbose here
        .url_rel_ref = &url[0],
        .req_body = {.serial_txt = NULL, .src_filepath = NULL},
        .upload_filepaths = {.size = 0, .capacity = 0, .entries = NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, itest_verify__discarded_file_cb, usrarg);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of  _api_test__discard_committed_file__common

Ensure(api_test__discard_committed_video__ok) {
    json_t *upld_req = NULL;
    int     idx = _available_resource_lookup(&upld_req, "video");
    if (upld_req) {
        itest_usrarg_t _usrarg = {.upld_req = upld_req, .expect_resp_code = 204};
        _api_test__discard_committed_file__common(&_usrarg);
        _usrarg.expect_resp_code = 404;
        _api_test__discard_committed_file__common(&_usrarg);
        json_array_remove(_app_itest_active_upload_requests, idx);
    } else {
        assert_that(upld_req, is_not_null);
    }
}

Ensure(api_test__discard_committed_image__ok) {
    json_t *upld_req = NULL;
    int     idx = _available_resource_lookup(&upld_req, "image");
    if (upld_req) {
        itest_usrarg_t _usrarg = {.upld_req = upld_req, .expect_resp_code = 204};
        _api_test__discard_committed_file__common(&_usrarg);
        _usrarg.expect_resp_code = 404;
        _api_test__discard_committed_file__common(&_usrarg);
        json_array_remove(_app_itest_active_upload_requests, idx);
    } else {
        assert_that(upld_req, is_not_null);
    }
}

Ensure(api_test__discard_committed_file__nonexist) {
    json_t *upld_req = json_object();
    json_object_set_new(upld_req, "usr_id", json_integer(135));
    json_object_set_new(upld_req, "resource_id", json_string("1b2s93K4a4e2P9"));
    itest_usrarg_t _usrarg = {.upld_req = upld_req, .expect_resp_code = 404};
    _api_test__discard_committed_file__common(&_usrarg);
    json_decref(upld_req);
} // end of  api_test__discard_committed_file__nonexist

Ensure(api_test__discard_committed_file__denied) {
    json_t *ref_upld_req = NULL, *mock_upld_req = json_object();
    _available_resource_lookup(&ref_upld_req, "image");
    const char *existing_res_id = json_string_value(json_object_get(ref_upld_req, "resource_id"));
    json_object_set_new(mock_upld_req, "usr_id", json_integer(9876));
    json_object_set_new(mock_upld_req, "resource_id", json_string(existing_res_id));
    itest_usrarg_t _usrarg = {.upld_req = mock_upld_req, .expect_resp_code = 403};
    _api_test__discard_committed_file__common(&_usrarg);
    json_decref(mock_upld_req);
} // end of  api_test__discard_committed_file__denied

TestSuite *api_discard_committed_file_tests(void) { // TODO, test consistency between storage and database
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__discard_committed_file__nonexist);
    add_test(suite, api_test__discard_committed_file__denied);
    add_test(suite, api_test__discard_committed_video__ok);
    add_test(suite, api_test__discard_committed_image__ok);
    return suite;
}
