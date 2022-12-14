#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include "auth.h"

#define NUM_REQ_HEADERS   3
#define NUM_RESP_HEADERS  2
#define NUM_HEADERS  (NUM_REQ_HEADERS + NUM_RESP_HEADERS)

typedef struct {
    char *data;
    const char *new_state;
} trcptr_t;

static void _update_trace_point(void *cb_args) {
    trcptr_t *tracepoint = (trcptr_t *)cb_args;
    tracepoint->data = (char *) tracepoint->new_state;
}

static int mock_final_req_handler(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    // dummy function only for test cases in this source file, to avoid app_run_next_middleware()
    // internally invokes app_cleanup_middlewares()
    return 0;
}

static  void mock_server_lowlevel_send(struct st_h2o_ostream_t *self, h2o_req_t *req, h2o_sendvec_t *bufs, size_t bufcnt, h2o_send_state_t state)
{
    // dummy function only for test cases in this source file, to avoid invoking read() / write()
    // on socket descriptor
} 

static h2o_iovec_t  mock_hdr_names[NUM_HEADERS] = {0};
static h2o_header_t   mock_headers[NUM_HEADERS];
static h2o_ostream_t  mock_sock_ostream = {.do_send=mock_server_lowlevel_send};
struct app_jwks_t     mock_jwks = {0};
static h2o_context_storage_item_t  ctx_storage_item0 = {0};
static h2o_context_t  mock_srv_ctx = {.emitted_error_status={0}};
static h2o_pathconf_t mock_pathconf = {0};
static h2o_handler_t  mock_hdlr = {0};
static h2o_conn_t     mock_conn = {0};
static h2o_req_t      mock_req  = {0};
static app_middleware_node_t  mock_mdchain_last = {0};
static app_middleware_node_t  mock_mdchain_head = {0};


Describe(MOCK_AUTH_PART);

BeforeEach(MOCK_AUTH_PART) {
    int idx = 0;
    for(idx = 0; idx < NUM_HEADERS; idx++) {
        mock_hdr_names[idx] = (h2o_iovec_t){.len=0, .base=NULL};
        mock_headers[idx].name = &mock_hdr_names[idx];
        mock_headers[idx].value = (h2o_iovec_t){.len=0, .base=NULL};
    }
    mock_jwks = (struct app_jwks_t) {0};
    ctx_storage_item0 = (h2o_context_storage_item_t) {.data = (struct app_jwks_t *)&mock_jwks};
    mock_srv_ctx.storage = (h2o_context_storage_t) {.size=1, .capacity=1, .entries=&ctx_storage_item0};
    mock_conn.ctx = &mock_srv_ctx;
    mock_req.conn = &mock_conn;
    mock_req.pathconf  = &mock_pathconf;
    mock_req._ostr_top = &mock_sock_ostream;
    mock_req.res.status  = 0;
    mock_req.res.headers = (h2o_headers_t){.size=0, .capacity=NUM_RESP_HEADERS, .entries=&mock_headers[NUM_REQ_HEADERS]};
    mock_req.headers     = (h2o_headers_t){.size=NUM_REQ_HEADERS, .capacity=NUM_REQ_HEADERS, .entries=&mock_headers[0]};
    h2o_mem_init_pool(&mock_req.pool);
    mock_mdchain_last = (app_middleware_node_t){.data=NULL, .next=NULL,  .fn=mock_final_req_handler};
    mock_mdchain_head = (app_middleware_node_t){.data=NULL, .next=&mock_mdchain_last, .fn=NULL};
} // end of BeforeEach(MOCK_AUTH_PART)

AfterEach(MOCK_AUTH_PART) {
    mock_srv_ctx.storage = (h2o_context_storage_t) {0};
    h2o_mem_clear_pool(&mock_req.pool);
    mock_req.pool = (h2o_mem_pool_t){0};
    memset(&mock_srv_ctx.emitted_error_status[0] , 0, sizeof(uint64_t) * H2O_STATUS_ERROR_MAX);
} // end of AfterEach(MOCK_AUTH_PART)


