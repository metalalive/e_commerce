#include <jansson.h>
#include "../test/integration/test.h"

#define  ITEST_URL_PATH  "https://localhost:8010/file/transcode"

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    json_t  *upld_req;
    int  expect_resp_code;
    const char *expect_err_field;
    test_verify_cb_t  fn_verify_job;
} itest_usrarg_t;


static  __attribute__((optimize("O0"))) void  itest_verify__job_progress_update_ok(
        CURL *curl, test_setup_priv_t *privdata, void *cb_arg)
{
    json_t  *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t  *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t  *err_info_recv = json_object_get(resp_obj, "error");
    assert_that(resp_obj, is_not_equal_to(NULL));
    assert_that(err_info_recv, is_equal_to(NULL));
    if(json_object_get(resp_obj, "percent_done")) {
        // there should be only one item returned for specific job progress
        float  old_percent_done = json_real_value(json_object_get(job_item, "percent_done"));
        int    old_timestamp = json_integer_value(json_object_get(job_item, "timestamp"));
        float  new_percent_done = json_real_value(json_object_get(resp_obj, "percent_done"));
        int    new_timestamp = json_integer_value(json_object_get(resp_obj, "timestamp"));
        assert_that((new_percent_done >= old_percent_done), is_equal_to(1));
        assert_that((new_timestamp >= old_timestamp), is_equal_to(1));
        if(new_timestamp > old_timestamp) {
            json_object_set_new(job_item, "percent_done", json_real(new_percent_done));
            json_object_set_new(job_item, "timestamp", json_integer(new_timestamp));
        }
        json_t  *done = new_percent_done >= 1.0f ? json_true(): json_false();
        json_object_set_new(job_item, "done", done);
    } else { // TODO, verify the error detail, possible fields e.g. model, storage 
        json_object_set_new(job_item, "error", json_true());
        json_object_set_new(job_item, "done", json_true());
    }
    json_decref(resp_obj);
} // end of itest_verify__job_progress_update_ok


static  __attribute__((optimize("O0"))) void  itest_verify__job_terminated_conflict(
        CURL *curl, test_setup_priv_t *privdata, void *cb_arg)
{
    json_t  *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t  *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t  *err_info_recv = json_object_get(resp_obj, "error");
    json_t  *err_storage = json_object_get(err_info_recv, "storage");
    assert_that(err_info_recv, is_not_null);
    assert_that(err_storage, is_not_null);
    json_object_set_new(job_item, "error", json_true());
    json_object_set_new(job_item, "done", json_true());
    json_decref(resp_obj);
} // end of itest_verify__job_terminated_conflict


static  __attribute__((optimize("O0"))) void  itest_verify__job_terminated_unsupported_format (
        CURL *curl, test_setup_priv_t *privdata, void *cb_arg)
{
    json_t  *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t  *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t  *err_info_recv = json_object_get(resp_obj, "error");
    json_t  *err_transcode = json_object_get(err_info_recv, "transcoder");
    assert_that(err_info_recv, is_not_null);
    assert_that(err_transcode, is_not_null);
    json_object_set_new(job_item, "error", json_true());
    json_object_set_new(job_item, "done", json_true());
    json_decref(resp_obj);
} // end of itest_verify__job_terminated_unsupported_format


static void _available_resource_lookup(json_t **upld_req, json_t **resource_id_item, const char *fsubtype_in)
{
    json_t *req = NULL, *async_job_ids_item = NULL;
    int idx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, req) {
        *resource_id_item  = json_object_get(req, "resource_id");
        async_job_ids_item = json_object_get(req, "async_job_ids");
        const char *fsubtype  = json_string_value(json_object_get(req, "subtype"));
        if(!fsubtype) {
            *resource_id_item  = NULL;
            continue;
        }
        const char *_res_id = json_string_value(*resource_id_item);
        size_t   num_async_jobs = json_array_size(async_job_ids_item);
        uint8_t  type_matched = strncmp(fsubtype, fsubtype_in, strlen(fsubtype)) == 0;
        if(_res_id && type_matched && num_async_jobs == 0) {
            break;
        } else {
            *resource_id_item  = NULL;
        }
    }
    if(req && *resource_id_item) {
        *upld_req = req;
    } else {
        fprintf(stderr, "[itest][start_transcoding_file] no more ressource"
                " with the subtype:%s \n" , fsubtype_in);
    }
} // end of _available_resource_lookup

