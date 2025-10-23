#include <jansson.h>
#include "utils.h"
#include "../test/integration/test.h"

#define REQBODY_BASEPATH       "media/test/integration/examples/transcode_req_body_template"
#define REQBODY_VIDEO_BASEPATH REQBODY_BASEPATH "/video"
#define REQBODY_IMAGE_BASEPATH REQBODY_BASEPATH "/image"
#define ITEST_URL_PATH         "/file/transcode"

#define RUNNER_LOAD_JSN_FILE(fullpath) json_load_file(fullpath, (size_t)0, NULL)

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    json_t          *upld_req;
    json_t          *output_versions;
    int              expect_resp_code;
    const char      *expect_err_field;
    test_verify_cb_t fn_verify_job;
} itest_usrarg_t;

static __attribute__((optimize("O0"))) void
itest_verify__job_progress_update_ok(CURL *curl, test_setup_priv_t *privdata, void *cb_arg) {
    json_t *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info_recv = json_object_get(resp_obj, "error");
    assert_that(resp_obj, is_not_equal_to(NULL));
    assert_that(err_info_recv, is_equal_to(NULL));
    if (json_object_get(resp_obj, "percent_done")) {
        // there should be only one item returned for specific job progress
        float old_percent_done = json_real_value(json_object_get(job_item, "percent_done"));
        int   old_timestamp = json_integer_value(json_object_get(job_item, "timestamp"));
        float new_percent_done = json_real_value(json_object_get(resp_obj, "percent_done"));
        int   new_timestamp = json_integer_value(json_object_get(resp_obj, "timestamp"));
        assert_that((new_percent_done >= old_percent_done), is_equal_to(1));
        assert_that((new_timestamp >= old_timestamp), is_equal_to(1));
        if (new_timestamp > old_timestamp) {
            json_object_set_new(job_item, "percent_done", json_real(new_percent_done));
            json_object_set_new(job_item, "timestamp", json_integer(new_timestamp));
        }
        json_t *done = new_percent_done >= 1.0f ? json_true() : json_false();
        json_object_set_new(job_item, "done", done);
    } else { // TODO, verify the error detail, possible fields e.g. model, storage
        json_object_set_new(job_item, "error", json_true());
        json_object_set_new(job_item, "done", json_true());
    }
    json_decref(resp_obj);
} // end of itest_verify__job_progress_update_ok

static __attribute__((optimize("O0"))) void
itest_verify__job_terminated_conflict(CURL *curl, test_setup_priv_t *privdata, void *cb_arg) {
    json_t *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info_recv = json_object_get(resp_obj, "error");
    json_t *err_storage = json_object_get(err_info_recv, "storage");
    assert_that(err_info_recv, is_not_null);
    assert_that(err_storage, is_not_null);
    json_object_set_new(job_item, "error", json_true());
    json_object_set_new(job_item, "done", json_true());
    json_decref(resp_obj);
} // end of itest_verify__job_terminated_conflict

static __attribute__((optimize("O0"))) void
itest_verify__job_terminated_unsupported_format(CURL *curl, test_setup_priv_t *privdata, void *cb_arg) {
    json_t *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info_recv = json_object_get(resp_obj, "error");
    json_t *err_transcode = json_object_get(err_info_recv, "transcoder");
    assert_that(err_info_recv, is_not_null);
    assert_that(err_transcode, is_not_null);
    json_object_set_new(job_item, "error", json_true());
    json_object_set_new(job_item, "done", json_true());
    json_decref(resp_obj);
} // end of itest_verify__job_terminated_unsupported_format

static __attribute__((optimize("O0"))) void
itest_verify__job_input_video_corruption(CURL *curl, test_setup_priv_t *privdata, void *cb_arg) {
    json_t *job_item = cb_arg;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info_recv = json_object_get(resp_obj, "error");
    assert_that(err_info_recv, is_not_null);
    if (err_info_recv) {
        json_object_set_new(job_item, "error", json_true());
        json_object_set_new(job_item, "done", json_true());
    }
    json_decref(resp_obj);
}

