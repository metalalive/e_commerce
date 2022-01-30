#include <cgreen/cgreen.h>
#include <curl/curl.h>
#include <jansson.h>
#include "app.h"


typedef struct {
    const char *cfg_file_path;
    const char *exe_path;
} test_init_app_data_t;

typedef struct {
    const char  *url;
    const char  *method;
    struct {
        const char  *serial_txt;
        const char  *src_filepath;
    } req_body;
    json_t      *headers; // array of JSON objects (key-value pairs)
    H2O_VECTOR(char *) upload_filepaths;
    int          verbose;
} test_setup_pub_t;

typedef struct {
    struct curl_slist *headers;
    struct {
        int req_body; 
        int resp_hdr; 
        int resp_body;
    } fds;
    curl_mime   *form;
} test_setup_priv_t;

size_t test_read_req_body_cb(char *buf, size_t sz, size_t nitems, void *usrdata)
{
   int fd = *(int *)usrdata;
   size_t max_buf_sz = sz * nitems;
   size_t nread = read(fd, buf, max_buf_sz);
   assert(max_buf_sz >= nread);
   return nread;
}

size_t test_write_resp_cb(char *buf, size_t sz, size_t nmemb, void *usrdata)
{
   int fd = *(int *)usrdata;
   size_t max_buf_sz = sz * nmemb;
   size_t nwrite = write(fd, buf, max_buf_sz);
   assert(max_buf_sz >= nwrite);
   return nwrite;
}


static void setup_tls_client_request(CURL *handle)
{
    CURLcode res;
    // res = curl_easy_setopt(ez_handle, CURLOPT_SSLKEY, "media/data/certs/test/ca.private.key");
    res = curl_easy_setopt(handle, CURLOPT_CAPATH, "media/data/certs/test/ca.crt");
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_SSLCERTTYPE, "PEM");
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_SSL_VERIFYPEER, 0L);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_SSL_ENABLE_ALPN, 1L);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_2TLS);
    assert_that(res, is_equal_to(CURLE_OK));
    // res = curl_easy_setopt(handle, , );
}

static void setup_client_request(CURL *handle, test_setup_priv_t *privdata, test_setup_pub_t *pubdata)
{
    CURLcode res;
    json_t *hdr_kv = NULL;
    size_t req_body_len = 0;
    int idx = 0;
    json_array_foreach(pubdata->headers, idx, hdr_kv) {
        assert(json_is_string(hdr_kv));
        privdata->headers = curl_slist_append(privdata->headers, json_string_value(hdr_kv));
    }
    res = curl_easy_setopt(handle, CURLOPT_HTTPHEADER, privdata->headers);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_VERBOSE, (long)pubdata->verbose);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_URL, pubdata->url);
    assert_that(res, is_equal_to(CURLE_OK));
    for(idx = 0; idx < pubdata->upload_filepaths.size; idx++) {
        curl_mimepart *field = NULL;
        field = curl_mime_addpart(privdata->form); // fill in data-upload field
        curl_mime_name(field, "sendfile");
        res = curl_mime_filedata(field, pubdata->upload_filepaths.entries[idx]);
        assert_that(res, is_equal_to(CURLE_OK));
        //// field = curl_mime_addpart(privdata->form); // fill in filename field
        //// curl_mime_name(field, "filename");
        //// curl_mime_data(field, "other info", CURL_ZERO_TERMINATED);
    }
    if(pubdata->req_body.serial_txt) {
        req_body_len = strlen(pubdata->req_body.serial_txt);
        write(privdata->fds.req_body, pubdata->req_body.serial_txt, req_body_len);
        lseek(privdata->fds.req_body, 0, SEEK_SET);
    } else if(pubdata->req_body.src_filepath) {
#define BUF_SZ 128
        req_body_len = 0;
        size_t nread = 0;
        char buf[BUF_SZ];
        int fd_in = open(pubdata->req_body.src_filepath, O_RDONLY);
        while ((nread = read(fd_in, &buf[0], BUF_SZ)) > 0) {
            req_body_len += nread;
            write(privdata->fds.req_body, &buf[0], nread);
        }
        assert(nread == 0); // end of file
        close(fd_in);
        lseek(privdata->fds.req_body, 0, SEEK_SET);
#undef BUF_SZ
    }
    if(pubdata->req_body.serial_txt || pubdata->req_body.src_filepath) {
        res = curl_easy_setopt(handle, CURLOPT_READFUNCTION, test_read_req_body_cb);
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_READDATA, (void *)&privdata->fds.req_body);
        assert_that(res, is_equal_to(CURLE_OK));
    }
    if(strcmp(pubdata->method, "POST") == 0) {
        if(pubdata->upload_filepaths.size > 0) {
            res = curl_easy_setopt(handle, CURLOPT_MIMEPOST, privdata->form);
        } else {
            res = curl_easy_setopt(handle, CURLOPT_POST, 1L);
        } // multipart upload does NOT work if traditional POST request is also enabled
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_POSTFIELDSIZE, (long)req_body_len);
        assert_that(res, is_equal_to(CURLE_OK));
        if(req_body_len > 0) {
            res = curl_easy_setopt(handle, CURLOPT_POSTFIELDS, NULL);
            assert_that(res, is_equal_to(CURLE_OK));
        }
    } else {
        res = curl_easy_setopt(handle, CURLOPT_CUSTOMREQUEST, pubdata->method);
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_INFILESIZE, (long)req_body_len);
        assert_that(res, is_equal_to(CURLE_OK));
        if(req_body_len > 0) {
            res = curl_easy_setopt(handle, CURLOPT_UPLOAD, 1L);
            assert_that(res, is_equal_to(CURLE_OK));
        }
    }
    // tell the handle NOT to include headers in response body, in order to separate from response headers
    res = curl_easy_setopt(handle, CURLOPT_HEADER, 0L);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_WRITEFUNCTION, test_write_resp_cb);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_WRITEDATA, (void *)&privdata->fds.resp_body);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_HEADERFUNCTION, test_write_resp_cb);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_HEADERDATA, (void *)&privdata->fds.resp_hdr);
    assert_that(res, is_equal_to(CURLE_OK));
} // end of setup_client_request


