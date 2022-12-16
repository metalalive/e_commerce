#include "views.h"
#include "../test/integration/test.h"

extern json_t *_app_itest_active_upload_requests;

typedef struct {
    uint32_t resp_code;
    uint32_t part;
    const char *f_chksum;
    const char *filepath;
    json_t *upld_req_ref;
} usrarg_t;

static void test__update_partnum_stats(json_t *stats, size_t partnum)
{
    json_t *part_nums = json_object_get(stats, "part");
    json_t *new_val = json_integer(partnum);
    json_array_append_new(part_nums, new_val);
} // end of test__update_partnum_stats

static void test_verify__app_server_response(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    usrarg_t  *_usr_arg = (usrarg_t *)usr_arg;
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
    if(actual_resp_code == 200) {
        test__update_partnum_stats(_usr_arg->upld_req_ref, _usr_arg->part);
    }
} // end of test_verify__app_server_response

#define EXPECT_PART  3
#define CHUNK_FILE_PATH  "media/test/integration/examples/test_file_chunk_0"
#define CHUNK_FILE_CHKSUM  "c618d7709f63b3e2cc11f799f3d1a7edb53b5bc0"
Ensure(api_test_upload_part__singlechunk_ok) {
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
    usrarg_t  cb_arg = { .resp_code=200, .part=EXPECT_PART,
        .f_chksum=CHUNK_FILE_CHKSUM, .upld_req_ref=upld_req };
    run_client_request(&setup_data, test_verify__app_server_response, (void *)&cb_arg);
    // TODO, verify that re-uploading the same part in a given request, it should erase
    //  previously uploaded part and update the database table.
    json_decref(header_kv_serials);
    json_decref(quota);
} // end of api_test_upload_part__singlechunk_ok
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