static void _available_resource_lookup(
    json_t **upld_req, json_t **resource_id_item, const char *fsubtype_in, uint8_t _is_broken_in
) {
    json_t *req = NULL, *async_job_ids_item = NULL;
    int     idx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, req) {
        *resource_id_item = json_object_get(req, "resource_id");
        async_job_ids_item = json_object_get(req, "async_job_ids");
        const char *fsubtype = json_string_value(json_object_get(req, "subtype"));
        uint8_t     _is_broken_saved = json_boolean_value(json_object_get(req, "broken"));
        if (!fsubtype) {
            *resource_id_item = NULL;
            continue;
        }
        const char *_res_id = json_string_value(*resource_id_item);
        size_t      num_async_jobs = json_array_size(async_job_ids_item);
        uint8_t     type_matched = strncmp(fsubtype, fsubtype_in, strlen(fsubtype)) == 0;
        uint8_t     broken_cond_matched = _is_broken_saved == _is_broken_in;
        if (_res_id && type_matched && broken_cond_matched && num_async_jobs == 0) {
            break;
        } else {
            *resource_id_item = NULL;
        }
    }
    if (req && *resource_id_item) {
        *upld_req = req;
    } else {
        *upld_req = NULL;
        fprintf(
            stderr,
            "[itest][start_transcoding_file] no more ressource"
            " with the subtype:%s \n",
            fsubtype_in
        );
    }
} // end of _available_resource_lookup

static void itest_api_verify__start_transcode(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
    CURLcode        res;
    itest_usrarg_t *usr_arg = (itest_usrarg_t *)_usr_arg;
    json_t         *upld_req_ref = usr_arg->upld_req;
    long            actual_resp_code = 0, expect_resp_code = usr_arg->expect_resp_code;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    if (actual_resp_code > 0 && actual_resp_code < 400) { // ok
        const char *job_id = json_string_value(json_object_get(resp_obj, "job_id"));
        assert_that(job_id, is_not_equal_to(NULL));
        if (job_id) {
            const char *ver_label = NULL;
            json_t     *info = json_object(), *tmp = NULL,
                   *_version_map = json_object_get(upld_req_ref, "_versions");
            json_t *async_job_ids_item = json_object_get(upld_req_ref, "async_job_ids");
            json_array_append_new(async_job_ids_item, info);
            json_object_set_new(info, "job_id", json_string(job_id));
            json_object_set_new(info, "fn_verify_job", json_integer((size_t)usr_arg->fn_verify_job));
            if (!_version_map) {
                _version_map = json_object();
                json_object_set_new(upld_req_ref, "_versions", _version_map);
            }
            json_object_foreach(usr_arg->output_versions, ver_label, tmp) {
                json_object_set_new(_version_map, ver_label, json_true());
            } // store version string, (TODO, store transcoding detail)
        } else {
            fprintf(stderr, "[itest][api][start_transcode] line:%d, job_id NOT returned", __LINE__);
        }
    } else { // error
        const char *err_field = usr_arg->expect_err_field;
        json_t     *err_info = json_object_get(resp_obj, err_field);
        assert_that(err_info, is_not_null);
    }
    json_decref(resp_obj);
} // end of itest_api_verify__start_transcode

static void _api__start_transcoding_test__accepted_common(
    const char *req_body_template_filepath, json_t *upld_req, json_t *resource_id_item,
    test_verify_cb_t _fn_verify
) {
    const char  *sys_basepath = getenv("SYS_BASE_PATH");
    json_error_t jerror = {0};
    json_t      *req_body_template =
        PATH_CONCAT_THEN_RUN(sys_basepath, req_body_template_filepath, RUNNER_LOAD_JSN_FILE);
    assert_that(req_body_template, is_not_null);
    assert_that((jerror.line >= 0), is_equal_to(0));
    assert_that((jerror.column >= 0), is_equal_to(0));
    if (jerror.line >= 0 || jerror.column >= 0)
        return;
    char *req_body_raw = NULL;
    json_object_set(req_body_template, "resource_id", resource_id_item);
    size_t MAX_BYTES_REQ_BODY = json_dumpb(req_body_template, NULL, 0, 0);
    req_body_raw = calloc(MAX_BYTES_REQ_BODY, sizeof(char));
    size_t nwrite = json_dumpb(req_body_template, req_body_raw, MAX_BYTES_REQ_BODY, JSON_COMPACT);
    assert_that(nwrite, is_less_than(MAX_BYTES_REQ_BODY));
    itest_usrarg_t mock_usr_srg = {
        .upld_req = upld_req,
        .expect_resp_code = 202,
        .expect_err_field = NULL,
        .output_versions = json_object_get(req_body_template, "outputs"),
        .fn_verify_job = _fn_verify
    };
    char        url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t     *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t  *quota = json_array();
    uint32_t res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = &url[0],
        .headers = header_kv_serials,
        .req_body = {.serial_txt = req_body_raw, .src_filepath = NULL},
    };
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    json_decref(header_kv_serials);
    json_decref(quota);
    if (req_body_raw)
        free(req_body_raw);
    json_decref(req_body_template);
} // end of _api__start_transcoding_test__accepted_common

