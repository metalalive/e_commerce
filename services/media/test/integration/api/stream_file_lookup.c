#include <sys/stat.h>
#include <jansson.h>

#include "utils.h"
#include "transcoder/video/hls.h"
#include "../test/integration/test.h"

#define MAX_NUM_STARTED_STREAMS        3
#define ITEST_MAX_SZ_DOC_ID            100
#define ITEST_DETAIL_SZ__HLS_MST_PLIST sizeof(HLS_MASTER_PLAYLIST_FILENAME)
#define ITEST_DETAIL_SZ__HLS_KEY_REQ   sizeof(HLS_REQ_KEYFILE_LABEL)
#define ITEST_DETAIL_SZ__HLS_L2_PLIST  APP_TRANSCODED_VERSION_SIZE + 1 + sizeof(HLS_PLAYLIST_FILENAME)
#define ITEST_DETAIL_SZ__HLS_INIT_MAP  APP_TRANSCODED_VERSION_SIZE + 1 + sizeof(HLS_FMP4_FILENAME)
#define ITEST_DETAIL_SZ__HLS_DATA_SEG \
    APP_TRANSCODED_VERSION_SIZE + 1 + sizeof(HLS_SEGMENT_FILENAME_PREFIX) + \
        HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS
#define ITEST_MAX_SZ_DETAIL__HLS \
    MAX(MAX(MAX(ITEST_DETAIL_SZ__HLS_MST_PLIST, ITEST_DETAIL_SZ__HLS_KEY_REQ), \
            MAX(ITEST_DETAIL_SZ__HLS_L2_PLIST, ITEST_DETAIL_SZ__HLS_INIT_MAP)), \
        ITEST_DETAIL_SZ__HLS_DATA_SEG)

#define ITEST_STREAM_SEEK_URI "/file/stream/seek"

#define ITEST_URL_PATTERN \
    ITEST_STREAM_SEEK_URI "?" API_QPARAM_LABEL__STREAM_DOC_ID "=%s&" API_QPARAM_LABEL__DOC_DETAIL "=%s"

typedef void (*client_req_cb_t)(CURL *, test_setup_priv_t *, void *usr_arg);

typedef struct {
    json_t         *_upld_req; // for recording result of stream init
    int             _expect_resp_code;
    const char     *url;
    client_req_cb_t verify_cb;
} itest_usrarg_t;

extern json_t *_app_itest_active_upload_requests;

static json_t *_app_itest_started_streams[MAX_NUM_STARTED_STREAMS] = {0};

static char itest_tmpbuf_path[64] = {0};

static int _api_test__validate_url(char *in) {
    char *ptr = NULL;
    ptr = strstr(ptr, ITEST_STREAM_SEEK_URI);
    assert_that(ptr, is_not_null);
    if (!ptr)
        return -1;
    ptr = strstr(ptr, API_QPARAM_LABEL__DOC_DETAIL "=");
    assert_that(ptr, is_not_null);
    if (!ptr)
        return -1;
    return 0;
}

static void _api_test_filestream_seek_elm__send_request(itest_usrarg_t *usr_arg) {
    json_t *header_kv_serials = json_array();
#if 0
    const char *codename_list[1] = {NULL};
    uint32_t usr_id  = json_integer_value(json_object_get(usr_arg->_upld_req, "usr_id"));
    json_t *quota = json_array();
    add_auth_token_to_http_header(header_kv_serials, usr_id, codename_list, quota);
#endif
    test_setup_pub_t setup_data = {
        .method = "GET",
        .verbose = 0,
        .url_rel_ref = usr_arg->url,
        .req_body = {.serial_txt = NULL, .src_filepath = NULL},
        .upload_filepaths = {.size = 0, .capacity = 0, .entries = NULL},
        .headers = header_kv_serials
    };
    run_client_request(&setup_data, usr_arg->verify_cb, usr_arg);
    json_decref(header_kv_serials);
#if 0
    json_decref(quota);
#endif
} // end of  _api_test_filestream_seek_elm__send_request