typedef void (*test_verify_cb_t)(CURL *, test_setup_priv_t *);

static void run_client_request(test_setup_pub_t *pubdata, test_verify_cb_t verify_cb)
{
    assert(pubdata);
    assert(verify_cb);
    curl_mime *form = NULL;
    CURL *ez_handle = curl_easy_init();
    CURLcode res;
    assert(ez_handle);
    char tmpfile_path[3][40] = {
        "./tmp/media_test_req_body_XXXXXX",
        "./tmp/media_test_resp_hdr_XXXXXX",
        "./tmp/media_test_resp_body_XXXXXX"
    };
    if(pubdata->upload_filepaths.size > 0) {
        form = curl_mime_init(ez_handle);
    }
    test_setup_priv_t privdata = {
        .headers = NULL,
        .form = form,
        .fds = { // constant string argument will cause SegFault
            .req_body  = mkstemp(&tmpfile_path[0][0]), 
            .resp_hdr  = mkstemp(&tmpfile_path[1][0]),
            .resp_body = mkstemp(&tmpfile_path[2][0])
        }
    };
    // delete immediately as soon as there is no file descriptor pointing to the temp file
    unlink(&tmpfile_path[0][0]);
    unlink(&tmpfile_path[1][0]);
    unlink(&tmpfile_path[2][0]);
    setup_client_request(ez_handle, &privdata, pubdata);
    setup_tls_client_request(ez_handle);
    res = curl_easy_perform(ez_handle); // send synchronous HTTP request
    assert_that(res, is_equal_to(CURLE_OK));
    lseek(privdata.fds.resp_body, 0, SEEK_SET);
    lseek(privdata.fds.resp_hdr,  0, SEEK_SET);
    verify_cb(ez_handle, &privdata);
    // ----- de-init -----
    close(privdata.fds.req_body);
    close(privdata.fds.resp_body);
    close(privdata.fds.resp_hdr);
    curl_slist_free_all(privdata.headers);
    if(form) {
        curl_mime_free(form);
        form = NULL;
    }
    curl_easy_cleanup(ez_handle);
} // end of run_client_request