static void itest_api_verify__start_transcode(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg)
{
    CURLcode res;
    itest_usrarg_t *usr_arg = (itest_usrarg_t *)_usr_arg;
    json_t *upld_req_ref = usr_arg ->upld_req;
    long expect_resp_code = usr_arg ->expect_resp_code;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    if(actual_resp_code > 0 && actual_resp_code < 400) { // ok
        const char *job_id = json_string_value(json_object_get(resp_obj, "job_id"));
        assert_that(job_id, is_not_equal_to(NULL));
        if(job_id) {
            json_t *info = json_object();
            json_t *async_job_ids_item = json_object_get(upld_req_ref, "async_job_ids");
            json_array_append_new(async_job_ids_item, info);
            json_object_set_new(info, "job_id",  json_string(job_id));
            json_object_set_new(info, "fn_verify_job",  json_integer((size_t)usr_arg ->fn_verify_job));
        } // TODO, store version string, possibly transcoding detail
    } else { // error
        const char *err_field = usr_arg->expect_err_field;
        json_t *err_info = json_object_get(resp_obj, err_field);
        assert_that(err_info, is_not_null);
    }
    json_decref(resp_obj);
} // end of itest_api_verify__start_transcode


static void  _api__start_transcoding_test__accepted_common(const char *req_body_template_filepath,
        json_t *upld_req,  json_t *resource_id_item, test_verify_cb_t  _fn_verify)
{
    json_error_t jerror = {0};
    json_t *req_body_template = json_load_file(req_body_template_filepath, (size_t)0, &jerror);
    if(jerror.line >= 0 || jerror.column >= 0) {
        assert_that(1, is_equal_to(0));
        return;
    }
    char *req_body_raw = NULL;
    json_object_set(req_body_template, "resource_id", resource_id_item);
    size_t MAX_BYTES_REQ_BODY = json_dumpb(req_body_template, NULL, 0, 0);
    req_body_raw = calloc(MAX_BYTES_REQ_BODY, sizeof(char));
    size_t nwrite = json_dumpb(req_body_template, req_body_raw,  MAX_BYTES_REQ_BODY, JSON_COMPACT);
    assert_that(nwrite, is_less_than(MAX_BYTES_REQ_BODY));
    itest_usrarg_t  mock_usr_srg = {.upld_req=upld_req, .expect_resp_code=202,
        .expect_err_field=NULL, .fn_verify_job=_fn_verify };
    char url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=req_body_raw, .src_filepath=NULL},
    };
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    json_decref(header_kv_serials);
    json_decref(quota);
    if(req_body_raw)
        free(req_body_raw);
    json_decref(req_body_template);
} // end of _api__start_transcoding_test__accepted_common


Ensure(api__start_transcoding_test__accepted) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    // subcase #1 : normal case
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4");
    if(resource_id_item) {
#define  REQ_BODY_TEMPLATE_FILEPATH  "./media/test/integration/examples/transcode_req_body_template/ok_1.json"
        _api__start_transcoding_test__accepted_common(REQ_BODY_TEMPLATE_FILEPATH, upld_req,
                resource_id_item,  itest_verify__job_progress_update_ok);
        // subcase #2 : send another async job with the same resource and the same version,
        // the RPC consumer should reject the later-coming job
        sleep(1);
        _api__start_transcoding_test__accepted_common(REQ_BODY_TEMPLATE_FILEPATH, upld_req,
                resource_id_item,  itest_verify__job_terminated_conflict);
#undef  REQ_BODY_TEMPLATE_FILEPATH
    } else {
        fprintf(stderr, "[itest] missing mp4 video in api__start_transcoding_test__accepted");
    }
    // subcase #3 : current only mp4 is supported. Try transcoding unsupported video,
    // rpc consumer will report error
    sleep(10);
    _available_resource_lookup(&upld_req, &resource_id_item, "avi");
    if(resource_id_item) {
#define  REQ_BODY_TEMPLATE_FILEPATH  "./media/test/integration/examples/transcode_req_body_template/ok_2.json"
        _api__start_transcoding_test__accepted_common(REQ_BODY_TEMPLATE_FILEPATH, upld_req,
                resource_id_item,  itest_verify__job_terminated_unsupported_format);
#undef  REQ_BODY_TEMPLATE_FILEPATH
    } else {
        fprintf(stderr, "[itest] missing avi video in api__start_transcoding_test__accepted");
    }
    // subcase #4 : try transcoding another different mp4 video
    do {
        _available_resource_lookup(&upld_req, &resource_id_item, "mp4");
        if(upld_req && resource_id_item) {
#define  REQ_BODY_TEMPLATE_FILEPATH  "./media/test/integration/examples/transcode_req_body_template/ok_3.json"
            sleep(15);
            _api__start_transcoding_test__accepted_common(REQ_BODY_TEMPLATE_FILEPATH, upld_req,
                    resource_id_item,  itest_verify__job_progress_update_ok);
#undef  REQ_BODY_TEMPLATE_FILEPATH
        }
    } while (upld_req && resource_id_item);
    //  subcase #5 : transcoding corrupted mp4 (TODO)
    {
        //// app_cfg_t *acfg = app_get_global_cfg();
        //// arpc_cfg_t *rpc_cfg = &acfg->rpc.entries[0];
        //// size_t delay_secs = 4 * rpc_cfg->attributes.timeout_secs;
        //// sleep(delay_secs);
        //// setup_data.req_body.src_filepath = "./media/test/integration/examples/transcode_req_body_2.json";
        //// run_client_request(&setup_data, test_verify__complete_multipart_upload, NULL);
    }
} // end of  api__start_transcoding_test__accepted