static void test_verify_stream__hls_mst_plist(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
#define RD_BUF_SZ sizeof(ITEST_URL_PATTERN) + ITEST_MAX_SZ_DOC_ID + ITEST_MAX_SZ_DETAIL__HLS
    CURLcode        res;
    long            actual_resp_code = 0;
    int             ret = 0;
    itest_usrarg_t *usr_arg = _usr_arg;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
    if (actual_resp_code <= 0 || actual_resp_code >= 400)
        return;
    json_t *stream_item = json_object_get(usr_arg->_upld_req, "streaming");
    json_t *stream_privdata = json_object_get(stream_item, "_priv_data");
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    FILE *_resp_file = fdopen(privdata->fds.resp_body, "r");
    char  buf[RD_BUF_SZ] = {0};
    int   version = 0;
    ret = fscanf(_resp_file, "#EXT%3s", &buf[0]);
    assert_that(ret, is_equal_to(1));
    assert_that(&buf[0], is_equal_to_string("M3U"));
    ret = fscanf(_resp_file, "\n#EXT-X-VERSION:%d", &version);
    assert_that(ret, is_equal_to(1));
    assert_that(version, is_equal_to(7));
    while (!feof(_resp_file)) {
        ret = fscanf(_resp_file, "\n#EXT-X-STREAM-INF:%s", &buf[0]);
        if (ret == EOF)
            break;
        assert_that(ret, is_equal_to(1));
        if (ret != 1)
            continue;
        ret = fscanf(_resp_file, "%s", &buf[0]);
        assert_that(ret, is_equal_to(1));
        if (ret != 1)
            continue;
        char *l2_pl_url = &buf[0];
        if (!_api_test__validate_url(l2_pl_url)) // received url as key to private dataset
            json_object_set_new(stream_privdata, l2_pl_url, json_object());
    } // end of while-loop
    fclose(_resp_file);
#undef RD_BUF_SZ
} // end of test_verify_stream__hls_mst_plist

Ensure(api_test__filestream_seek__hls_mst_plist_ok) {
    json_t *upld_req = NULL;
    int     idx = 0, num_started_stream = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, upld_req) {
        if (num_started_stream >= MAX_NUM_STARTED_STREAMS)
            break;
        json_t *stream_item = json_object_get(upld_req, "streaming");
        if (!stream_item)
            continue;
        const char *st_type = json_string_value(json_object_get(stream_item, "type"));
        if (strncmp(st_type, "hls", 3))
            continue;
        _app_itest_started_streams[num_started_stream++] = upld_req;
        const char *doc_id = json_string_value(json_object_get(stream_item, API_QPARAM_LABEL__STREAM_DOC_ID));
        const char *detail_keyword =
            json_string_value(json_object_get(stream_item, API_QPARAM_LABEL__DOC_DETAIL));
        size_t url_sz = sizeof(ITEST_URL_PATTERN) + strlen(doc_id) + strlen(detail_keyword);
        char   _url[url_sz];
        size_t nwrite = snprintf(&_url[0], url_sz, ITEST_URL_PATTERN, doc_id, detail_keyword);
        assert(nwrite < url_sz);
        itest_usrarg_t usr_arg = {
            ._upld_req = upld_req,
            ._expect_resp_code = 200,
            .url = &_url[0],
            .verify_cb = test_verify_stream__hls_mst_plist
        };
        _api_test_filestream_seek_elm__send_request(&usr_arg);
    } // end of loop
} // end of  api_test__filestream_seek__hls_mst_plist_ok