static void test_verify__initiate_multipart_upload(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    // analyza response body
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    assert_that(resp_obj, is_not_equal_to(NULL));
    if(resp_obj) { // should return short-term token for upload request
        const char *access_token = json_string_value(json_object_get(resp_obj, "upld_id"));
        assert_that(access_token, is_not_null);
    }
    json_decref(resp_obj);
}


Ensure(api_initiate_multipart_upload_test) {
    char url[128] = {0};
    // the resource id client wants to claim, server may return auth failure if the user doesn't
    //  have access to modify the resource pointed by this ID
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/upload/multipart/initiate");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__initiate_multipart_upload);
}


static void test_verify__upload_part(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    int expect_part = 3;
    int actual_part = (int)json_integer_value(json_object_get(resp_obj, "part"));
    assert_that(expect_part, is_equal_to(actual_part));
    json_decref(resp_obj);
}


Ensure(api_upload_part_test) {
    char url[128] = {0};
    int expect_part = 3;
    sprintf(&url[0], "https://%s:%d%s?upload_id=%s&part=%d", "localhost",
            8010, "/upload/multipart/part", "1c037a57581e", expect_part);
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0, .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .headers = header_kv_serials
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 1);
    setup_data.upload_filepaths.entries[0] = "./tmp/test_file_chunk_0";
    setup_data.upload_filepaths.size = 1;
    run_client_request(&setup_data, test_verify__upload_part);
}


static void test_verify__complete_multipart_upload(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 201;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *expect_upld_id = "1c037a57581e";
    const char *actual_upld_id = json_string_value(json_object_get(resp_obj, "upload_id"));
    assert_that(expect_upld_id, is_equal_to_string(actual_upld_id));
    json_decref(resp_obj);
}


Ensure(api_complete_multipart_upload_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?resource_id=%s&upload_id=%s", "localhost",
            8010, "/upload/multipart/complete", "bMerI8f", "1c037a57581e");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__complete_multipart_upload);
}


static void test_verify__abort_multipart_upload(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_abort_multipart_upload_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?upload_id=%s", "localhost",
            8010, "/upload/multipart/abort", "1c037a57581e");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__abort_multipart_upload);
}


static void test_verify__single_chunk_upload(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 201;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *item = NULL;
    int idx = 0;
    json_array_foreach(resp_obj, idx, item) {
        const char *actual_resource_id = json_string_value(json_object_get(item, "resource_id"));
        const char *actual_file_name   = json_string_value(json_object_get(item, "file_name"));
        assert_that(actual_resource_id, is_not_null);
        assert_that(actual_file_name  , is_not_null);
    }
    json_decref(resp_obj);
}

Ensure(api_single_chunk_upload_test) {
    // this API endpoint accept multiple files in one flight
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?resource_id=%s,%s", "localhost",
            8010, "/upload", "bMerI8f", "8fQwhBj");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .headers = header_kv_serials
    };
    h2o_vector_reserve(NULL, &setup_data.upload_filepaths, 2);
    setup_data.upload_filepaths.entries[0] = "./tmp/test_file_chunk_0";
    setup_data.upload_filepaths.entries[1] = "./tmp/test_file_chunk_1";
    setup_data.upload_filepaths.size = 2;
    run_client_request(&setup_data, test_verify__single_chunk_upload);
}


static void test_verify__start_transcoding_file(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 202;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    const char *actual_job_id = json_string_value(json_object_get(resp_obj, "job"));
    assert_that(actual_job_id, is_not_null);
    json_decref(resp_obj);
}

Ensure(api_start_transcoding_file_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s", "localhost", 8010, "/file/transcode");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "POST", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath="./media/test/integration/examples/transcode_req_body.json"},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__start_transcoding_file);
}


static void test_verify__discard_ongoing_job(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_discard_ongoing_job_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/job", "1b2934ad4e2c9");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__discard_ongoing_job);
}


static void test_verify__monitor_job_progress(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
    json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
    json_t *actual_elm_streams = json_object_get(resp_obj, "elementary_streams");
    json_t *actual_outputs     = json_object_get(resp_obj, "outputs");
    assert_that(actual_elm_streams, is_not_null);
    assert_that(actual_outputs    , is_not_null);
    assert_that(json_is_array(actual_elm_streams), is_true);
    assert_that(json_is_array(actual_outputs    ), is_true);
    json_decref(resp_obj);
}