Ensure(api__start_transcoding_test__invalid_body) {
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0); 
    char url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t  res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    itest_usrarg_t  mock_usr_srg = {.upld_req=upld_req, .expect_resp_code=400, .expect_err_field=NULL };
    setup_data.req_body.serial_txt = "plain text";
    mock_usr_srg.expect_err_field = "non-field";
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    setup_data.req_body.serial_txt = "{}";
    mock_usr_srg.expect_err_field = API_QPARAM_LABEL__RESOURCE_ID;
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    setup_data.req_body.serial_txt = "{\"resource_id\":null}";
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    setup_data.req_body.serial_txt = "{\"resource_id\":\"aH1234s\"}";
    mock_usr_srg.expect_resp_code = 404;
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    setup_data.req_body.serial_txt = "{\"resource_id\":\"aH1234x\", \"elementary_streams\":{}}";
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of  api__start_transcoding_test__invalid_body 


static void test_verify__start_transcoding_invalid_elm_stream(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg)
{
    itest_usrarg_t  *usr_arg = _usr_arg;
    const char *err_field_in_st_elm = usr_arg->expect_err_field;
    usr_arg->expect_err_field = "elementary_streams";
    itest_api_verify__start_transcode(handle, privdata, _usr_arg);
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(json_object_get(resp_obj, "elementary_streams"), err_field_in_st_elm);
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_elm_stream


Ensure(api__start_transcoding_test__invalid_elm_stream)
{
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4");
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
    char url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t  res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    itest_usrarg_t  mock_usr_srg = {.upld_req=upld_req, .expect_resp_code=400, .expect_err_field=NULL };
    for (int idx = 0; idx < 5; idx++) {
        json_t *template = json_load_file(test_data[idx].template_filepath, 0, NULL);
        assert_that(template, is_not_null);
        if(!template) continue;
        json_object_set(template, "resource_id", resource_id_item);
        size_t nb_required = json_dumpb(template, NULL, 0, 0);
        char renderred_req_body[nb_required];
        size_t nwrite = json_dumpb(template, &renderred_req_body[0], nb_required, JSON_COMPACT);
        renderred_req_body[nwrite] = 0;
        setup_data.req_body.serial_txt = &renderred_req_body[0];
        mock_usr_srg.expect_err_field = test_data[idx].expect_field;
        run_client_request( &setup_data, test_verify__start_transcoding_invalid_elm_stream,
                (void *)&mock_usr_srg );
        json_decref(template);
    } // end of loop
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api__start_transcoding_test__invalid_elm_stream


Ensure(api__start_transcoding_test__invalid_resource_id)
{
    json_t *upld_req2 = NULL, *resource_id_item = NULL;
    json_t *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    itest_usrarg_t  mock_usr_srg = {.upld_req=upld_req, .expect_resp_code=404, .expect_err_field=API_QPARAM_LABEL__RESOURCE_ID};
    const char *template_filepath = "./media/test/integration/examples/transcode_req_body_template/nonexist_resource_id.json";
    char url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=template_filepath},
    };
    run_client_request(&setup_data, itest_api_verify__start_transcode,  (void *)&mock_usr_srg);
    char *req_body_raw = NULL;
    { // subcase #2, given user id doesn't match the owner of resource
        _available_resource_lookup(&upld_req2, &resource_id_item, "mp4");
        json_t *req_body_item = json_load_file(template_filepath, (size_t)0, NULL);
        json_object_set(req_body_item, "resource_id", resource_id_item);
        size_t MAX_BYTES_REQ_BODY  = json_dumpb(req_body_item, NULL, 0, 0);
        req_body_raw = calloc(MAX_BYTES_REQ_BODY, sizeof(char));
        size_t nwrite = json_dumpb(req_body_item, req_body_raw,  MAX_BYTES_REQ_BODY, JSON_COMPACT);
        assert_that(nwrite, is_less_than(MAX_BYTES_REQ_BODY));
        json_decref(req_body_item);
        setup_data.req_body.src_filepath = NULL;
        setup_data.req_body.serial_txt = req_body_raw;
    }
    mock_usr_srg.expect_resp_code = 403;
    mock_usr_srg.upld_req = upld_req2;
    mock_usr_srg.expect_err_field = "usr_id";
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    json_decref(header_kv_serials);
    json_decref(quota);
    if(req_body_raw)
        free(req_body_raw);
} // end of api__start_transcoding_test__invalid_resource_id