static void test_verify_stream__hls_lvl2_plist(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
#define RD_BUF_SZ sizeof(ITEST_URL_PATTERN) + ITEST_MAX_SZ_DOC_ID + ITEST_MAX_SZ_DETAIL__HLS
    CURLcode        res;
    long            actual_resp_code = 0;
    int             ret = 0;
    itest_usrarg_t *usr_arg = _usr_arg;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
    if (actual_resp_code <= 0 || actual_resp_code >= 400)
        return;
    json_t *stream_item = json_object_get(usr_arg->_upld_req, "streaming");
    json_t *stream_privdata = json_object_get(stream_item, "_priv_data");
    json_t *l2_pl_item = json_object_get(stream_privdata, usr_arg->url);
    assert_that(l2_pl_item, is_not_null);
    if (!l2_pl_item)
        return;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    FILE *_resp_file = fdopen(privdata->fds.resp_body, "r");
    char  buf[RD_BUF_SZ] = {0};
    ret = fscanf(_resp_file, "#EXT%s", &buf[0]); // M3U
    assert_that(ret, is_equal_to(1));
    ret = fscanf(_resp_file, "\n#EXT-X-VERSION:%s", &buf[0]);
    assert_that(ret, is_equal_to(1));
    ret = fscanf(_resp_file, "\n#EXT-X-TARGETDURATION:%s", &buf[0]);
    assert_that(ret, is_equal_to(1));
    ret = fscanf(_resp_file, "\n#EXT-X-MEDIA-SEQUENCE:%s", &buf[0]);
    assert_that(ret, is_equal_to(1));
    ret = fscanf(_resp_file, "\n#EXT-X-PLAYLIST-TYPE:%s", &buf[0]);
    assert_that(ret, is_equal_to(1));
    { // extract url for acquiring key
#define IV_HEX_SZ (HLS__NBYTES_IV << 1)
        int  nbits = 0;
        char iv_hex[IV_HEX_SZ + 1] = {0};
        ret = fscanf(
            _resp_file, "\n#EXT-X-KEY:METHOD=AES-%d,URI=\"%[^\"]\",IV=0x%[abcdefABCDEF0123456789]", &nbits,
            &buf[0], &iv_hex[0]
        );
        assert_that(ret, is_equal_to(3));
        if (ret != 3)
            goto done;
        char *url_keyreq = &buf[0];
        if (!_api_test__validate_url(url_keyreq))
            json_object_set_new(l2_pl_item, "key", json_string(url_keyreq));
#undef IV_HEX_SZ
    }
    { // extract url for HLS initialization map
        ret = fscanf(_resp_file, "\n#EXT-X-MAP:URI=\"%[^\"]\"", &buf[0]);
        assert_that(ret, is_equal_to(1));
        if (ret != 1)
            goto done;
        char *url_initmap = &buf[0];
        if (!_api_test__validate_url(url_initmap))
            json_object_set_new(l2_pl_item, "init_map", json_string(url_initmap));
    }
    json_t *_segments = json_array();
    json_object_set_new(l2_pl_item, "segments", _segments);
    while (!feof(_resp_file)) { // extract url for each HLS segment
        ret = fscanf(_resp_file, "\n#EXTINF:%s", &buf[0]);
        if (ret == EOF)
            break;
        assert_that(ret, is_equal_to(1)); // skip duration of each segment
        if (ret == 0)
            continue;
        ret = fscanf(_resp_file, "\n%s", &buf[0]);
        char *url_dataseg = &buf[0];
        if (!_api_test__validate_url(url_dataseg))
            json_array_append_new(_segments, json_string(url_dataseg));
    } // end of loop
    assert_that(json_array_size(_segments), is_greater_than(0));
done:
    fclose(_resp_file);
#undef RD_BUF_SZ
} // end of  test_verify_stream__hls_lvl2_plist

Ensure(api_test__filestream_seek__hls_lvl2_plist_ok) {
    json_t *upld_req = NULL;
    for (int idx = 0; _app_itest_started_streams[idx] && (idx < MAX_NUM_STARTED_STREAMS); idx++) {
        upld_req = _app_itest_started_streams[idx];
        json_t     *stream_item = json_object_get(upld_req, "streaming");
        json_t     *stream_privdata = json_object_get(stream_item, "_priv_data");
        const char *url = NULL;
        json_t     *item = NULL;
        json_object_foreach(stream_privdata, url, item) {
            itest_usrarg_t usr_arg = {
                ._upld_req = upld_req,
                ._expect_resp_code = 200,
                .url = url,
                .verify_cb = test_verify_stream__hls_lvl2_plist
            };
            _api_test_filestream_seek_elm__send_request(&usr_arg);
        } // end of loop
    } // end of loop
} // end of  api_test__filestream_seek__hls_lvl2_plist_ok

