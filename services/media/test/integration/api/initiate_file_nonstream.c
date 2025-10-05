#include <jansson.h>
#include "../test/integration/test.h"

#define ITEST_STREAM_HOST "localhost:8010"
#define ITEST_URL_PATTERN \
    "https://" ITEST_STREAM_HOST "/file?" API_QPARAM_LABEL__RESOURCE_ID "=%s&" API_QPARAM_LABEL__DOC_DETAIL \
    "=%s"

extern json_t  *itest_filefetch_avail_resource_lookup(uint8_t public_access, const char *fsubtype_in);
extern uint32_t itest_fileftech__get_approved_usr_id(json_t *upld_req);

typedef struct {
    json_t     *_upld_req; // for recording result of stream init
    const char *version_label;
    int         _expect_resp_code;
    uint32_t    usr_id;
} itest_usrarg_t;

static void itest_verify__nonstream_init(
    CURL *handle, test_setup_priv_t *privdata, void *_usr_arg
) { // expected response will be whatever transcoded single file
    CURLcode        res;
    long            actual_resp_code = 0;
    itest_usrarg_t *usr_arg = _usr_arg;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
}

static void _itest_nonstream_init__filefetch(itest_usrarg_t *usr_arg) {
    const char *resource_id = json_string_value(json_object_get(usr_arg->_upld_req, "resource_id"));
    char       *resource_id_escaped = curl_easy_escape(NULL, resource_id, strlen(resource_id));
    size_t url_sz = sizeof(ITEST_URL_PATTERN) + strlen(resource_id_escaped) + strlen(usr_arg->version_label);
    char   url[url_sz];
    size_t nwrite = sprintf(&url[0], ITEST_URL_PATTERN, resource_id_escaped, usr_arg->version_label);
    assert(nwrite < url_sz);
    json_t *header_kv_serials = json_array(), *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/octet-stream"));
    if (usr_arg->usr_id > 0) {
        const char *codename_list[1] = {NULL};
        add_auth_token_to_http_header(header_kv_serials, usr_arg->usr_id, codename_list, quota);
    }
    test_setup_pub_t setup_data = {
        .method = "GET", .verbose = 0, .url = &url[0], .headers = header_kv_serials
    };
    run_client_request(&setup_data, itest_verify__nonstream_init, usr_arg);
    free(resource_id_escaped);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of  _itest_nonstream_init__filefetch

#define RESOURCE_OWNER(__upld_req) json_integer_value(json_object_get(__upld_req, "usr_id"))
Ensure(api_test__filenonstream_init__resource_privileged) {
#define RUN_CODE(__upld_req, _usr_id, __expect_resp_code) \
    { \
        const char *ver_label = NULL; \
        if (__upld_req) { \
            json_t *_version_map = json_object_get(__upld_req, "_versions"); \
            json_t *tmp = NULL; \
            assert_that(_version_map, is_not_null); \
            assert_that(json_is_object(_version_map), is_true); \
            assert_that(json_object_size(_version_map), is_greater_than(0)); \
            json_object_foreach(_version_map, ver_label, tmp) { break; } \
            if (ver_label) { \
                itest_usrarg_t usr_arg = { \
                    ._upld_req = __upld_req, \
                    .usr_id = _usr_id, \
                    ._expect_resp_code = __expect_resp_code, \
                    .version_label = ver_label \
                }; \
                _itest_nonstream_init__filefetch(&usr_arg); \
            } else { \
                assert_that(ver_label, is_not_null); \
            } \
        } else { \
            assert_that(__upld_req, is_not_null); \
            return; \
        } \
    }
    json_t *upld_reqs[4] = {0};
    upld_reqs[0] = itest_filefetch_avail_resource_lookup(0, "jpg");
    RUN_CODE(upld_reqs[0], itest_fileftech__get_approved_usr_id(upld_reqs[0]), 200)
    RUN_CODE(upld_reqs[0], RESOURCE_OWNER(upld_reqs[0]), 200)
    upld_reqs[1] = itest_filefetch_avail_resource_lookup(0, "tiff");
    RUN_CODE(upld_reqs[1], RESOURCE_OWNER(upld_reqs[1]), 200)
    RUN_CODE(upld_reqs[1], itest_fileftech__get_approved_usr_id(upld_reqs[1]), 200)
    upld_reqs[2] = itest_filefetch_avail_resource_lookup(0, "png");
    RUN_CODE(upld_reqs[2], RESOURCE_OWNER(upld_reqs[2]), 200)
    RUN_CODE(upld_reqs[2], itest_fileftech__get_approved_usr_id(upld_reqs[2]), 200)
    upld_reqs[3] = itest_filefetch_avail_resource_lookup(0, "gif");
    RUN_CODE(upld_reqs[3], itest_fileftech__get_approved_usr_id(upld_reqs[3]), 200)
    RUN_CODE(upld_reqs[3], RESOURCE_OWNER(upld_reqs[3]), 200)
    for (int idx = 0; idx < 4; idx++) {
        RUN_CODE(upld_reqs[idx], 9998, 403)
        RUN_CODE(upld_reqs[idx], itest_fileftech__get_approved_usr_id(upld_reqs[idx]), 200)
    }
} // end of  api_test__filenonstream_init__resource_privileged

Ensure(api_test__filenonstream_init__resource_public) {
    json_t *upld_req = itest_filefetch_avail_resource_lookup(1, "tiff");
    RUN_CODE(upld_req, 0, 200)
    RUN_CODE(upld_req, 9987, 200)
    RUN_CODE(upld_req, 98765, 200)
    RUN_CODE(upld_req, 0, 200)
    RUN_CODE(upld_req, itest_fileftech__get_approved_usr_id(upld_req), 200)
    RUN_CODE(upld_req, RESOURCE_OWNER(upld_req), 200)
    RUN_CODE(upld_req, 0, 200)
} // end of  api_test__filenonstream_init__resource_public
#undef RUN_CODE
#undef RESOURCE_OWNER

Ensure(api_test__filenonstream_init__nonexist_res_id) {
    json_t *upld_req = json_object();
    json_object_set_new(upld_req, "resource_id", json_string("ffff0fff"));
    itest_usrarg_t usr_arg = {
        ._upld_req = upld_req, .usr_id = 9876, ._expect_resp_code = 404, .version_label = "ab"
    };
    _itest_nonstream_init__filefetch(&usr_arg);
    json_decref(upld_req);
} // end of  api_test__filenonstream_init__nonexist_res_id

TestSuite *api_file_nonstream_init_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__filenonstream_init__resource_privileged);
    add_test(suite, api_test__filenonstream_init__resource_public);
    add_test(suite, api_test__filenonstream_init__nonexist_res_id);
    return suite;
}