Ensure(MOCK_AUTH_PART, auth_header_missing_tests) {
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=11, .base="content-md5"};
    *mock_req.headers.entries[1].name = (h2o_iovec_t){.len=4,  .base="food"};
    *mock_req.headers.entries[2].name = (h2o_iovec_t){.len=7, .base="culture"};
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_srv_ctx.emitted_error_status[H2O_STATUS_ERROR_403] , is_equal_to(1));
    assert_that(mock_req.res.status, is_equal_to(401));
}

Ensure(MOCK_AUTH_PART, auth_header_incomplete_tests) {
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=11, .base="content-md5"};
    *mock_req.headers.entries[1].name = (h2o_iovec_t){.len=13, .base="authorization"};
    *mock_req.headers.entries[2].name = (h2o_iovec_t){.len=7, .base="culture"};
    mock_req.headers.entries[1].value = (h2o_iovec_t){.len=0, .base=NULL};
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
    mock_req.res.status  = 0;
    mock_req.headers.entries[1].value = (h2o_iovec_t){.len=6, .base="abc123"};
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
    mock_req.res.status  = 0;
    mock_req.headers.entries[1].value = (h2o_iovec_t){.len=6, .base="Bearer"};
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
}

Ensure(MOCK_AUTH_PART, auth_jwt_init_failure_tests) {
    // has to be mutable cuz strtok_r() always tries to modify the given string
    char mock_encoded_token[] = "Bearer abc123wrongJwt\x00";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=11, .base="content-md5"};
    *mock_req.headers.entries[1].name = (h2o_iovec_t){.len=13, .base="authorization"};
    *mock_req.headers.entries[2].name = (h2o_iovec_t){.len=7, .base="culture"};
    mock_req.headers.entries[1].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // subcase 1 : assume jwt initialization failure
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init, will_return(RHN_ERROR_INVALID));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
}

Ensure(MOCK_AUTH_PART, auth_jwt_encode_error_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init, will_return(RHN_OK), will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_ERROR_INVALID));
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
}

Ensure(MOCK_AUTH_PART, auth_jwt_incorrect_audience_1_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains valid audience, but should be in json array
    json_t *mock_aud_claim = json_string(APP_LABEL);
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
    json_decref(mock_aud_claim);
}

Ensure(MOCK_AUTH_PART, auth_jwt_incorrect_audience_2_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_2"));
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_3"));
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
}

Ensure(MOCK_AUTH_PART, auth_jwt_missing_keyid_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string(APP_LABEL));
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_get_type, will_return(R_JWT_TYPE_SIGN));
    expect(r_jwt_get_header_str_value,
            will_return(NULL),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("kid")) );
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
} // end of auth_jwt_missing_keyid_tests


Ensure(MOCK_AUTH_PART, auth_jwt_incorrect_pubkey_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string(APP_LABEL));
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    const char *mock_keyid = "12345678";
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_get_type, will_return(R_JWT_TYPE_SIGN));
    expect(r_jwt_get_header_str_value,
            will_return(mock_keyid),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("kid")) );
    expect(r_jwks_get_by_kid,   will_return(NULL),   when(kid, is_equal_to_string(mock_keyid)));
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
} // end of auth_jwt_incorrect_pubkey_tests


