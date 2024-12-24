#include <sys/types.h>
#include <unistd.h>
#include <string.h>

#include "api/setup.h"
#include "../test/integration/test.h"

#define NUM_KEY_PAIRS 2

typedef struct {
    struct {
        jwks_t *privkey;
        jwks_t *pubkey;
    } store;
    struct {
        char *pubkey;
    } filepath;
    struct {
        int pubkey;
    } fd;
} mock_jwks_t;

static mock_jwks_t _mock_jwks = {0};

#define _API_MIDDLEWARE_CHAIN_get_jwks_pubkey  2, API_FINAL_HANDLER_get_jwks_pubkey, 1
#define _RESTAPI_PERM_CODES_get_jwks_pubkey    NULL


RESTAPI_ENDPOINT_HANDLER(get_jwks_pubkey, GET, self, req)
{
    int flags = 0;
    if(_mock_jwks.fd.pubkey) {
        h2o_iovec_t  mime_type = {.base = "application/json", .len = sizeof("application/json") - 1};
        return h2o_file_send(req, 200, "OK", _mock_jwks.filepath.pubkey,  mime_type, flags);
    } else {    // jwks file hasn't been prepared yet
        h2o_send_error_404(req, "JWKS internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
       return 0; 
    }
} // end of get_jwks_pubkey


static int gen_signed_access_token_helper(json_t *headers, json_t *claims, char **out) {
    int result = RHN_OK;
    jwt_t *jwt = NULL;
    jwk_t *privkey = NULL;
    result = r_jwt_init(&jwt);
    if(result != RHN_OK) { goto done; }
    result = r_jwt_set_full_header_json_t(jwt, headers);
    if(result != RHN_OK) { goto done; }
    result = r_jwt_set_full_claims_json_t(jwt, claims);
    if(result != RHN_OK) { goto done; }
    privkey = r_jwks_get_at(_mock_jwks.store.privkey, 0);
    if(!privkey) { goto done; }
    result = r_jwt_set_sign_alg(jwt, R_JWA_ALG_RS256);
    if(result != RHN_OK) { goto done; }
    // the sign function internally allocated space for new generated token,
    // which nust be freed after use.
    *out = r_jwt_serialize_signed(jwt, privkey, 0);
done:
    if(privkey) {
        r_jwk_free(privkey);
        privkey = NULL;
    }
    if(jwt) {
        r_jwt_free(jwt);
        jwt = NULL;
    }
    return result;
} // end of gen_signed_access_token_helper

int gen_signed_access_token(unsigned int usr_id, json_t *perm_codes, json_t *quota, char **out)
{
    int result = 0;
    if(!json_is_array(perm_codes) || !json_is_array(quota) || !out) {
        return result;
    }
    json_t *headers = json_object();
    json_t *claims  = json_object();
    time_t issued_time = time(NULL); // TODO, find more reliable way of reading current time in seconds
    time_t expiry_time = issued_time + 600; // 10 minutes available by default
    json_object_set(headers, "typ", json_string("JWT"));
    json_object_set(claims, "profile", json_integer(usr_id));
    json_object_set(claims, "iat", json_integer(issued_time));
    json_object_set(claims, "exp", json_integer(expiry_time));
    {
        json_t *audience = json_array();
        json_array_append(audience, json_string("service1"));
        json_array_append(audience, json_string("service2"));
        json_array_append(audience, json_string(APP_LABEL));
        json_array_append(audience, json_string("service3"));
        json_object_set(claims, "aud", audience);
    }
    json_object_set(claims, "perms", perm_codes);
    json_object_set(claims, "quota", quota);
    result = gen_signed_access_token_helper(headers, claims, out);
    json_decref(headers);
    json_decref(claims );
    return result;
} // end of gen_signed_access_token


int add_auth_token_to_http_header(json_t *headers_kv_raw, unsigned int usr_id, const char **codename_list, json_t *quota)
{
    assert(headers_kv_raw);
    assert(json_is_array(headers_kv_raw));
    assert(quota);
    assert(json_is_array(quota));
    assert(usr_id > 0);
    json_t *perm_codes = json_array();
    char *signed_access_token = NULL;
    char *auth_header_raw = NULL;
    for(int idx = 0; codename_list && codename_list[idx] ; idx++) {
        json_t *perm_code = json_object();
        json_object_set(perm_code, "app_code", json_integer(APP_CODE));
        json_object_set(perm_code, "codename", json_string(codename_list[idx]));
        json_array_append(perm_codes, perm_code);
    }
    int result = gen_signed_access_token(usr_id, perm_codes, quota, &signed_access_token);
    assert_that(result , is_equal_to(RHN_OK));
    assert_that(signed_access_token, is_not_null);
    const char *auth_header_pattern = "Authorization:Bearer %s";
    auth_header_raw = h2o_mem_alloc(strlen(signed_access_token) + strlen(auth_header_pattern));
    sprintf(auth_header_raw, auth_header_pattern, signed_access_token);
    json_array_append_new(headers_kv_raw, json_string(auth_header_raw));
    json_decref(perm_codes);
    free(signed_access_token);
    free(auth_header_raw);
    return 0;
} // end of add_auth_token_to_http_header


void init_mock_auth_server(const char *tmpfile_path) {
    unsigned int rsa_bits[NUM_KEY_PAIRS] = {2048, 3072}; // 256 / 384 bytes
    assert(r_jwks_init(&_mock_jwks.store.privkey) == RHN_OK);
    assert(r_jwks_init(&_mock_jwks.store.pubkey) == RHN_OK);
    for(int idx = 0; idx < NUM_KEY_PAIRS; idx++) {
        jwk_t *privkey = NULL, *pubkey = NULL;
        assert(r_jwk_init(&privkey) == RHN_OK);
        assert(r_jwk_init(&pubkey) == RHN_OK);
        assert(r_jwks_append_jwk(_mock_jwks.store.privkey, privkey) == RHN_OK);
        assert(r_jwks_append_jwk(_mock_jwks.store.pubkey , pubkey) == RHN_OK);
        int ret = r_jwk_generate_key_pair(privkey, pubkey, R_KEY_TYPE_RSA,
               rsa_bits[idx], NULL); // set kid automatically by library
        assert(ret == RHN_OK);
    } // end of loop
    size_t tmpfile_path_sz = strlen(tmpfile_path) + 1;
    _mock_jwks.filepath.pubkey = (char *) malloc(tmpfile_path_sz);
    memcpy(_mock_jwks.filepath.pubkey, &tmpfile_path[0], tmpfile_path_sz);
    int fd = mkstemp(_mock_jwks.filepath.pubkey);
    assert(fd >= 0);
    char *serial_data = r_jwks_export_to_json_str(_mock_jwks.store.pubkey, JSON_COMPACT);
    assert(serial_data != NULL);
    write(fd, serial_data, strlen(serial_data));
    lseek(fd, 0, SEEK_SET);
    free(serial_data);
    _mock_jwks.fd.pubkey = fd;
} // end of init_mock_auth_server


void deinit_mock_auth_server(void) {
    unlink(_mock_jwks.filepath.pubkey);
    close(_mock_jwks.fd.pubkey);
    while(r_jwks_size(_mock_jwks.store.privkey) > 0) {
        jwk_t *privkey = r_jwks_get_at(_mock_jwks.store.privkey, 0);
        jwk_t *pubkey  = r_jwks_get_at(_mock_jwks.store.pubkey , 0);
        r_jwks_remove_at(_mock_jwks.store.privkey, 0);
        r_jwks_remove_at(_mock_jwks.store.pubkey , 0);
        r_jwk_free(privkey);
        r_jwk_free(pubkey );
    } // end of loop
    free(_mock_jwks.filepath.pubkey);
    _mock_jwks.filepath.pubkey = NULL;
    r_jwks_free(_mock_jwks.store.pubkey);
    r_jwks_free(_mock_jwks.store.privkey);
} // end of deinit_mock_auth_server


static void test_verify__common_auth_token_fail(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 401;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
}

void api_test_common_auth_token_fail(test_setup_pub_t *setup_data)
{
    int result = 0;
    json_t *header_kv_serials = setup_data->headers;
    // subcase #1, missing header
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    // subcase #2, corrupted auth header
    result = json_array_append_new(header_kv_serials, json_string("Authorization: invalid_access_token"));
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    result = json_array_remove(header_kv_serials, json_array_size(header_kv_serials) - 1);
    assert_that(result, is_equal_to(0));
    result = json_array_append_new(header_kv_serials, json_string("Authorization:Bearer  invalid_access_token"));
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    // subcase #3, send token which wasn't signed by the mocked auth server
    result = json_array_remove(header_kv_serials, json_array_size(header_kv_serials) - 1);
    assert_that(result, is_equal_to(0));
    result = json_array_append_new(header_kv_serials, json_string("Authorization:Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"));
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
} // end of api_test_common_auth_token_fail

static void test_verify__common_perm_chk_fail(CURL *handle, test_setup_priv_t *privdata, void *usr_arg)
{
    CURLcode res;
    long expect_resp_code = 403;
    long actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
}

void api_test_common_permission_check_fail(test_setup_pub_t *setup_data)
{
    run_client_request(setup_data, test_verify__common_perm_chk_fail, NULL);
}

