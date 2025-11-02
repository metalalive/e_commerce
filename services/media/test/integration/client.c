#include <stdlib.h>
#include <string.h>
#include "utils.h"
#include "../test/integration/test.h"

static size_t test_read_req_body_cb(char *buf, size_t sz, size_t nitems, void *usrdata) {
    int    fd = *(int *)usrdata;
    size_t max_buf_sz = sz * nitems;
    size_t nread = read(fd, buf, max_buf_sz);
    assert(max_buf_sz >= nread);
    return nread;
}

static size_t test_write_resp_cb(char *buf, size_t sz, size_t nmemb, void *usrdata) {
    int    fd = *(int *)usrdata;
    size_t max_buf_sz = sz * nmemb;
    size_t nwrite = write(fd, buf, max_buf_sz);
    assert(max_buf_sz >= nwrite);
    return nwrite;
}

static void setup_tls_client_request(CURL *handle, const char *sys_basepath) {
    // res = curl_easy_setopt(ez_handle, CURLOPT_SSLKEY, "media/data/certs/test/ca.private.key");
#define RUNNER(fullpath) curl_easy_setopt(handle, CURLOPT_CAPATH, fullpath)
    CURLcode res = PATH_CONCAT_THEN_RUN(sys_basepath, "media/data/certs/test/ca.crt", RUNNER);
#undef RUNNER
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

static void setup_client_request(
    CURL *handle, const char *sys_basepath, test_setup_priv_t *privdata, test_setup_pub_t *pubdata
) {
    const char *api_host_domain = getenv("API_HOST"), *api_port = getenv("API_PORT"),
               *timeout_secs_str = getenv("API_TIMEOUT_SECONDS");

    CURLcode res;
    json_t  *hdr_kv = NULL;
    size_t   req_body_len = 0;
    int      idx = 0, default_timeout_secs = (int)strtol(timeout_secs_str, NULL, 10);
    json_array_foreach(pubdata->headers, idx, hdr_kv) {
        assert(json_is_string(hdr_kv));
        privdata->headers = curl_slist_append(privdata->headers, json_string_value(hdr_kv));
    }
    res = curl_easy_setopt(handle, CURLOPT_HTTPHEADER, privdata->headers);
    assert_that(res, is_equal_to(CURLE_OK));
    res = curl_easy_setopt(handle, CURLOPT_VERBOSE, (long)pubdata->verbose);
    assert_that(res, is_equal_to(CURLE_OK));
    int timeout_sec = pubdata->http_timeout_sec;
    if (timeout_sec == 0)
        timeout_sec = default_timeout_secs;
    res = curl_easy_setopt(handle, CURLOPT_TIMEOUT, (long)timeout_sec);
    assert_that(res, is_equal_to(CURLE_OK));
    {
#define URL_FMT_PATTERN "https://%s:%s%s"
        size_t m_sz = strlen(api_host_domain) + strlen(api_port) + strlen(pubdata->url_rel_ref) +
                      sizeof(URL_FMT_PATTERN) + 1;
        char   merged[m_sz];
        size_t wr_sz =
            snprintf(merged, m_sz, URL_FMT_PATTERN, api_host_domain, api_port, pubdata->url_rel_ref);
        assert(wr_sz <= (m_sz - 1));
        res = curl_easy_setopt(handle, CURLOPT_URL, merged);
#undef URL_FMT_PATTERN
    }
    assert_that(res, is_equal_to(CURLE_OK));
    for (idx = 0; idx < pubdata->upload_filepaths.size; idx++) {
        curl_mimepart *field = NULL;
        field = curl_mime_addpart(privdata->form); // fill in data-upload field
        curl_mime_name(field, "sendfile");
        const char *final_upld_fpath = pubdata->upload_filepaths.entries[idx];
        if (final_upld_fpath[0] == '/') {
            res = curl_mime_filedata(field, final_upld_fpath);
        } else {
#define RUNNER(fullpath) curl_mime_filedata(field, fullpath)
            res = PATH_CONCAT_THEN_RUN(sys_basepath, final_upld_fpath, RUNNER);
        }
        assert_that(res, is_equal_to(CURLE_OK));
        //// field = curl_mime_addpart(privdata->form); // fill in filename field
        //// curl_mime_name(field, "filename");
        //// curl_mime_data(field, "other info", CURL_ZERO_TERMINATED);
    }
    if (pubdata->req_body.serial_txt) {
        req_body_len = strlen(pubdata->req_body.serial_txt);
        write(privdata->fds.req_body, pubdata->req_body.serial_txt, req_body_len);
        lseek(privdata->fds.req_body, 0, SEEK_SET);
    } else if (pubdata->req_body.src_filepath) {
        const char *final_reqbody_fpath = pubdata->req_body.src_filepath;
#define BUF_SZ 128
        req_body_len = 0;
        size_t nread = 0;
        char   buf[BUF_SZ];
        int    fd_in = -1;
        if (final_reqbody_fpath[0] == '/') {
            fd_in = open(final_reqbody_fpath, O_RDONLY);
        } else {
#define RUNNER(fullpath) open(fullpath, O_RDONLY)
            fd_in = PATH_CONCAT_THEN_RUN(sys_basepath, final_reqbody_fpath, RUNNER);
#undef RUNNER
        }
        assert(fd_in > 0);
        while ((nread = read(fd_in, &buf[0], BUF_SZ)) > 0) {
            req_body_len += nread;
            write(privdata->fds.req_body, &buf[0], nread);
        }
        assert(nread == 0); // end of file
        close(fd_in);
        lseek(privdata->fds.req_body, 0, SEEK_SET);
#undef BUF_SZ
    }
    if (pubdata->req_body.serial_txt || pubdata->req_body.src_filepath) {
        res = curl_easy_setopt(handle, CURLOPT_READFUNCTION, test_read_req_body_cb);
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_READDATA, (void *)&privdata->fds.req_body);
        assert_that(res, is_equal_to(CURLE_OK));
    }
    if (strcmp(pubdata->method, "POST") == 0) {
        if (pubdata->upload_filepaths.size > 0) {
            res = curl_easy_setopt(handle, CURLOPT_MIMEPOST, privdata->form);
        } else {
            res = curl_easy_setopt(handle, CURLOPT_POST, 1L);
        } // multipart upload does NOT work if traditional POST request is also enabled
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_POSTFIELDSIZE, (long)req_body_len);
        assert_that(res, is_equal_to(CURLE_OK));
        if (req_body_len > 0) {
            res = curl_easy_setopt(handle, CURLOPT_POSTFIELDS, NULL);
            assert_that(res, is_equal_to(CURLE_OK));
        }
    } else {
        res = curl_easy_setopt(handle, CURLOPT_CUSTOMREQUEST, pubdata->method);
        assert_that(res, is_equal_to(CURLE_OK));
        res = curl_easy_setopt(handle, CURLOPT_INFILESIZE, (long)req_body_len);
        assert_that(res, is_equal_to(CURLE_OK));
        if (req_body_len > 0) {
            res = curl_easy_setopt(handle, CURLOPT_UPLOAD, 1L);
            assert_that(res, is_equal_to(CURLE_OK));
        }
    }
    // tell the handle NOT to include headers in response body, in order to separate from response
    // headers
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

