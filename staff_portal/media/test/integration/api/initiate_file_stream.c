#include <jansson.h>

#include "transcoder/video/hls.h"
#include "../test/integration/test.h"

#define  ITEST_STREAM_HOST  "localhost:8010"
#define  ITEST_URL_PATTERN  "https://" ITEST_STREAM_HOST "/file/stream/init?" API_QPARAM_LABEL__RESOURCE_ID "=%s"

typedef struct {
    json_t  *_upld_req; // for recording result of stream init
    int  _expect_resp_code;
    const char *expect_st_type;
    const char *expect_st_host;
    const char *expect_doc_id;
    const char *expect_detail_keyword;
} itest_usrarg_t;

extern  json_t * itest_filefetch_avail_resource_lookup(uint8_t public_access, const char *fsubtype_in);
extern uint32_t  itest_fileftech__get_approved_usr_id (json_t *upld_req);

static void test_verify__filestream_init (CURL *handle, test_setup_priv_t *privdata, void *_usr_arg)
{
    // example response
    // {"type":"hls","host":"localhost:8912","doc_id":"whatever-doc-id-gen-by server", "d_detail":"keyword-for-specific-format"}
    CURLcode res;
    long actual_resp_code = 0;
    itest_usrarg_t  *usr_arg = _usr_arg; 
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
    if(actual_resp_code <= 0 || actual_resp_code >= 400)
        return;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t  *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const  char *actual_st_type = json_string_value(json_object_get(resp_obj, "type"));
    const  char *actual_st_host = json_string_value(json_object_get(resp_obj, "host"));
    const  char *actual_doc_id = json_string_value(json_object_get(resp_obj, API_QPARAM_LABEL__STREAM_DOC_ID));
    const  char *actual_detail_keyword = json_string_value(json_object_get(resp_obj, API_QPARAM_LABEL__DOC_DETAIL));
    assert_that(actual_st_type, is_equal_to_string(usr_arg->expect_st_type));
    assert_that(actual_st_host, is_equal_to_string(usr_arg->expect_st_host));
    assert_that(actual_doc_id, is_not_null);
    assert_that(actual_detail_keyword, is_not_null);
    if(usr_arg->expect_doc_id)
        assert_that(actual_doc_id, is_equal_to_string(usr_arg->expect_doc_id));
    if(usr_arg->expect_detail_keyword)
        assert_that(actual_detail_keyword, is_equal_to_string(usr_arg->expect_detail_keyword));
    json_object_set_new(resp_obj, "_priv_data", json_object());
    json_object_set_new(usr_arg->_upld_req, "streaming", resp_obj);
} // end of  test_verify__filestream_init


static  void  _itest_fstream_init__send_request (itest_usrarg_t *usr_arg, uint32_t usr_id)
{
    const char *resource_id = json_string_value(json_object_get(usr_arg->_upld_req, "resource_id"));
    char *resource_id_escaped = curl_easy_escape(NULL, resource_id, strlen(resource_id));
    size_t url_sz = sizeof(ITEST_URL_PATTERN) + strlen(resource_id_escaped);
    char url[url_sz];
    size_t  nwrite = sprintf(&url[0], ITEST_URL_PATTERN, resource_id_escaped);
    assert(nwrite < url_sz);
    json_t *header_kv_serials = json_array(), *quota = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    if(usr_id > 0) {
        const char *codename_list[1] = {NULL};
        add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    }
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0], .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL}, .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__filestream_init, usr_arg);
    free(resource_id_escaped);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of _itest_fstream_init__send_request


