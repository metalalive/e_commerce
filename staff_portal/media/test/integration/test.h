#ifndef MEDIA__INTEGRATION_TEST_H
#define MEDIA__INTEGRATION_TEST_H
#ifdef __cplusplus
extern "C" {
#endif

#include <cgreen/cgreen.h>
#include <curl/curl.h>
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

typedef void (*test_verify_cb_t)(CURL *, test_setup_priv_t *);

// declare & implementation in test/integration/auth.c
void init_mock_auth_server(void);
void deinit_mock_auth_server(void);
int gen_signed_access_token(unsigned int usr_id, json_t *perm_codes, json_t *quota, char **out);
int add_auth_token_to_http_header(json_t *headers_kv_raw, const char **codename_list);
void api_test_common_auth_token_fail(test_setup_pub_t *setup_data);
void api_test_common_permission_check_fail(test_setup_pub_t *setup_data);

// declare & implementation in test/integration/client.c
void run_client_request(test_setup_pub_t *pubdata, test_verify_cb_t verify_cb);

// declare & implementation in test/integration/api/xxxx.c
TestSuite *api_initiate_multipart_upload_tests(void);
TestSuite *api_upload_part_tests(void);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEDIA__INTEGRATION_TEST_H