static void test_verify_stream__hls_key_req(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
#define RD_BUF_SZ HLS__NBYTES_KEY + 1
    CURLcode        res;
    long            actual_resp_code = 0;
    itest_usrarg_t *usr_arg = _usr_arg;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
    if (actual_resp_code <= 0 || actual_resp_code >= 400)
        return;
    lseek(privdata->fds.resp_body, 0, SEEK_SET);
    char   buf[RD_BUF_SZ] = {0};
    size_t nread = read(privdata->fds.resp_body, &buf[0], RD_BUF_SZ);
    assert_that(nread, is_equal_to(HLS__NBYTES_KEY));
    // verify received key octet
    uint32_t   _usr_id = json_integer_value(json_object_get(usr_arg->_upld_req, "usr_id"));
    uint32_t   _upld_req_id = json_integer_value(json_object_get(usr_arg->_upld_req, "req_seq"));
#define PATH_PATTERN "%s/%d/%08x/%s"
    size_t path_sz = sizeof(PATH_PATTERN) + strlen(itest_tmpbuf_path) + USR_ID_STR_SIZE +
                     UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(HLS_CRYPTO_KEY_FILENAME);
    char   path[path_sz];
    size_t nwrite = snprintf(
        &path[0], path_sz, PATH_PATTERN, itest_tmpbuf_path, _usr_id, _upld_req_id, HLS_CRYPTO_KEY_FILENAME
    );
    assert(path_sz > nwrite);
    json_t     *keyinfo = json_load_file(&path[0], 0, NULL), *item = NULL, *keyitem = NULL;
    const char *id = NULL;
    uint8_t     recv_key_valid = 0;
    json_object_foreach(keyinfo, id, item) {
        keyitem = json_object_get(item, "key");
        const char *key_hex = json_string_value(json_object_get(keyitem, "data"));
        size_t      key_octet_sz = json_integer_value(json_object_get(keyitem, "nbytes"));
        size_t      key_hex_sz = strlen(key_hex);
        char        key_octets[key_octet_sz + 1];
        int         err = app_hexstr_to_chararray(&key_octets[0], key_octet_sz, key_hex, key_hex_sz);
        assert_that(err, is_equal_to(0));
        recv_key_valid = !strncmp(&key_octets[0], &buf[0], HLS__NBYTES_KEY);
        if (recv_key_valid)
            break;
    } // end of loop
    assert_that(recv_key_valid, is_equal_to(1));
    json_decref(keyinfo);
#undef PATH_PATTERN
#undef RD_BUF_SZ
} // end of test_verify_stream__hls_key_req

Ensure(api_test__filestream_seek__hls_key_req_ok
) { // TODO, will be non-cacheable and perform authorization check
    json_t *upld_req = NULL;
    int     idx = 0;
    for (idx = 0; idx < MAX_NUM_STARTED_STREAMS; idx++) {
        upld_req = _app_itest_started_streams[idx];
        if (!upld_req)
            break;
        json_t *stream_item = json_object_get(upld_req, "streaming");
        json_t *stream_privdata = json_object_get(stream_item, "_priv_data");
        assert_that(json_object_size(stream_privdata), is_greater_than(0));
        const char *url = NULL;
        json_t     *l2_pl_item = NULL;
        uint8_t     req_sent = 0;
        json_object_foreach(stream_privdata, url, l2_pl_item) {
            const char *url_keyreq = json_string_value(json_object_get(l2_pl_item, "key"));
            if (!url_keyreq)
                continue;
            itest_usrarg_t usr_arg = {
                ._upld_req = upld_req,
                ._expect_resp_code = 200,
                .url = url_keyreq,
                .verify_cb = test_verify_stream__hls_key_req
            };
            _api_test_filestream_seek_elm__send_request(&usr_arg);
            req_sent = 1;
            break; // currently all versions of a HLS video request the same key
        } // end of loop
        if (!req_sent)
            fprintf(stderr, "[itest][stream_idx] line:%d, skip to find key at index %d \r\n", __LINE__, idx);
    } // end of loop
} // end of  api_test__filestream_seek__hls_key_req_ok