Ensure(api__transcode_test_video__accepted) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    // subcase #1 : normal case
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    if (resource_id_item) {
#define REQ_BODY_TEMPLATE_FILEPATH REQBODY_VIDEO_BASEPATH "/ok_1.json"
        _api__start_transcoding_test__accepted_common(
            REQ_BODY_TEMPLATE_FILEPATH, upld_req, resource_id_item, itest_verify__job_progress_update_ok
        );
        // subcase #2 : send another async job with the same resource and the same versions,
        // the RPC consumer should reject the later-coming job
        sleep(1);
        _api__start_transcoding_test__accepted_common(
            REQ_BODY_TEMPLATE_FILEPATH, upld_req, resource_id_item, itest_verify__job_terminated_conflict
        );
#undef REQ_BODY_TEMPLATE_FILEPATH
    } else {
        fprintf(stderr, "[itest][api][start_transcode] line:%d, missing mp4 video", __LINE__);
    }
    // subcase #3 : current only mp4 is supported. Try transcoding unsupported video,
    // rpc consumer will report error
    sleep(3);
    _available_resource_lookup(&upld_req, &resource_id_item, "avi", 0);
    if (resource_id_item) {
        _api__start_transcoding_test__accepted_common(
            REQBODY_VIDEO_BASEPATH "/ok_2.json", upld_req, resource_id_item,
            itest_verify__job_terminated_unsupported_format
        );
        sleep(3);
    } else {
        fprintf(stderr, "[itest][api][start_transcode] line:%d, missing avi video", __LINE__);
    }
#if 1
    do { // subcase #4 : try transcoding other different mp4 videos
        _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
        if (upld_req && resource_id_item) {
            sleep(13);
            _api__start_transcoding_test__accepted_common(
                REQBODY_VIDEO_BASEPATH "/ok_3.json", upld_req, resource_id_item,
                itest_verify__job_progress_update_ok
            );
        }
    } while (upld_req && resource_id_item);
#endif
} // end of  api__transcode_test_video__accepted

Ensure(api__transcode_test_video__corrupted_video) {
#define SELECT_BROKEN_VIDEO 1
    json_t *upld_req = NULL, *resource_id_item = NULL;
    do {
        _available_resource_lookup(&upld_req, &resource_id_item, "mp4", SELECT_BROKEN_VIDEO);
        if (upld_req && resource_id_item) {
            _api__start_transcoding_test__accepted_common(
                REQBODY_VIDEO_BASEPATH "/ok_3.json", upld_req, resource_id_item,
                itest_verify__job_input_video_corruption
            );
        }
    } while (upld_req && resource_id_item);
#undef SELECT_BROKEN_VIDEO
}

Ensure(api__transcode_test_video__improper_resolution
) { // will get transcoding error due to the improper odd number of the height
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    if (resource_id_item) {
        _api__start_transcoding_test__accepted_common(
            REQBODY_VIDEO_BASEPATH "/improper_video_resolution.json", upld_req, resource_id_item,
            itest_verify__job_input_video_corruption
        );
    } else {
        fprintf(stderr, "[itest][api][start_transcode] line:%d, missing mp4 video", __LINE__);
    }
}

Ensure(api__transcode_test_video__overwrite_existing) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    int     idx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, upld_req) {
        json_t *async_job_ids_item = json_object_get(upld_req, "async_job_ids");
        json_array_clear(async_job_ids_item);
    } // clean up all previous async jobs
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    if (resource_id_item) {
        _api__start_transcoding_test__accepted_common(
            REQBODY_VIDEO_BASEPATH "/ok_1_overwrite_version.json", upld_req, resource_id_item,
            itest_verify__job_progress_update_ok
        );
    } else {
        fprintf(stderr, "[itest][api][start_transcode] line:%d, missing mp4 video", __LINE__);
    }
} // end of  api__transcode_test_video__overwrite_existing

