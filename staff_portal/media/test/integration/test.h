#ifndef MEDIA__INTEGRATION_TEST_H
#define MEDIA__INTEGRATION_TEST_H
#ifdef __cplusplus
extern "C" {
#endif

#include <cgreen/cgreen.h>
#include <curl/curl.h>
#include "app_cfg.h"

typedef struct {
    const char *cfg_file_path;
    const char *exe_path;
} test_init_app_data_t;

#define  ITEST_NUM_UPLD_REQS__FOR_ERR_CHK   2
#define  ITEST_UPLD_REQ__SAME_USER__IDX_1   (ITEST_NUM_UPLD_REQS__FOR_ERR_CHK + 0)
#define  ITEST_UPLD_REQ__SAME_USER__IDX_2   (ITEST_NUM_UPLD_REQS__FOR_ERR_CHK + 2)

typedef struct {
    const char  *url;
    const char  *method;
    struct {
        const char  *serial_txt;
        const char  *src_filepath;
    } req_body;
    json_t      *headers; // array of JSON objects (key-value pairs)
    H2O_VECTOR(char *) upload_filepaths;
    uint32_t  expect_resp_code;
    int       verbose;
    int  http_timeout_sec;
} test_setup_pub_t;

typedef struct {
    struct curl_slist *headers;
    struct {
        int req_body; 
        int resp_hdr; 
        int resp_body;
    } fds;
    curl_mime   *form;
    uint32_t  expect_resp_code;
} test_setup_priv_t;

typedef void (*test_verify_cb_t)(CURL *, test_setup_priv_t *, void *cb_arg);

// declare & implementation in test/integration/auth.c
void init_mock_auth_server(const char *tmpfile_path);
void deinit_mock_auth_server(void);
int gen_signed_access_token(unsigned int usr_id, json_t *perm_codes, json_t *quota, char **out);
int add_auth_token_to_http_header(json_t *headers_kv_raw, unsigned int usr_id, const char **codename_list, json_t *quota);
void api_test_common_auth_token_fail(test_setup_pub_t *setup_data);
void api_test_common_permission_check_fail(test_setup_pub_t *setup_data);

// declare & implementation in test/integration/client.c
void run_client_request(test_setup_pub_t *pubdata, test_verify_cb_t verify_cb, void *cb_arg);

// declare & implementation in test/integration/api/xxxx.c
TestSuite *api_initiate_multipart_upload_tests(json_t *root_cfg);
TestSuite *api_upload_part_tests(json_t *root_cfg);
TestSuite *api_complete_multipart_upload_tests(void);
TestSuite *api_file_acl_tests(void);
TestSuite *api_start_transcoding_file_tests(void);
TestSuite *api_start_transcoding_file_v2_tests(void);
TestSuite *api_monitor_job_progress_tests(void);
TestSuite *api_file_streaming_init_tests(void);
TestSuite *api_file_stream_seek_elm_tests(void);
void api_deinitiate_multipart_upload_tests(void);

// declare & implementation in test/integration/rpc_consumer.c
void  itest_rpc_usermgt__setup_usr_ids(uint32_t *in, size_t in_sz, uint8_t _no_resp);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEDIA__INTEGRATION_TEST_H