#define EXPECT_PART  3
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
#define  CHUNK_FILE_PATH_1    "media/test/integration/examples/test_file_chunk_0"
#define  CHUNK_FILE_PATH_2    "media/test/integration/examples/test_file_chunk_1"
#define  CHUNK_FILE_PATH_3    "media/test/integration/examples/test_file_chunk_2"
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
        json_object_set_new(item, "app_code", json_integer(APP_CODE));
        json_object_set_new(item, "mat_code", json_integer(QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER));
        json_object_set_new(item, "maxnum", json_integer(max_upld_kbytes));
        json_array_append_new(quota, item);
    }
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.size = 1;
    usrarg_t  cb_args[NUM_PARTS] = {
        {.resp_code=200, .part=7, .f_chksum=CHUNK_FILE_CHKSUM_1, .filepath=CHUNK_FILE_PATH_1, .upld_req_ref=upld_req},
        {.resp_code=200, .part=3, .f_chksum=CHUNK_FILE_CHKSUM_2, .filepath=CHUNK_FILE_PATH_2, .upld_req_ref=upld_req},
        {.resp_code=403, .part=4, .f_chksum=CHUNK_FILE_CHKSUM_3, .filepath=CHUNK_FILE_PATH_3, .upld_req_ref=upld_req},
    };
    for (size_t idx = 0; idx < NUM_PARTS; idx++) {
        char url[128] = {0};
        sprintf(&url[0], "https://%s:%d%s?req_seq=%d&part=%d", "localhost", 8010, "/upload/multipart/part",
                req_seq, cb_args[idx].part );
        setup_data.url = &url[0];
#pragma GCC diagnostic ignored "-Wdiscarded-qualifiers"
        setup_data.upload_filepaths.entries[0] = cb_args[idx].filepath;
#pragma GCC diagnostic pop
        run_client_request(&setup_data, test_verify__app_server_response, (void *)&cb_args[idx]);
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


static  char *_itest_filechunk_metadata = NULL;

Ensure(api_test_upload_part__multichunk_outoforder)
{
    json_t *header_kv_serials = json_array();
    json_t *usr_upload_quota = json_object();
    json_error_t jerror;
    uint8_t  is_array = 0;
    json_t *files_info = json_load_file(_itest_filechunk_metadata, (size_t)0, &jerror);
    {
        is_array = json_is_array(files_info);
        assert_that(is_array, is_equal_to(1));
        if(!is_array) { goto done; }
    }
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .headers = header_kv_serials,
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.size = 1;
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_t *info, *file_info, *chunkinfo;
    size_t idx, jdx;
    json_array_foreach(files_info, idx, file_info) {
        chunkinfo = json_object_get(file_info, "chunks");
        const char *file_type = json_string_value(json_object_get(file_info, "type"));
        const char *file_subtype = json_string_value(json_object_get(file_info, "subtype"));
        uint8_t     is_broken = json_boolean_value(json_object_get(file_info, "broken"));
        {
            is_array = json_is_array(chunkinfo);
            assert_that(is_array, is_equal_to(1));
            if(!is_array) { break; }
        }
        size_t req_seq_idx = 2 + idx; // the first 2 upload requests are reserved for the 2 test cases above
        json_t *upld_req = json_array_get(_app_itest_active_upload_requests, req_seq_idx);
        assert_that(upld_req, is_not_equal_to(NULL));
        if(!upld_req) { break; } // not have enough upload requests
        json_object_set_new(upld_req, "type",    json_string(file_type));
        json_object_set_new(upld_req, "subtype", json_string(file_subtype));
        json_object_set_new(upld_req, "broken", json_boolean(is_broken));
        uint32_t usr_id  = json_integer_value(json_object_get(upld_req, "usr_id" ));
        uint32_t req_seq = json_integer_value(json_object_get(upld_req, "req_seq"));
        ssize_t file_tot_sz_bytes = 0;
        json_array_foreach(chunkinfo, jdx, info) {
            const char *path = json_string_value(json_object_get(info, "path"));
            assert_that(path, is_not_null);
            if(!path) { goto done; }
            int err = access(path, R_OK | F_OK);
            assert_that(err, is_equal_to(0));
            if(err) { goto done; }
            struct stat fstatbuf = {0};
            err = stat(path, &fstatbuf);
            if(err) { goto done; }
            file_tot_sz_bytes += (size_t) fstatbuf.st_size; // sizes in bytes
        } // end of inner loop
        {
            char usr_id_str[USR_ID_STR_SIZE + 1] = {0};
            snprintf(&usr_id_str[0], USR_ID_STR_SIZE, "%u", usr_id);
            ssize_t  accumulated_sz_bytes = json_integer_value(json_object_get(usr_upload_quota, &usr_id_str[0]));
            accumulated_sz_bytes += file_tot_sz_bytes;
            json_object_set_new(usr_upload_quota, &usr_id_str[0], json_integer(accumulated_sz_bytes));
            ssize_t  max_upld_kbytes = (accumulated_sz_bytes >> 10) + 1;
            json_t *quota = json_array();
            json_t *item = json_object();
            json_object_set_new(item, "app_code", json_integer(APP_CODE));
            json_object_set_new(item, "mat_code", json_integer(QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER));
            json_object_set_new(item, "maxnum", json_integer(max_upld_kbytes));
            json_array_append_new(quota, item);
            const char *codename_list[2] = {"upload_files", NULL};
            json_array_remove(header_kv_serials, (json_array_size(header_kv_serials) - 1));
            add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
            json_decref(quota);
        }
        json_array_foreach(chunkinfo, jdx, info) {
            const char *path   = json_string_value(json_object_get(info, "path"));
            const char *chksum = json_string_value(json_object_get(info, "checksum"));
            size_t  partnum = (size_t) json_integer_value(json_object_get(info, "part"));
            usrarg_t  cb_arg = {.resp_code=200, .part=partnum, .f_chksum=chksum,
                .filepath=path, .upld_req_ref=upld_req};
            char url[128] = {0};
            sprintf(&url[0], "https://%s:%d%s?req_seq=%d&part=%d", "localhost", 8010, "/upload/multipart/part",
                    req_seq, cb_arg.part );
            setup_data.url = &url[0];
            setup_data.upload_filepaths.entries[0] = (char *) cb_arg.filepath;
            run_client_request(&setup_data, test_verify__app_server_response, (void *)&cb_arg);
        } // end of inner loop
    } // end of outer loop
done:
    json_decref(files_info);
    json_decref(header_kv_serials);
    json_decref(usr_upload_quota);
    if(_itest_filechunk_metadata)
        free(_itest_filechunk_metadata);
} // end of api_test_upload_part__multichunk_outoforder


TestSuite *api_upload_part_tests(json_t *root_cfg)
{
    json_t  *fchunk_cfg = json_object_get(json_object_get(root_cfg, "test"), "file_chunk");
    const char *metadata_fname = json_string_value(json_object_get(fchunk_cfg, "output_metadata"));
    const char *base_folder = json_string_value(json_object_get(fchunk_cfg, "base_folder"));
    size_t metadata_fname_sz = strlen(metadata_fname);
    size_t base_folder_sz    = strlen(base_folder);
    size_t  _meta_filepath_sz = metadata_fname_sz + base_folder_sz + 2;
    _itest_filechunk_metadata = calloc(_meta_filepath_sz, sizeof(char));
    size_t  nwrite = snprintf(_itest_filechunk_metadata, _meta_filepath_sz, "%s/%s",
            base_folder, metadata_fname);
    assert(nwrite < _meta_filepath_sz);
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test_upload_part__missing_auth_token);
    add_test(suite, api_test_upload_part__singlechunk_ok);
    add_test(suite, api_test_upload_part__uri_error);
    add_test(suite, api_test_upload_part__invalid_req);
    add_test(suite, api_test_upload_part__quota_exceed);
    add_test(suite, api_test_upload_part__multichunk_outoforder);
    return suite;
}