Ensure(MOCK_AUTH_PART, auth_jwt_verify_signature_failure_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string(APP_LABEL));
    jwk_t  mock_jwk = {0};
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    const char *mock_keyid = "12345678";
    trcptr_t tracepoint = {.data=NULL, .new_state="reach_verify_signature"};
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK),  when(jwt, is_equal_to(mock_jwt_ptr)));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_get_type, will_return(R_JWT_TYPE_SIGN));
    expect(r_jwt_get_header_str_value,
            will_return(mock_keyid),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("kid")) );
    expect(r_jwks_get_by_kid,  will_return(&mock_jwk),   when(kid, is_equal_to_string(mock_keyid)));
    expect(r_jwk_is_valid, will_return(RHN_OK), when(jwk, is_equal_to(&mock_jwk)));
    expect(r_jwt_verify_signature,
            will_return(RHN_ERROR),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(pubkey, is_equal_to(&mock_jwk)),
            with_side_effect(&_update_trace_point, (void *)&tracepoint)
        );
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
    assert_that(tracepoint.data, is_equal_to(tracepoint.new_state));
} // end of auth_jwt_verify_signature_failure_tests


Ensure(MOCK_AUTH_PART, auth_jwt_expiry_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string(APP_LABEL));
    jwk_t  mock_jwk = {0};
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    const char *mock_keyid = "12345678";
    expect(r_jwks_is_valid, will_return(RHN_OK));
    expect(r_jwt_init,
            will_return(RHN_OK),
            will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
    expect(r_jwt_parse, will_return(RHN_OK),  when(jwt, is_equal_to(mock_jwt_ptr)));
    expect(r_jwt_get_claim_json_t_value,
            will_return(mock_aud_claim),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("aud")) );
    expect(r_jwt_get_type, will_return(R_JWT_TYPE_SIGN));
    expect(r_jwt_get_header_str_value,
            will_return(mock_keyid),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(key, is_equal_to_string("kid")) );
    expect(r_jwks_get_by_kid,  will_return(&mock_jwk),   when(kid, is_equal_to_string(mock_keyid)));
    expect(r_jwk_is_valid, will_return(RHN_OK), when(jwk, is_equal_to(&mock_jwk)));
    expect(r_jwt_verify_signature,
            will_return(RHN_OK),
            when(jwt, is_equal_to(mock_jwt_ptr)),
            when(pubkey, is_equal_to(&mock_jwk)));
    expect(r_jwt_validate_claims,
            will_return(RHN_ERROR),
            when(jwt, is_equal_to(mock_jwt_ptr)));
    expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    app_authenticate_user(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(401));
} // end of auth_jwt_expiry_tests