Ensure(api__transcode_test_image__accepted) {
#if 1
    json_t     *upld_req = NULL, *resource_id_item = NULL;
    int         idx = 0;
    const char *img_subtypes[4] = {"png", "gif", "jpg", "tiff"};
    const char *reqbody_templates[4] = {
        REQBODY_IMAGE_BASEPATH "/ok_1.json",
        REQBODY_IMAGE_BASEPATH "/ok_2.json",
        REQBODY_IMAGE_BASEPATH "/ok_3.json",
        REQBODY_IMAGE_BASEPATH "/ok_4.json",
    };
    for (idx = 0; idx < 4; idx++) {
        do {
            _available_resource_lookup(&upld_req, &resource_id_item, img_subtypes[idx], 0);
            if (upld_req && resource_id_item) {
                sleep(1);
                _api__start_transcoding_test__accepted_common(
                    reqbody_templates[idx], upld_req, resource_id_item, itest_verify__job_progress_update_ok
                );
            }
        } while (upld_req && resource_id_item);
    } // end of loop
#endif
} // end of  api__transcode_test_image__accepted

Ensure(api__transcode_test_image__overwrite_existing) {
#if 1
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "png", 0);
    if (upld_req && resource_id_item) {
        _api__start_transcoding_test__accepted_common(
            REQBODY_IMAGE_BASEPATH "/ok_1_overwrite.json", upld_req, resource_id_item,
            itest_verify__job_progress_update_ok
        );
    } else {
        fprintf(stderr, "[itest][api][start_transcode] line:%d, missing png picture", __LINE__);
    }
#endif
} // end of  api__transcode_test_image__overwrite_existing

Ensure(api__transcode_test_video__invalid_body) {
    json_t     *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    char        url[] = ITEST_URL_PATH;
    const char *codename_list[2] = {"upload_files", NULL};
    json_t     *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t  *quota = json_array();
    uint32_t res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = &url[0],
        .headers = header_kv_serials,
        .req_body = {.serial_txt = NULL, .src_filepath = NULL},
    };
    itest_usrarg_t mock_usr_srg = {.upld_req = upld_req, .expect_resp_code = 400, .expect_err_field = NULL};
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
} // end of  api__transcode_test_video__invalid_body

static void
test_verify__start_transcoding_invalid_elm_stream(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
    itest_usrarg_t *usr_arg = _usr_arg;
    const char     *err_field_in_st_elm = usr_arg->expect_err_field;
    usr_arg->expect_err_field = "elementary_streams";
    itest_api_verify__start_transcode(handle, privdata, _usr_arg);
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_detail =
        json_object_get(json_object_get(resp_obj, "elementary_streams"), err_field_in_st_elm);
    assert_that(err_detail, is_not_null);
    if (!err_detail)
        fprintf(
            stderr, "[itest][api][transcode] line:%d, error detail not found, label:%s \n", __LINE__,
            err_field_in_st_elm
        );
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_elm_stream

Ensure(api__transcode_test_video__invalid_elm_stream) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    struct {
        const char *template_filepath;
        const char *expect_field;
    } test_data[] = {
        {REQBODY_VIDEO_BASEPATH "/invalid_stream_type.json", "type"},
        {REQBODY_VIDEO_BASEPATH "/invalid_stream_codec.json", "codec"},
        {REQBODY_VIDEO_BASEPATH "/invalid_stream_video_attr_1.json", "height_pixel"},
        {REQBODY_VIDEO_BASEPATH "/invalid_stream_video_attr_2.json", "framerate"},
        {REQBODY_VIDEO_BASEPATH "/invalid_stream_audio_attr_1.json", "bitrate_kbps"},
    };
    char        url[] = ITEST_URL_PATH;
    const char *sys_basepath = getenv("SYS_BASE_PATH");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t     *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t  *quota = json_array();
    uint32_t res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = &url[0],
        .headers = header_kv_serials,
        .req_body = {.serial_txt = NULL, .src_filepath = NULL},
    };
    itest_usrarg_t mock_usr_srg = {.upld_req = upld_req, .expect_resp_code = 400, .expect_err_field = NULL};
    for (int idx = 0; idx < 5; idx++) {
        json_t *template =
            PATH_CONCAT_THEN_RUN(sys_basepath, test_data[idx].template_filepath, RUNNER_LOAD_JSN_FILE);
        assert_that(template, is_not_null);
        if (!template)
            continue;
        json_object_set(template, "resource_id", resource_id_item);
        size_t nb_required = json_dumpb(template, NULL, 0, 0);
        char   renderred_req_body[nb_required];
        size_t nwrite = json_dumpb(template, &renderred_req_body[0], nb_required, JSON_COMPACT);
        renderred_req_body[nwrite] = 0;
        setup_data.req_body.serial_txt = &renderred_req_body[0];
        mock_usr_srg.expect_err_field = test_data[idx].expect_field;
        run_client_request(
            &setup_data, test_verify__start_transcoding_invalid_elm_stream, (void *)&mock_usr_srg
        );
        json_decref(template);
    } // end of loop
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api__transcode_test_video__invalid_elm_stream

