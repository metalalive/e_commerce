#include <jansson.h>
#include "../test/integration/test.h"

extern json_t *_app_itest_active_upload_requests;

static void _active_upload_requests_lookup(json_t **upld_req, json_t **resource_id_item, json_t **transcode_outputs)
{
    json_t *req = NULL;
    int idx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, req) {
        *resource_id_item  = json_object_get(req, "resource_id");
        *transcode_outputs = json_object_get(req, "transcode_outputs");
        if(json_string_value(*resource_id_item) && json_object_size(*transcode_outputs) == 0) {
            break;
        } else {
            *resource_id_item  = NULL;
            *transcode_outputs = NULL;
        }
    }
    assert_that(*resource_id_item, is_not_null);
    if(req) {
        *upld_req = req;
    }
} // end of _active_upload_requests_lookup

static void test_verify__start_transcoding_accepted(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    json_t *upld_req_ref = (json_t *)usr_arg;
    CURLcode res;
    long expect_resp_code = 202;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *expect_resource_id = json_string_value(json_object_get(upld_req_ref, "resource_id"));
    const char *actual_resource_id = json_string_value(json_object_get(resp_obj, "resource_id"));
    assert_that(actual_resource_id, is_equal_to_string(expect_resource_id));
    {
        json_t *expect_outputs = json_object_get(upld_req_ref, "transcode_outputs");
        json_t *actual_outputs = json_object_get(resp_obj, "outputs");
        json_t *item = NULL;
        const char *version = NULL;
        json_object_foreach(expect_outputs, version, item) {
            json_t *actual_output = json_object_get(actual_outputs, version);
            assert_that(actual_output, is_not_null);
            if(!actual_output) { continue; }
            int expect_status = (int)json_integer_value(json_object_get(item, "expect_status"));
            int actual_status = (int)json_integer_value(json_object_get(actual_output, "status"));
            assert_that(actual_status, is_equal_to(expect_status));
            const char *job_id = json_string_value(json_object_get(actual_output, "job_id"));
            assert_that(job_id, is_not_null);
            if(job_id) {
                json_object_set_new(item, "job_id", json_string(job_id));
            }
        }
    }
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_accepted


#define  MAX_BYTES_REQ_BODY  700
#define  REQ_BODY_TEMPLATE_FILEPATH  "./media/test/integration/examples/transcode_req_body_template/ok_1.json"
Ensure(api__start_transcoding_test__accepted) {
    json_t *upld_req = NULL, *resource_id_item = NULL, *transcode_outputs = NULL;
    _active_upload_requests_lookup(&upld_req, &resource_id_item, &transcode_outputs);
    if(!resource_id_item) {return;}
    char req_body_raw[MAX_BYTES_REQ_BODY] = {0};
    json_error_t jerror = {0};
    json_t *req_body_item = json_load_file(REQ_BODY_TEMPLATE_FILEPATH, (size_t)0, &jerror);
    if(jerror.line >= 0 || jerror.column >= 0) {
        return;
    } else {
        json_object_set(req_body_item, "resource_id", resource_id_item);
        size_t nwrite = json_dumpb((const json_t *)req_body_item, &req_body_raw[0],  MAX_BYTES_REQ_BODY, JSON_COMPACT);
        assert_that(nwrite, is_less_than(MAX_BYTES_REQ_BODY));
        json_t *dummy = NULL;
        const char *version = NULL;
        json_object_foreach(json_object_get(req_body_item, "outputs"), version, dummy) {
            json_t *item = json_object();
            json_object_set_new(item, "expect_status", json_integer(202));
            json_object_set_new(transcode_outputs, version, item);
        }
    }
    char url[] = "https://localhost:8010/file/transcode";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=&req_body_raw[0], .src_filepath=NULL},
    };
    run_client_request(&setup_data, test_verify__start_transcoding_accepted, (void *)upld_req);
    { // TODO:wait until connection timeout, consume API again, the app server should reconnect the AMQP broker
        //// app_cfg_t *acfg = app_get_global_cfg();
        //// arpc_cfg_t *rpc_cfg = &acfg->rpc.entries[0];
        //// size_t delay_secs = 4 * rpc_cfg->attributes.timeout_secs;
        //// sleep(delay_secs);
        //// setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_2.json";
        //// run_client_request(&setup_data, test_verify__complete_multipart_upload, NULL);
    }
    json_decref(req_body_item);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of  api__start_transcoding_test__accepted
#undef  MAX_BYTES_REQ_BODY
#undef  REQ_BODY_TEMPLATE_FILEPATH


static void test_verify__start_transcoding_invalid_body(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    const char *expect_field = (const char *)usr_arg;
    CURLcode res;
    long expect_resp_code = 400;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(resp_obj, expect_field);
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_body

Ensure(api__start_transcoding_test__invalid_body) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0); 
    char url[] = "https://localhost:8010/file/transcode";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    setup_data.req_body.serial_txt = "plain text";
    run_client_request(&setup_data, test_verify__start_transcoding_invalid_body, (void *)"non-field");
    setup_data.req_body.serial_txt = "{}";
    run_client_request(&setup_data, test_verify__start_transcoding_invalid_body, (void *)"resource_id");
    setup_data.req_body.serial_txt = "{\"resource_id\":null}";
    run_client_request(&setup_data, test_verify__start_transcoding_invalid_body, (void *)"resource_id");
    setup_data.req_body.serial_txt = "{\"resource_id\":\"aH1234s\"}";
    run_client_request(&setup_data, test_verify__start_transcoding_invalid_body, (void *)"elementary_streams");
    setup_data.req_body.serial_txt = "{\"resource_id\":\"aH1234s\", \"elementary_streams\":{}}";
    run_client_request(&setup_data, test_verify__start_transcoding_invalid_body, (void *)"elementary_streams");
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of  api__start_transcoding_test__invalid_body 