Ensure(MOCK_AUTH_PART, auth_jwt_succeed_tests) {
    char mock_encoded_token[] = "Bearer assume_it_is_encoded_access_token";
    *mock_req.headers.entries[0].name = (h2o_iovec_t){.len=13, .base="authorization"};
    mock_req.headers.entries[0].value = (h2o_iovec_t){.len=sizeof(mock_encoded_token), .base=&mock_encoded_token[0]};
    // this contains json array without expected audience for this application
    uint32_t mock_auth_usr_id = 182;
    json_t *mock_full_claims = json_object();
    json_t *mock_aud_claim = json_array();
    json_array_append_new(mock_aud_claim, json_string("unrelated_service_1"));
    json_array_append_new(mock_aud_claim, json_string(APP_LABEL));
    // Note r_jwt_get_claim_json_t_value() internally allocates extra memory space
    json_object_set (mock_full_claims, "aud", mock_aud_claim);
    json_object_set_new (mock_full_claims, "profile", json_integer(mock_auth_usr_id));
    jwk_t  mock_jwk = {0};
    jwt_t  mock_jwt = {0};
    jwt_t *mock_jwt_ptr = &mock_jwt;
    const char *mock_keyid = "12345678";
    trcptr_t tracepoint = {.data=NULL, .new_state="reach_r_jwt_get_full_claims_json_t"};
    {
        expect(r_jwks_is_valid, will_return(RHN_OK));
        expect(r_jwt_init,
                will_return(RHN_OK),
                will_set_contents_of_parameter(jwt, &mock_jwt_ptr, sizeof(jwt_t **)));
        expect(r_jwt_parse, will_return(RHN_OK),  when(jwt, is_equal_to(mock_jwt_ptr)));
        expect(r_jwt_get_claim_json_t_value,
                will_return(mock_aud_claim),
                when(jwt, is_equal_to(mock_jwt_ptr)),
                when(key, is_equal_to_string("aud")) );
        expect(r_jwt_get_type, will_return(R_JWT_TYPE_SIGN));
        expect(r_jwt_get_header_str_value,
                will_return(mock_keyid),
                when(jwt, is_equal_to(mock_jwt_ptr)),
                when(key, is_equal_to_string("kid")) );
        expect(r_jwks_get_by_kid,  will_return(&mock_jwk),   when(kid, is_equal_to_string(mock_keyid)));
        expect(r_jwk_is_valid, will_return(RHN_OK), when(jwk, is_equal_to(&mock_jwk)));
        expect(r_jwt_verify_signature,
                will_return(RHN_OK),
                when(jwt, is_equal_to(mock_jwt_ptr)),
                when(pubkey, is_equal_to(&mock_jwk)));
        expect(r_jwt_validate_claims,
                will_return(RHN_OK),
                when(jwt, is_equal_to(mock_jwt_ptr)));
        expect(r_jwt_get_full_claims_json_t,
                will_return(mock_full_claims),
                when(jwt, is_equal_to(mock_jwt_ptr)),
                with_side_effect(&_update_trace_point, (void *)&tracepoint));
        expect(r_jwt_free, when(jwt, is_equal_to(mock_jwt_ptr)));
    }
    struct hsearch_data  mock_hashmap = {0};
    hcreate_r(2, &mock_hashmap);
    mock_mdchain_head.data = &mock_hashmap;
    mock_mdchain_last.data = &mock_hashmap;
    mock_mdchain_head.fn = app_authenticate_user;
    mock_mdchain_head.fn(&mock_hdlr, &mock_req, &mock_mdchain_head);
    assert_that(mock_req.res.status, is_equal_to(0)); // not modified
    assert_that(tracepoint.data, is_not_null);
    assert_that(tracepoint.data, is_equal_to(tracepoint.new_state));
    {
        ENTRY  e = {.key = "auth", .data = NULL};
        ENTRY *e_ret = NULL;
        hsearch_r(e, FIND, &e_ret, &mock_hashmap);
        assert_that(e_ret, is_not_null);
        assert_that(e_ret->data, is_not_null);
        assert_that(e_ret->data, is_equal_to(mock_full_claims));
    }
    hdestroy_r(&mock_hashmap);
    json_decref(mock_full_claims);
} // end of auth_jwt_succeed_tests


Ensure(perm_chk_failure_tests) {
    int result = 0;
    // subcase 1: missing essential claims
    const char *expect_perms[] = {"can_do_this", "can_do_that", NULL};
    json_t *actual_perms = NULL;
    json_t *auth_claims = json_object();
    struct hsearch_data  mock_hashmap = {0};
    hcreate_r(2, &mock_hashmap);
    {
        ENTRY  e = {.key = "auth", .data = (void *)auth_claims};
        ENTRY *e_ret = NULL;
        hsearch_r(e, ENTER, &e_ret, &mock_hashmap);
        e.key  = "expect_perm";
        e.data = (void *)&expect_perms[0];
        hsearch_r(e, ENTER, &e_ret, &mock_hashmap);
    }
    result = app_basic_permission_check(&mock_hashmap);
    assert_that(result, is_not_equal_to(0));
    // subcase 2: inappropriate code format
    actual_perms = json_integer((json_int_t)1234);
    json_object_set(auth_claims, "perms", actual_perms);
    result = app_basic_permission_check(&mock_hashmap);
    assert_that(result, is_not_equal_to(0));
    json_object_del(auth_claims, "perms");
    json_decref(actual_perms);
    // -----
    hdestroy_r(&mock_hashmap);
    json_decref(auth_claims);
} // end of perm_chk_failure_tests


