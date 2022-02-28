#include "../test/integration/test.h"


static size_t test_read_req_body_cb(char *buf, size_t sz, size_t nitems, void *usrdata)
{
   int fd = *(int *)usrdata;
   size_t max_buf_sz = sz * nitems;
   size_t nread = read(fd, buf, max_buf_sz);
   assert(max_buf_sz >= nread);
   return nread;
}

static size_t test_write_resp_cb(char *buf, size_t sz, size_t nmemb, void *usrdata)
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
    res = curl_easy_setopt(handle, CURLOPT_TIMEOUT, (long)5);
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



void run_client_request(test_setup_pub_t *pubdata, test_verify_cb_t verify_cb)
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