static void test_verify_stream__hls_segment(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg) {
    CURLcode        res;
    long            actual_resp_code = 0;
    int             ret = 0;
    itest_usrarg_t *usr_arg = _usr_arg;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_arg->_expect_resp_code));
    if (actual_resp_code <= 0 || actual_resp_code >= 400)
        return;
    char  *detail = NULL;
    size_t detail_sz = 0;
    {
        char *ptr1 = strstr(usr_arg->url, API_QPARAM_LABEL__DOC_DETAIL "=");
        assert_that(ptr1, is_not_null);
        if (!ptr1)
            return;
        ptr1 += sizeof(API_QPARAM_LABEL__DOC_DETAIL "=") - 1;
        char *ptr2 = strstr(ptr1, "&");
        if (ptr2) {
            detail_sz = (size_t)ptr2 - (size_t)ptr1;
        } else {
            detail_sz = strlen(ptr1);
        }
        detail = calloc(detail_sz + 1, sizeof(char));
        memcpy(detail, ptr1, detail_sz);
    }
    json_t     *stream_item = json_object_get(usr_arg->_upld_req, "streaming");
    const char *doc_id = json_string_value(json_object_get(stream_item, API_QPARAM_LABEL__STREAM_DOC_ID));
#define PATH_PATTERN "%s/%s/%s/%s"
    size_t path_sz = sizeof(PATH_PATTERN) + strlen(itest_tmpbuf_path) + strlen(doc_id) +
                     sizeof(ATFP_CACHED_FILE_FOLDERNAME) + detail_sz;
    char   path[path_sz];
    size_t nwrite = snprintf(
        &path[0], path_sz, PATH_PATTERN, itest_tmpbuf_path, ATFP_CACHED_FILE_FOLDERNAME, doc_id, detail
    );
    assert(path_sz > nwrite);
#undef PATH_PATTERN
    free(detail);
    struct stat actual_f_stats = {0}, expect_f_stats = {0};
    ret = fstat(privdata->fds.resp_body, &actual_f_stats);
    assert_that(ret, is_equal_to(0));
    ret = stat(&path[0], &expect_f_stats);
    assert_that(ret, is_equal_to(0));
    assert_that(actual_f_stats.st_size, is_greater_than(0));
    assert_that(actual_f_stats.st_size, is_equal_to(expect_f_stats.st_size));
    if (actual_f_stats.st_size == expect_f_stats.st_size) {
        int exp_fd = open(&path[0], O_RDONLY, S_IRUSR), act_fd = privdata->fds.resp_body;
        lseek(act_fd, 0, SEEK_SET);
#define BUF_SZ 15
        char expect_content[BUF_SZ + 1] = {0}, actual_content[BUF_SZ + 1] = {0};
        read(exp_fd, &expect_content[0], BUF_SZ * sizeof(char));
        read(act_fd, &actual_content[0], BUF_SZ * sizeof(char));
#undef BUF_SZ
        assert_that(&actual_content[0], is_equal_to_string(&expect_content[0]));
        close(exp_fd);
    }
} // end of  test_verify_stream__hls_segment