Ensure(perm_chk_not_satisfy_all_tests) {
    int result = 0;
    struct hsearch_data  mock_hashmap = {0};
    const char *expect_perms[] = {"can_do_this", "can_do_that", NULL};
    json_t *actual_perms = json_array();
    json_t *auth_claims = json_object();
    {
        json_t *perm = json_object();
        json_object_set_new(perm, "app_code", json_integer(APP_CODE));
        json_object_set_new(perm, "codename", json_string("can_do_that"));
        json_array_append_new(actual_perms, perm);
        perm = json_object();
        json_object_set_new(perm, "app_code", json_integer(APP_CODE));
        json_object_set_new(perm, "codename", json_string("can_do_1234"));
        json_array_append_new(actual_perms, perm);
        json_object_set_new(auth_claims, "perms", actual_perms);
    }
    hcreate_r(2, &mock_hashmap);
    {
        ENTRY  e = {.key = "auth", .data = (void *)auth_claims};
        ENTRY *e_ret = NULL;
        hsearch_r(e, ENTER, &e_ret, &mock_hashmap);
        e.key  = "expect_perm";
        e.data = (void *)&expect_perms[0];
        hsearch_r(e, ENTER, &e_ret, &mock_hashmap);
    }
    result = app_basic_permission_check(&mock_hashmap);
    assert_that(result, is_not_equal_to(0));
    {
        json_t *perm = json_object();
        json_object_set_new(perm, "app_code", json_integer(APP_CODE));
        json_object_set_new(perm, "codename", json_string("can_do_this"));
        json_array_append_new(actual_perms, perm);
    }
    result = app_basic_permission_check(&mock_hashmap);
    assert_that(result, is_equal_to(0));
    // -----
    hdestroy_r(&mock_hashmap);
    json_decref(auth_claims);
} // end of perm_chk_not_satisfy_all_tests


Ensure(quota_lookup_test) {
    json_t *mock_jwt_claims = json_object();
    json_t *quotas = json_array();
    json_t *result = NULL;
    int expect_app_code = 12;
    int expect_mat_code = 23;
    int expect_max_num = 975;
    {
        json_t *quota = json_object();
        json_object_set_new(quota, "app_code", json_integer(expect_app_code));
        json_object_set_new(quota, "mat_code", json_integer(expect_mat_code));
        json_object_set_new(quota, "maxnum", json_integer(expect_max_num));
        json_array_append_new(quotas, quota);
        json_object_set_new(mock_jwt_claims, "quota", quotas);
    }
    result = app_find_quota_arragement(mock_jwt_claims, 19, 78);
    assert_that(result, is_null);
    result = app_find_quota_arragement(mock_jwt_claims, expect_app_code, expect_mat_code);
    assert_that(result, is_not_null);
    if(result) {
        int actual_max_num = (int) json_integer_value(json_object_get(result, "maxnum"));
        assert_that(actual_max_num, is_equal_to(expect_max_num));
    }
    json_decref(mock_jwt_claims);
} // end of quota_lookup_test


Ensure(rotate_jwks_not_expired_tests) {
    char src_url[] = "fake_url_to_auth_server_jwks";
    unsigned int  max_expiry_secs = 600;
    mock_jwks = (struct app_jwks_t) {
        .src_url = &src_url[0],  .max_expiry_secs = max_expiry_secs,
        .last_update = time(NULL) - (max_expiry_secs - 5) // time in seconds
    };
    int result = app_rotate_jwks_store(&mock_jwks);
    assert_that(result, is_equal_to(1));
} // end of rotate_jwks_not_expired_tests