Ensure(api__transcode_test_video__invalid_resource_id) {
    json_t        *upld_req2 = NULL, *resource_id_item = NULL;
    json_t        *upld_req = json_array_get(_app_itest_active_upload_requests, 0);
    itest_usrarg_t mock_usr_srg = {
        .upld_req = upld_req, .expect_resp_code = 404, .expect_err_field = API_QPARAM_LABEL__RESOURCE_ID
    };
    const char *sys_basepath = getenv("SYS_BASE_PATH");
    const char *template_filepath = REQBODY_VIDEO_BASEPATH "/nonexist_resource_id.json";
    const char *codename_list[2] = {"upload_files", NULL};
    json_t     *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t  *quota = json_array();
    uint32_t res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = ITEST_URL_PATH,
        .headers = header_kv_serials,
        .req_body = {.serial_txt = NULL, .src_filepath = template_filepath},
    };
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&mock_usr_srg);
    char *req_body_raw = NULL;
    { // subcase #2, given user id doesn't match the owner of resource
        _available_resource_lookup(&upld_req2, &resource_id_item, "mp4", 0);
        json_t *req_body_item = PATH_CONCAT_THEN_RUN(sys_basepath, template_filepath, RUNNER_LOAD_JSN_FILE);
        json_object_set(req_body_item, "resource_id", resource_id_item);
        size_t MAX_BYTES_REQ_BODY = json_dumpb(req_body_item, NULL, 0, 0);
        req_body_raw = calloc(MAX_BYTES_REQ_BODY, sizeof(char));
        size_t nwrite = json_dumpb(req_body_item, req_body_raw, MAX_BYTES_REQ_BODY, JSON_COMPACT);
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
    if (req_body_raw)
        free(req_body_raw);
} // end of api__transcode_test_video__invalid_resource_id

static void
test_verify__start_transcoding_invalid_outputs(CURL *handle, test_setup_priv_t *privdata, void *usr_arg) {
    const char   **expect_fields = (const char **)usr_arg;
    itest_usrarg_t mock_usr_srg = {
        .upld_req = NULL, .expect_resp_code = 400, .expect_err_field = expect_fields[0]
    };
    itest_api_verify__start_transcode(handle, privdata, (void *)&mock_usr_srg);
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *err_info = json_object_get(json_object_get(resp_obj, expect_fields[0]), expect_fields[1]);
    assert_that(err_info, is_not_null);
    json_decref(resp_obj);
} // end of test_verify__start_transcoding_invalid_outputs

Ensure(api__transcode_test_video__invalid_output) {
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    char        url[] = ITEST_URL_PATH;
    const char *sys_basepath = getenv("SYS_BASE_PATH");
    const char *codename_list[2] = {"upload_files", NULL};
    json_t     *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t  *quota = json_array();
    uint32_t res_owner_id = json_integer_value(json_object_get(upld_req, "usr_id"));
    add_auth_token_to_http_header(header_kv_serials, res_owner_id, codename_list, quota);
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = &url[0],
        .headers = header_kv_serials,
        .req_body = {.serial_txt = NULL, .src_filepath = NULL},
    };