Ensure(api_test__filestream_init__hls_auth_usr)
{
#define  RESOURCE_OWNER(__upld_req)  json_integer_value(json_object_get(__upld_req, "usr_id"))
    json_t *upld_reqs[3] = {0};
#define  RUN_CODE(__upld_req, _usr_id, __expect_resp_code) { \
    if(__upld_req) { \
        itest_usrarg_t  usr_arg = {._upld_req=__upld_req, ._expect_resp_code=__expect_resp_code, \
            .expect_st_host=ITEST_STREAM_HOST, .expect_detail_keyword=HLS_MASTER_PLAYLIST_FILENAME, \
            .expect_st_type="hls" }; \
        json_t *st_prev_resp = json_object_get(__upld_req, "streaming"); \
        if(st_prev_resp) { \
            usr_arg.expect_doc_id = json_string_value(json_object_get( \
                        st_prev_resp, API_QPARAM_LABEL__STREAM_DOC_ID)); \
        } \
        _itest_fstream_init__send_request(&usr_arg, _usr_id); \
    } else { \
        fprintf(stderr, "[itest][init_stream] line:%d, failed to find transcoded file \r\n", __LINE__); \
        assert_that(__upld_req, is_not_null); \
        return; \
    } \
}
    upld_reqs[0] = itest_filefetch_avail_resource_lookup(0, "mp4");
    RUN_CODE(upld_reqs[0], itest_fileftech__get_approved_usr_id(upld_reqs[0]), 200)
    RUN_CODE(upld_reqs[0], RESOURCE_OWNER(upld_reqs[0]),  200)
    RUN_CODE(upld_reqs[0], itest_fileftech__get_approved_usr_id(upld_reqs[0]), 200)
    upld_reqs[1] = itest_filefetch_avail_resource_lookup(0, "mp4");
    RUN_CODE(upld_reqs[1], RESOURCE_OWNER(upld_reqs[1]),  200)
    RUN_CODE(upld_reqs[1], itest_fileftech__get_approved_usr_id(upld_reqs[1]), 200)
    RUN_CODE(upld_reqs[1], 9987, 403)
    RUN_CODE(upld_reqs[1], itest_fileftech__get_approved_usr_id(upld_reqs[1]), 200)
    RUN_CODE(upld_reqs[1], RESOURCE_OWNER(upld_reqs[1]),  200)
    upld_reqs[2] = itest_filefetch_avail_resource_lookup(0, "mp4");
    RUN_CODE(upld_reqs[2], itest_fileftech__get_approved_usr_id(upld_reqs[2]), 200)
    RUN_CODE(upld_reqs[2], RESOURCE_OWNER(upld_reqs[2]), 200)
    RUN_CODE(upld_reqs[2], 98765, 403)
    // --------
    RUN_CODE(upld_reqs[0], itest_fileftech__get_approved_usr_id(upld_reqs[0]), 200)
    RUN_CODE(upld_reqs[1], itest_fileftech__get_approved_usr_id(upld_reqs[1]), 200)
    RUN_CODE(upld_reqs[2], itest_fileftech__get_approved_usr_id(upld_reqs[2]), 200)
    RUN_CODE(upld_reqs[0], itest_fileftech__get_approved_usr_id(upld_reqs[0]), 200)
    RUN_CODE(upld_reqs[1], itest_fileftech__get_approved_usr_id(upld_reqs[1]), 200)
    RUN_CODE(upld_reqs[2], itest_fileftech__get_approved_usr_id(upld_reqs[2]), 200)
} // end of  api_test__filestream_init__hls_auth_usr


Ensure(api_test__filestream_init__hls_public) 
{
    json_t *upld_req = itest_filefetch_avail_resource_lookup(1, "mp4");
    RUN_CODE(upld_req, 0,   200)
    RUN_CODE(upld_req, 9987, 200)
    RUN_CODE(upld_req, 98765, 200)
    RUN_CODE(upld_req, 0,   200)
    RUN_CODE(upld_req, itest_fileftech__get_approved_usr_id(upld_req), 200)
    RUN_CODE(upld_req, RESOURCE_OWNER(upld_req),  200)
    RUN_CODE(upld_req, 0,   200)
#undef  RUN_CODE
#undef  RESOURCE_OWNER
} // end of  api_test__filestream_init__hls_public


Ensure(api_test__filestream_init__hls_rotate_keyfile)
{
    // TODO, re-init streaming, require prior knowledge of previously initialized streams
    //  (keep the upload requests in previous test case `api_test__filestream_init__hls_ok`)
} // end of  api_test__filestream_init__hls_rotate_keyfile


Ensure(api_test__filestream_init__nonexist_id)
{
    json_t *upld_req = json_object();
    const char *resource_id = "e234s678";
    json_object_set_new(upld_req, "resource_id", json_string(resource_id));
    itest_usrarg_t  usr_arg = {._expect_resp_code=404, ._upld_req=upld_req};
    _itest_fstream_init__send_request(&usr_arg, 9876);
    json_decref(upld_req);
} // end of api_test__filestream_init__nonexist_id


TestSuite *api_file_streaming_init_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__filestream_init__hls_auth_usr);
    add_test(suite, api_test__filestream_init__hls_public);
    add_test(suite, api_test__filestream_init__hls_rotate_keyfile);
    add_test(suite, api_test__filestream_init__nonexist_id);
    return suite;
}
