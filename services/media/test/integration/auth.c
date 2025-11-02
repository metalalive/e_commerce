#include <sys/types.h>
#include <unistd.h>
#include <string.h>

// #include "api/setup.h"
#include "../test/integration/test.h"

#define NUM_KEY_PAIRS 2

typedef struct {
    jwks_t *privkey;
} mock_jwks_t;

static mock_jwks_t _mock_jwks = {0};

static int gen_signed_access_token_helper(json_t *headers, json_t *claims, char **out) {
    int    result = RHN_OK;
    jwt_t *jwt = NULL;
    jwk_t *privkey = NULL;
    result = r_jwt_init(&jwt);
    if (result != RHN_OK) {
        goto done;
    }
    result = r_jwt_set_full_header_json_t(jwt, headers);
    if (result != RHN_OK) {
        goto done;
    }
    result = r_jwt_set_full_claims_json_t(jwt, claims);
    if (result != RHN_OK) {
        goto done;
    }
    privkey = r_jwks_get_at(_mock_jwks.privkey, 0);
    if (!privkey) {
        goto done;
    }
    result = r_jwt_set_sign_alg(jwt, R_JWA_ALG_RS256);
    if (result != RHN_OK) {
        goto done;
    }
    // the sign function internally allocated space for new generated token,
    // which nust be freed after use.
    *out = r_jwt_serialize_signed(jwt, privkey, 0);
done:
    if (privkey) {
        r_jwk_free(privkey);
        privkey = NULL;
    }
    if (jwt) {
        r_jwt_free(jwt);
        jwt = NULL;
    }
    return result;
} // end of gen_signed_access_token_helper

int gen_signed_access_token(unsigned int usr_id, json_t *perm_codes, json_t *quota, char **out) {
    int result = 0;
    if (!json_is_array(perm_codes) || !json_is_array(quota) || !out) {
        return result;
    }
    json_t *headers = json_object(), *claims = json_object();
    time_t  issued_time = time(NULL); // TODO, find more reliable way of reading current time in seconds
    time_t  expiry_time = issued_time + 600; // 10 minutes available by default
    json_object_set_new(headers, "typ", json_string("JWT"));
    json_object_set_new(claims, "profile", json_integer(usr_id));
    json_object_set_new(claims, "iat", json_integer(issued_time));
    json_object_set_new(claims, "exp", json_integer(expiry_time));
    {
        json_t *audience = json_array();
        json_array_append_new(audience, json_string("service1"));
        json_array_append_new(audience, json_string("service2"));
        json_array_append_new(audience, json_string(APP_LABEL));
        json_array_append_new(audience, json_string("service3"));
        json_object_set_new(claims, "aud", audience);
    }
    json_object_set(claims, "perms", perm_codes);
    json_object_set(claims, "quota", quota);
    result = gen_signed_access_token_helper(headers, claims, out);
    json_decref(headers);
    json_decref(claims);
    return result;
} // end of gen_signed_access_token

int add_auth_token_to_http_header(
    json_t *headers_kv_raw, unsigned int usr_id, const char **codename_list, json_t *quota
) {
    assert(headers_kv_raw);
    assert(json_is_array(headers_kv_raw));
    assert(quota);
    assert(json_is_array(quota));
    assert(usr_id > 0);
    json_t *perm_codes = json_array();
    char   *signed_access_token = NULL, *auth_header_raw = NULL;
    for (int idx = 0; codename_list && codename_list[idx]; idx++) {
        json_t *perm_code = json_object();
        json_object_set_new(perm_code, "app_code", json_integer(APP_CODE));
        json_object_set_new(perm_code, "codename", json_string(codename_list[idx]));
        json_array_append_new(perm_codes, perm_code);
    }
    int result = gen_signed_access_token(usr_id, perm_codes, quota, &signed_access_token);
    assert_that(result, is_equal_to(RHN_OK));
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

void init_mock_auth_jwks(const char *tmpfile_path) {
    json_error_t jerr = {0};
    json_t      *jsnobj = json_load_file(tmpfile_path, O_RDONLY, &jerr);
    assert(jsnobj);
    assert(r_jwks_init(&_mock_jwks.privkey) == RHN_OK);
    assert(r_jwks_import_from_json_t(_mock_jwks.privkey, jsnobj) == RHN_OK);
    json_decref(jsnobj);
}

void deinit_mock_auth_server(void) { r_jwks_free(_mock_jwks.privkey); } // end of deinit_mock_auth_server

static void test_verify__common_auth_token_fail(CURL *handle, test_setup_priv_t *privdata, void *usr_arg) {
    CURLcode res;
    long     expect_resp_code = 401;
    long     actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
}

void api_test_common_auth_token_fail(test_setup_pub_t *setup_data) {
    int     result = 0;
    json_t *header_kv_serials = setup_data->headers;
    // subcase #1, missing header
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    // subcase #2, corrupted auth header
    result = json_array_append_new(header_kv_serials, json_string("Authorization: invalid_access_token"));
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    result = json_array_remove(header_kv_serials, json_array_size(header_kv_serials) - 1);
    assert_that(result, is_equal_to(0));
    result =
        json_array_append_new(header_kv_serials, json_string("Authorization:Bearer  invalid_access_token"));
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
    // subcase #3, send token which wasn't signed by the mocked auth server
    result = json_array_remove(header_kv_serials, json_array_size(header_kv_serials) - 1);
    assert_that(result, is_equal_to(0));
    result = json_array_append_new(
        header_kv_serials,
        json_string("Authorization:Bearer "
                    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9."
                    "eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ."
                    "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c")
    );
    assert_that(result, is_equal_to(0));
    run_client_request(setup_data, test_verify__common_auth_token_fail, NULL);
} // end of api_test_common_auth_token_fail

static void test_verify__common_perm_chk_fail(CURL *handle, test_setup_priv_t *privdata, void *usr_arg) {
    CURLcode res;
    long     expect_resp_code = 403;
    long     actual_resp_code = 0;
    res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(expect_resp_code));
}

void api_test_common_permission_check_fail(test_setup_pub_t *setup_data) {
    run_client_request(setup_data, test_verify__common_perm_chk_fail, NULL);
}