Ensure(rotate_jwks_remote_server_error_tests) {
    char src_url[] = "fake_url_to_auth_server_jwks";
    unsigned int  max_expiry_secs = 600;
    jwks_t  old_handle = {0};
    jwks_t  new_handle = {0};
    jwks_t *new_handle_ptr = &new_handle;
    mock_jwks = (struct app_jwks_t) {
        .src_url = &src_url[0], .handle = &old_handle,  .max_expiry_secs = max_expiry_secs,
        .last_update = time(NULL) - (max_expiry_secs + 5) // time in seconds
    };
    time_t expect_last_update = mock_jwks.last_update;
    int result = 0;
    for(int idx = 0; idx < 3; idx ++)
    { // try re-entering the function and it should still get the same error
        expect(r_jwks_init,
                will_return(RHN_OK),
                will_set_contents_of_parameter(jwks, &new_handle_ptr, sizeof(jwks_t **)));
        expect(r_jwks_free, when(jwks, is_equal_to(new_handle_ptr)));
        expect(DEV_r_jwks_import_from_uri,
                will_return(RHN_ERROR),
                when(jwks, is_equal_to(new_handle_ptr)),
                when(uri,  is_equal_to_string(src_url)) );
        result = app_rotate_jwks_store(&mock_jwks);
        assert_that(result, is_equal_to(1));
        assert_that(mock_jwks.handle, is_equal_to(&old_handle));
        assert_that(mock_jwks.last_update, is_equal_to(expect_last_update));
    }
} // end of rotate_jwks_remote_server_error_tests


Ensure(rotate_jwks_succeed_tests) {
    char src_url[] = "fake_url_to_auth_server_jwks";
    unsigned int  max_expiry_secs = 600;
    jwks_t  old_handle = {0};
    jwks_t  new_handle = {0};
    jwks_t *new_handle_ptr = &new_handle;
    mock_jwks = (struct app_jwks_t) {
        .src_url = &src_url[0], .handle = &old_handle,  .max_expiry_secs = max_expiry_secs,
        .last_update = time(NULL) - (max_expiry_secs + 15) // time in seconds
    };
    {
        expect(r_jwks_init,
                will_return(RHN_OK),
                will_set_contents_of_parameter(jwks, &new_handle_ptr, sizeof(jwks_t **)));
        expect(DEV_r_jwks_import_from_uri,
                will_return(RHN_OK),
                when(jwks, is_equal_to(new_handle_ptr)),
                when(uri,  is_equal_to_string(src_url)) );
        expect(r_jwks_is_valid,
                will_return(RHN_OK),
                when(jwks, is_equal_to(new_handle_ptr)) );
        expect(r_jwks_free, when(jwks, is_equal_to(&old_handle)));
        time_t previous_update = mock_jwks.last_update;
        int result = app_rotate_jwks_store(&mock_jwks);
        assert_that(result, is_equal_to(0));
        assert_that(mock_jwks.handle, is_equal_to(new_handle_ptr));
        assert_that(previous_update, is_less_than(mock_jwks.last_update));
    }
    { // try re-entering the function and it should NOT rotate again
      // because the refreshed JWKS hasn't expired yet.
        time_t previous_update = mock_jwks.last_update;
        int result = app_rotate_jwks_store(&mock_jwks);
        assert_that(result, is_equal_to(1));
        assert_that(previous_update, is_equal_to(mock_jwks.last_update));
    }
} // end of rotate_jwks_succeed_tests


TestSuite *app_auth_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test_with_context(suite, MOCK_AUTH_PART, auth_header_missing_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_header_incomplete_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_init_failure_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_encode_error_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_incorrect_audience_1_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_incorrect_audience_2_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_missing_keyid_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_incorrect_pubkey_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_verify_signature_failure_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_expiry_tests);
    add_test_with_context(suite, MOCK_AUTH_PART, auth_jwt_succeed_tests);
    add_test(suite, perm_chk_failure_tests);
    add_test(suite, perm_chk_not_satisfy_all_tests);
    add_test(suite, quota_lookup_test);
    add_test(suite, rotate_jwks_not_expired_tests);
    add_test(suite, rotate_jwks_remote_server_error_tests);
    add_test(suite, rotate_jwks_succeed_tests);
    return suite;
}