Ensure(api_test__filestream_seek__hls_segment_ok) {
    json_t *upld_req = NULL;
    int     idx = 0;
    for (idx = 0; idx < MAX_NUM_STARTED_STREAMS; idx++) {
        upld_req = _app_itest_started_streams[idx];
        if (!upld_req)
            break;
        json_t *stream_item = json_object_get(upld_req, "streaming");
        json_t *stream_privdata = json_object_get(stream_item, "_priv_data");
        assert_that(json_object_size(stream_privdata), is_greater_than(0));
        const char *url = NULL;
        json_t     *l2_pl_item = NULL;
        uint8_t     req_sent = 0;
        json_object_foreach(stream_privdata, url, l2_pl_item) {
            const char *url_initmap = json_string_value(json_object_get(l2_pl_item, "init_map"));
            if (!url_initmap)
                continue;
            itest_usrarg_t usr_arg = {
                ._upld_req = upld_req,
                ._expect_resp_code = 200,
                .url = url_initmap,
                .verify_cb = test_verify_stream__hls_segment
            };
            _api_test_filestream_seek_elm__send_request(&usr_arg);
            req_sent = 1;
            json_t     *_segments = json_object_get(l2_pl_item, "segments");
            const char *url_dataseg0 = json_string_value(json_array_get(_segments, 0));
            usr_arg.url = url_dataseg0;
            _api_test_filestream_seek_elm__send_request(&usr_arg);
            break; // currently all versions of a HLS video request the same key
        } // end of loop
        if (!req_sent)
            fprintf(stderr, "[itest][stream_idx] line:%d, skip to find key at index %d \r\n", __LINE__, idx);
    } // end of loop
} // end of  api_test__filestream_seek__hls_segment_ok

Ensure(api_test__filestream_seek__hls_nonexist_detail) {
    json_t *upld_req = NULL;
    int     idx = 0, num_started_stream = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, upld_req) {
        if (num_started_stream >= MAX_NUM_STARTED_STREAMS)
            break;
        json_t *stream_item = json_object_get(upld_req, "streaming");
        if (!stream_item)
            continue;
        const char *st_type = json_string_value(json_object_get(stream_item, "type"));
        if (strncmp(st_type, "hls", 3))
            continue;
        const char *doc_id = json_string_value(json_object_get(stream_item, API_QPARAM_LABEL__STREAM_DOC_ID));
        const char *detail_keyword = "random/invalid/file";
        size_t      url_sz = sizeof(ITEST_URL_PATTERN) + strlen(doc_id) + strlen(detail_keyword);
        char        _url[url_sz];
        size_t      nwrite = snprintf(&_url[0], url_sz, ITEST_URL_PATTERN, doc_id, detail_keyword);
        assert(nwrite < url_sz);
        itest_usrarg_t usr_arg = {
            ._upld_req = upld_req,
            ._expect_resp_code = 400,
            .url = &_url[0],
            .verify_cb = test_verify_stream__hls_mst_plist
        };
        _api_test_filestream_seek_elm__send_request(&usr_arg);
    } // end of loop
} // end of  api_test__filestream_seek__hls_nonexist_detail

TestSuite *api_file_stream_seek_elm_tests(json_t *root_cfg) {
    json_t     *tmpbuf_cfg = json_object_get(root_cfg, "tmp_buf");
    const char *tmpbuf_path = json_string_value(json_object_get(tmpbuf_cfg, "path"));
    const char *sys_basepath = getenv("SYS_BASE_PATH");
#define RUNNER(fullpath) strcpy(itest_tmpbuf_path, fullpath)
    PATH_CONCAT_THEN_RUN(sys_basepath, tmpbuf_path, RUNNER);
#undef RUNNER
    TestSuite *suite = create_test_suite();
    add_test(suite, api_test__filestream_seek__hls_mst_plist_ok);
    add_test(suite, api_test__filestream_seek__hls_lvl2_plist_ok);
    add_test(suite, api_test__filestream_seek__hls_key_req_ok);
    add_test(suite, api_test__filestream_seek__hls_segment_ok);
    add_test(suite, api_test__filestream_seek__hls_nonexist_detail);
    return suite;
}
#undef URL_PATTERN