void run_client_request(test_setup_pub_t *pubdata, test_verify_cb_t verify_cb, void *cb_arg) {
    const char *sys_basepath = getenv("SYS_BASE_PATH");
    assert(pubdata);
    assert(verify_cb);
    curl_mime *form = NULL;
    CURL      *ez_handle = curl_easy_init();
    CURLcode   res;
    assert(ez_handle);
    char tmpfile_path[3][80] = {
        "./log/media-itest-req-body-XXXXXX", "./log/media-itest-resp-hdr-XXXXXX",
        "./log/media-itest-resp-body-XXXXXX"
    };
    for (int idx = 0; idx < 3; idx++) {
#define RUNNER(fullpath) strcpy(tmpfile_path[idx], fullpath)
        PATH_CONCAT_THEN_RUN(sys_basepath, tmpfile_path[idx], RUNNER);
#undef RUNNER
    }
    if (pubdata->upload_filepaths.size > 0) {
        form = curl_mime_init(ez_handle);
    }
    test_setup_priv_t privdata = {
        .headers = NULL,
        .form = form,
        .expect_resp_code = pubdata->expect_resp_code,
        // constant string argument will cause SegFault
        .fds =
            {.req_body = mkstemp(&tmpfile_path[0][0]),
             .resp_hdr = mkstemp(&tmpfile_path[1][0]),
             .resp_body = mkstemp(&tmpfile_path[2][0])}
    };
    setup_client_request(ez_handle, sys_basepath, &privdata, pubdata);
    setup_tls_client_request(ez_handle, sys_basepath);
    res = curl_easy_perform(ez_handle); // send synchronous HTTP request
    assert_that(res, is_equal_to(CURLE_OK));
    lseek(privdata.fds.resp_body, 0, SEEK_SET);
    lseek(privdata.fds.resp_hdr, 0, SEEK_SET);
    verify_cb(ez_handle, &privdata, cb_arg);
    // ----- de-init -----
    close(privdata.fds.req_body);
    close(privdata.fds.resp_body);
    close(privdata.fds.resp_hdr);
    // delete immediately as soon as there is no file descriptor pointing to the temp file
    unlink(&tmpfile_path[0][0]);
    unlink(&tmpfile_path[1][0]);
    unlink(&tmpfile_path[2][0]);
    curl_slist_free_all(privdata.headers);
    if (form) {
        curl_mime_free(form);
        form = NULL;
    }
    curl_easy_cleanup(ez_handle);
} // end of run_client_request