static void test_verify__start_transcoding_invalid_elm_stream(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    const char *expect_field = (const char *)usr_arg;
    test_verify__start_transcoding_invalid_body(handle, privdata, (void *)"elementary_streams");
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(json_object_get(resp_obj, "elementary_streams"), expect_field);
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_elm_stream

Ensure(api__start_transcoding_test__invalid_elm_stream) {
    json_t *upld_req = NULL, *resource_id_item = NULL, *transcode_outputs = NULL;
    _active_upload_requests_lookup(&upld_req, &resource_id_item, &transcode_outputs);
    struct {
        const char *template_filepath;
        const char *expect_field;
    } test_data[] = {
       {"./media/test/integration/examples/transcode_req_body_template/invalid_stream_type.json" , "type"},
       {"./media/test/integration/examples/transcode_req_body_template/invalid_stream_codec.json", "codec"},
       {"./media/test/integration/examples/transcode_req_body_template/invalid_stream_video_attr_1.json", "height_pixel"},
       {"./media/test/integration/examples/transcode_req_body_template/invalid_stream_video_attr_2.json", "framerate"},
       {"./media/test/integration/examples/transcode_req_body_template/invalid_stream_audio_attr_1.json", "bitrate_kbps"},
    };
    char url[] = "https://localhost:8010/file/transcode";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    for (int idx = 0; idx < 5; idx++) {
        setup_data.req_body.src_filepath = test_data[idx].template_filepath;
        run_client_request( &setup_data, test_verify__start_transcoding_invalid_elm_stream,
                (void *)test_data[idx].expect_field );
    }
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api__start_transcoding_test__invalid_elm_stream


static void  test_verify__start_transcoding__invalid_resource_id(
        CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    long *expect_resp_code = (long *)usr_arg;
    CURLcode res;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(*expect_resp_code));
} // end of test_verify__start_transcoding__invalid_resource_id

Ensure(api__start_transcoding_test__invalid_resource_id) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    const char *template_filepath = "./media/test/integration/examples/transcode_req_body_template/nonexist_resource_id.json";
    char url[] = "https://localhost:8010/file/transcode";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=template_filepath},
    };
    long expect_resp_code = 404;
    run_client_request(&setup_data, test_verify__start_transcoding__invalid_resource_id,
            (void *)&expect_resp_code);
#define  MAX_BYTES_REQ_BODY  500
    char req_body_raw[MAX_BYTES_REQ_BODY] = {0};
    { // subcase #2, given user id doesn't match the owner of resource
        json_t *upld_req2 = NULL, *resource_id_item = NULL, *transcode_outputs = NULL;
        _active_upload_requests_lookup(&upld_req2, &resource_id_item, &transcode_outputs);
        json_error_t jerror = {0};
        json_t *req_body_item = json_load_file(template_filepath, (size_t)0, &jerror);
        json_object_set(req_body_item, "resource_id", resource_id_item);
        size_t nwrite = json_dumpb((const json_t *)req_body_item, &req_body_raw[0],  MAX_BYTES_REQ_BODY, JSON_COMPACT);
        assert_that(nwrite, is_less_than(MAX_BYTES_REQ_BODY));
        json_decref(req_body_item);
    }
#undef  MAX_BYTES_REQ_BODY
    setup_data.req_body.src_filepath = NULL;
    setup_data.req_body.serial_txt = &req_body_raw[0];
    expect_resp_code = 403;
    run_client_request(&setup_data, test_verify__start_transcoding__invalid_resource_id,
            (void *)&expect_resp_code);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api__start_transcoding_test__invalid_resource_id


static void test_verify__start_transcoding_invalid_outputs(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    const char **expect_fields = (const char **)usr_arg;
    test_verify__start_transcoding_invalid_body(handle, privdata, (void *)expect_fields[0]);
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(json_object_get(resp_obj, expect_fields[0]),
            expect_fields[1] );
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_outputs


Ensure(api__start_transcoding_test__invalid_output) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    char url[] = "https://localhost:8010/file/transcode";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    { // subcase #1, invalid muxer
        setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_template/invalid_output_muxer.json";
        const char *expect_fields_hier[2] = {"outputs", "container"};
        run_client_request(&setup_data,  test_verify__start_transcoding_invalid_outputs,
                (void *)&expect_fields_hier[0]);
    }
    { // subcase #3, invalid version label
        setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_template/invalid_output_version.json";
        const char *expect_fields_hier[2] = {"outputs", "version"};
        run_client_request(&setup_data,  test_verify__start_transcoding_invalid_outputs,
                (void *)&expect_fields_hier[0]);
    }
    { // subcase #3, invalid map to elementary stream
        setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_template/invalid_elm_stream_map.json";
        const char *expect_fields_hier[2] = {"outputs", "elementary_streams"};
        run_client_request(&setup_data,  test_verify__start_transcoding_invalid_outputs,
                (void *)&expect_fields_hier[0]);
    }
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api__start_transcoding_test__invalid_output


TestSuite *api_start_transcoding_file_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api__start_transcoding_test__invalid_body);
    add_test(suite, api__start_transcoding_test__invalid_elm_stream);
    add_test(suite, api__start_transcoding_test__invalid_resource_id);
    add_test(suite, api__start_transcoding_test__invalid_output);
    add_test(suite, api__start_transcoding_test__accepted);
    return suite;
}