#define RUN_CODE(temp_filepath, ...) \
    { \
        json_t *template = PATH_CONCAT_THEN_RUN(sys_basepath, temp_filepath, RUNNER_LOAD_JSN_FILE); \
        json_object_set(template, "resource_id", resource_id_item); \
        size_t nb_required = json_dumpb(template, NULL, 0, 0); \
        char   renderred_req_body[nb_required]; \
        size_t nwrite = json_dumpb(template, &renderred_req_body[0], nb_required, JSON_COMPACT); \
        renderred_req_body[nwrite] = 0; \
        setup_data.req_body.serial_txt = &renderred_req_body[0]; \
        const char *expect_fields_hier[2] = {__VA_ARGS__}; \
        run_client_request( \
            &setup_data, test_verify__start_transcoding_invalid_outputs, (void *)&expect_fields_hier[0] \
        ); \
    }
    // subcase #1, invalid muxer
    RUN_CODE(REQBODY_VIDEO_BASEPATH "/invalid_output_muxer.json", "outputs", "container")
    // subcase #2, invalid version label
    RUN_CODE(REQBODY_VIDEO_BASEPATH "/invalid_output_version.json", "outputs", "version")
    // subcase #3, invalid map to elementary stream
    RUN_CODE(REQBODY_VIDEO_BASEPATH "/invalid_elm_stream_map.json", "outputs", "elementary_streams")
    json_decref(header_kv_serials);
    json_decref(quota);
#undef RUN_CODE
} // end of api__transcode_test_video__invalid_output

Ensure(api__transcode_test_video__permission_denied) {
#define REQ_BODY_PATTERN "{\"resource_id\":\"%s\"}"
    json_t *upld_req = NULL, *resource_id_item = NULL;
    _available_resource_lookup(&upld_req, &resource_id_item, "mp4", 0);
    assert_that(upld_req, is_not_null);
    assert_that(resource_id_item, is_not_null);
    if (!upld_req || !resource_id_item)
        return;
    uint32_t approved_usr_id = 0;
    { // look for the user who does NOT have permission to transcode the file
        int     idx = 0;
        json_t *existing_acl = json_object_get(upld_req, "ulvl_acl"), *item = NULL;
        json_array_foreach(existing_acl, idx, item) {
            json_t *capability = json_object_get(item, "access_control");
            if (capability) {
                json_t *transcode_item = json_object_get(capability, "transcode");
                if (transcode_item) {
                    uint8_t can_transcode = json_boolean_value(transcode_item);
                    if (!can_transcode) {
                        approved_usr_id = json_integer_value(json_object_get(item, "usr_id"));
                        break;
                    }
                }
            }
        } // end of loop
        assert_that(approved_usr_id, is_greater_than(0));
        if (approved_usr_id == 0)
            return;
    }
    const char *resource_id = json_string_value(resource_id_item);
    size_t      req_body_sz = sizeof(REQ_BODY_PATTERN) + strlen(resource_id);
    char        url[] = ITEST_URL_PATH, req_body[req_body_sz];
    json_t     *quota = json_array();
    json_t     *header_kv_serials = json_array();
    const char *codename_list[2] = {"upload_files", NULL};
    add_auth_token_to_http_header(header_kv_serials, approved_usr_id, codename_list, quota);
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t setup_data = {
        .method = "POST",
        .verbose = 0,
        .url_rel_ref = &url[0],
        .headers = header_kv_serials,
        .req_body = {.serial_txt = &req_body[0], .src_filepath = NULL}
    };
    itest_usrarg_t usr_args = {.upld_req = upld_req, .expect_resp_code = 403, .expect_err_field = "usr_id"};
    {
        size_t nwrite = snprintf(&req_body[0], req_body_sz, REQ_BODY_PATTERN, resource_id);
        req_body[nwrite] = 0;
    }
    fprintf(
        stderr, "[itest][api][transcode] line:%d, resource_id:%s, approved_usr_id:%d \n", __LINE__,
        resource_id, approved_usr_id
    );
    run_client_request(&setup_data, itest_api_verify__start_transcode, (void *)&usr_args);
    json_decref(header_kv_serials);
    json_decref(quota);
#undef REQ_BODY_PATTERN
} // end of  api__transcode_test_video__permission_denied

TestSuite *api_start_transcoding_file_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, api__transcode_test_video__invalid_body);
    add_test(suite, api__transcode_test_video__invalid_elm_stream);
    add_test(suite, api__transcode_test_video__invalid_resource_id);
    add_test(suite, api__transcode_test_video__invalid_output);
    add_test(suite, api__transcode_test_video__permission_denied);
    add_test(suite, api__transcode_test_video__accepted);
    add_test(suite, api__transcode_test_image__accepted);
    return suite;
}

TestSuite *api_start_transcoding_file_v2_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, api__transcode_test_video__overwrite_existing);
    add_test(suite, api__transcode_test_video__corrupted_video);
    add_test(suite, api__transcode_test_video__improper_resolution);
    add_test(suite, api__transcode_test_image__overwrite_existing);
    return suite;
}