static void test_verify__start_transcoding_invalid_outputs(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    const char **expect_fields = (const char **)usr_arg;
    itest_usrarg_t  mock_usr_srg = {.upld_req=NULL, .expect_resp_code=400, .expect_err_field=expect_fields[0]};
    itest_api_verify__start_transcode(handle, privdata, (void *)&mock_usr_srg);
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(json_object_get(resp_obj, expect_fields[0]),
            expect_fields[1] );
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_outputs


Ensure(api__start_transcoding_test__invalid_output) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4");
    char url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *quota = json_array();
    uint32_t res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
#define  RUN_CODE(temp_filepath, ...) { \
    json_t *template = json_load_file(temp_filepath, 0, NULL); \
    json_object_set(template, "resource_id", resource_id_item); \
    size_t nb_required = json_dumpb(template, NULL, 0, 0); \
    char renderred_req_body[nb_required]; \
    size_t nwrite = json_dumpb(template, &renderred_req_body[0], nb_required, JSON_COMPACT); \
    renderred_req_body[nwrite] = 0; \
    setup_data.req_body.serial_txt = &renderred_req_body[0]; \
    const char *expect_fields_hier[2] = {__VA_ARGS__}; \
    run_client_request(&setup_data,  test_verify__start_transcoding_invalid_outputs, \
            (void *)&expect_fields_hier[0]); \
}
    // subcase #1, invalid muxer
    RUN_CODE("media/test/integration/examples/transcode_req_body_template/invalid_output_muxer.json", "outputs", "container")
    // subcase #2, invalid version label
    RUN_CODE("media/test/integration/examples/transcode_req_body_template/invalid_output_version.json", "outputs", "version")
    // subcase #3, invalid map to elementary stream
    RUN_CODE("media/test/integration/examples/transcode_req_body_template/invalid_elm_stream_map.json", "outputs", "elementary_streams")
    json_decref(header_kv_serials);
    json_decref(quota);
#undef  RUN_CODE
} // end of api__start_transcoding_test__invalid_output


Ensure(api__start_transcoding_test__permission_denied)
{
#define  REQ_BODY_PATTERN  "{\"resource_id\":\"%s\"}"
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4");
    assert_that(upld_req, is_not_null);
    assert_that(resource_id_item, is_not_null);
    if(!upld_req || !resource_id_item)
        return;
    uint32_t approved_usr_id  = 0;
    { // look for the user who does NOT have permission to transcode the file
        int idx = 0;
        json_t *existing_acl = json_object_get(upld_req, "ulvl_acl"), *item = NULL;
        json_array_foreach(existing_acl, idx, item) {
            json_t *capability = json_object_get(item,"access_control");
            uint8_t can_transcode = json_integer_value(json_object_get(capability, "transcode"));
            if(!can_transcode) {
                approved_usr_id = json_integer_value(json_object_get(item, "usr_id"));
                break;
            }
        } // end of loop
        assert_that(approved_usr_id, is_greater_than(0));
        if(approved_usr_id == 0)
            return;
    }
    const char *resource_id = json_string_value(resource_id_item);
    size_t req_body_sz = sizeof(REQ_BODY_PATTERN) + strlen(resource_id);
    char url[] = ITEST_URL_PATH, req_body[req_body_sz];
    json_t *quota = json_array();
    json_t *header_kv_serials = json_array();
    const char *codename_list[2] = {"upload_files", NULL};
    add_auth_token_to_http_header(header_kv_serials, approved_usr_id, codename_list, quota);
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {.method="POST", .verbose=0, .url=&url[0], .headers=header_kv_serials,
        .req_body = {.serial_txt=&req_body[0], .src_filepath=NULL}};
    itest_usrarg_t  usr_args = {.upld_req=upld_req, .expect_resp_code=403, .expect_err_field="usr_id"};
    {
        size_t nwrite = snprintf(&req_body[0], req_body_sz, REQ_BODY_PATTERN, resource_id);
        req_body[nwrite] = 0;
    }
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&usr_args);
    json_decref(header_kv_serials);
    json_decref(quota);
#undef   REQ_BODY_PATTERN 
} // end of  api__start_transcoding_test__permission_denied


TestSuite *api_start_transcoding_file_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api__start_transcoding_test__invalid_body);
    add_test(suite, api__start_transcoding_test__invalid_elm_stream);
    add_test(suite, api__start_transcoding_test__invalid_resource_id);
    add_test(suite, api__start_transcoding_test__invalid_output);
    add_test(suite, api__start_transcoding_test__permission_denied);
    add_test(suite, api__start_transcoding_test__accepted);
    return suite;
}