Ensure(api_monitor_job_progress_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/job", "1b2934ad4e2c9");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    test_setup_pub_t  setup_data = {
        .method = "GET", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__monitor_job_progress);
}


static void test_verify__fetch_entire_file(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_fetch_entire_file_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s&trncver=%s", "localhost", 8010,
            "/file", "1b2934ad4e2c9", "SD");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "GET", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__fetch_entire_file);
}

Ensure(api_get_next_media_segment_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s&trncver=%s", "localhost", 8010,
            "/file/playback", "1b2934ad4e2c9", "SD");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "GET", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__fetch_entire_file);
}


static void test_verify__discard_file(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 204;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_discard_file_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/file", "1b2934ad4e2c9");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "DELETE", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__discard_file);
}


static void test_verify__edit_file_acl(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_edit_file_acl_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/file/acl", "8gWs5oP");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json"));
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath="./media/test/integration/examples/edit_file_acl_req_body.json"},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__edit_file_acl);
}



static void test_verify__read_file_acl(CURL *handle, test_setup_priv_t *privdata)
{
    CURLcode res;
    long expect_resp_code = 200;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(expect_resp_code, is_equal_to(actual_resp_code));
}

Ensure(api_read_file_acl_test) {
    char url[128] = {0};
    sprintf(&url[0], "https://%s:%d%s?id=%s", "localhost", 8010, "/file/acl", "8gWs5oP");
    json_t *header_kv_serials = json_array();
    json_array_append_new(header_kv_serials, json_string("Accept:application/json"));
    json_array_append_new(header_kv_serials, json_string("Authorization: token 12r23t346y"));
    test_setup_pub_t  setup_data = {
        .method = "GET", .verbose = 0,  .url = &url[0],
        .req_body = {.serial_txt=NULL, .src_filepath=NULL},
        .upload_filepaths = {.size=0, .capacity=0, .entries=NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, test_verify__read_file_acl);
}

TestSuite *app_api_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_initiate_multipart_upload_test);
    add_test(suite, api_upload_part_test);
    add_test(suite, api_complete_multipart_upload_test);
    add_test(suite, api_abort_multipart_upload_test);
    add_test(suite, api_single_chunk_upload_test);
    add_test(suite, api_start_transcoding_file_test);
    add_test(suite, api_discard_ongoing_job_test);
    add_test(suite, api_monitor_job_progress_test);
    add_test(suite, api_fetch_entire_file_test);
    add_test(suite, api_get_next_media_segment_test);
    add_test(suite, api_discard_file_test);
    add_test(suite, api_edit_file_acl_test);
    add_test(suite, api_read_file_acl_test);
    return suite;
}

static void run_app_server(void *data) {
    test_init_app_data_t *data1 = (test_init_app_data_t *)data;
    start_application(data1->cfg_file_path, data1->exe_path);
} // end of run_app_server()


int main(int argc, char **argv) {
    test_init_app_data_t  init_app_data = {
        .cfg_file_path = "./media/settings/test.json",
        .exe_path = "./media/build/integration_test.out"
    };
    int result = 0;
    uv_thread_t app_tid = 0;
    result = uv_thread_create( &app_tid, run_app_server, (void *)&init_app_data );
    assert(result == 0);
    assert(app_tid > 0);
    TestSuite *suite = create_named_test_suite("media_app_integration_test");
    TestReporter *reporter = create_text_reporter();
    add_suite(suite, app_api_tests());
    curl_global_init(CURL_GLOBAL_DEFAULT);
    do {
        result = pthread_tryjoin_np(app_tid, NULL);
        if(result == 0) {
            fprintf(stderr, "[test] app server thread terminated due to some error\n");
            goto done;
        }
    } while(!app_server_ready());
    fprintf(stdout, "[test] curl version : %s \n", curl_version());
    fprintf(stdout, "[test] app server is ready, start integration test cases ...\n");
    if(argc > 1) {
        const char *test_name = argv[argc - 1];
        result = run_single_test(suite, test_name, reporter);
    } else {
        result = run_test_suite(suite, reporter);
    }
    pthread_kill(app_tid, SIGTERM);
    pthread_join(app_tid, NULL);
done:
    curl_global_cleanup();
    destroy_test_suite(suite);
    destroy_reporter(reporter);
    return result;
} // end of main()
