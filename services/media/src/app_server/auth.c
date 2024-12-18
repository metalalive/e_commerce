#include "third_party/rhonabwy.h"
#include "utils.h"
#include "auth.h"

#define AUTH_HDR_VAL_PREFIX  "Bearer "

static  char * extract_header_auth_token(h2o_req_t *req, size_t found_idx, size_t *o_sz)
{ // extract encoded JWT, e.g. `Authorization` header should contain : Bearer 401f7ac837da42b97f613d789819ff93537bee6a
    char *out = NULL;
    size_t hdr_val_prefix_sz = sizeof(AUTH_HDR_VAL_PREFIX) - 1;
    h2o_iovec_t *rawdata = &req->headers.entries[found_idx].value;
    if(!rawdata || !rawdata->base || rawdata->len == 0)
        goto done;
    int ret = strncmp(rawdata->base, AUTH_HDR_VAL_PREFIX, hdr_val_prefix_sz);
    if(ret != 0)
        goto done;
    out = (char *)rawdata->base + hdr_val_prefix_sz; // find first blank char
    *o_sz = rawdata->len - hdr_val_prefix_sz;
done:
    if(out == NULL)
        fprintf(stderr, "[auth] line:%d, found_idx:%lu, header name:%s, value:%s, value-sz:%lu \n",
               __LINE__, found_idx, req->headers.entries[found_idx].orig_name , rawdata->base,
                rawdata->len);
    return out;
} // end of extract_header_auth_token


static int verify_jwt_claim_audience(jwt_t *jwt) {
    json_t *audiences = r_jwt_get_claim_json_t_value(jwt, "aud");
    int result = RHN_ERROR;
    if(audiences && json_is_array(audiences)) {
        int idx = 0;
        json_t *item = NULL;
        json_array_foreach(audiences, idx, item) {
            const char* curr_aud = json_string_value(item);
            if(strncmp(curr_aud, APP_LABEL, (size_t)APP_LABEL_LEN) == 0) {
                result = RHN_OK;
                break;
            }
        }
        // Note r_jwt_get_claim_json_t_value() internally allocates extra memory space,
        //  it has to be freed as soon as it is not in use.
        json_decref(audiences);
    }
    return result;
} // end of verify_jwt_claim_audience


static json_t *perform_jwt_authentication(jwks_t *keyset, const char *in, size_t in_sz)
{ // In this application , auth server (user_management) always issue new access token with `kid` field
    jwk_t  *pubkey = NULL;
    jwt_t  *jwt = NULL;
    json_t *claims = NULL;
    int result = RHN_OK, typ = 0;
    result = r_jwt_init(&jwt);
    if(result != RHN_OK || !jwt)
        goto done;
    // only parse header & payload portion wihtout signature verification
    result = r_jwt_parsen(jwt, in, in_sz, R_FLAG_IGNORE_REMOTE);
    if(result != RHN_OK) {
        fprintf(stderr, "[auth] line:%d, parsing failure, result:%d, in_sz:%lu \n",
               __LINE__, result, in_sz );
        goto done;
    }
    // check whether essential claims exist prior to validating signature,
    result = verify_jwt_claim_audience(jwt);
    if(result != RHN_OK) {
        fprintf(stderr, "[auth] line:%d, failed to claim audience, result:%d, in_sz:%lu \n",
               __LINE__, result, in_sz );
        goto done;
    }
    typ =  r_jwt_get_type(jwt);
    if(typ != R_JWT_TYPE_SIGN) {
        goto done;
    } // TODO, support both encryption (JWE) and signature on jwt object (in auth server)
    const char *kid = r_jwt_get_header_str_value(jwt, "kid");
    if(!kid)
        goto done;
    pubkey =  r_jwks_get_by_kid(keyset, (const char *)kid);
    if(!pubkey) {
        fprintf(stderr, "[auth] line:%d, failed to get key ID object,"
                " result:%d, kid:%s \n", __LINE__, result, kid );
        goto done;
    }
    result = r_jwk_is_valid(pubkey);
    if(result != RHN_OK) {
        fprintf(stderr, "[auth] line:%d, invalid pub key, result:%d, kid:%s "
                "\n", __LINE__, result, kid );
        goto done;
    }
    result = r_jwt_verify_signature(jwt, pubkey, 0);
    if(result != RHN_OK) {
        fprintf(stderr, "[auth] line:%d, fail to verify sign, result:%d, kid:%s "
                "\n", __LINE__, result, kid );
        goto done;
    }
    result = r_jwt_validate_claims(jwt,
            R_JWT_CLAIM_EXP, R_JWT_CLAIM_NOW,
            R_JWT_CLAIM_IAT, R_JWT_CLAIM_NOW,
            R_JWT_CLAIM_NOP);
    if(result != RHN_OK)
        goto done;
    claims = r_jwt_get_full_claims_json_t(jwt); // do I need to copy it ?
done:
    if(jwt) {
        r_jwt_free(jwt);
        jwt = NULL;
    }
    if(pubkey) {
        r_jwk_free(pubkey);
        pubkey = NULL;
    }
    return claims;
} // end of perform_jwt_authentication


int app_deinit_auth_jwt_claims(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    if(jwt_claims) {
        json_decref(jwt_claims);
        ENTRY  e = {.key = "auth", .data = NULL};
        ENTRY *e_ret = NULL;
        hsearch_r(e, ENTER, &e_ret, node->data);
    }
    app_run_next_middleware(self, req, node);
    return 0;
} // end of app_deinit_auth_jwt_claims


json_t *app_auth_httphdr_decode_jwt (h2o_req_t *req)
{
    size_t  encoded_sz = 0;
    char   *encoded = NULL;
    json_t *decoded = NULL;
#define AUTH_HEADER_NAME  "authorization"
    size_t  name_len = sizeof(AUTH_HEADER_NAME) - 1; // exclude final byte which represent NULL-terminating character
    int found_idx = (int)h2o_find_header_by_str(&req->headers, AUTH_HEADER_NAME, name_len, -1);
#undef AUTH_HEADER_NAME
    if(found_idx == -1 || found_idx >= req->headers.size) // not found
        goto done;
    encoded = extract_header_auth_token(req, found_idx, &encoded_sz);
    if(encoded == NULL || encoded_sz == 0)
        goto done;
    struct app_jwks_t *jwks = (struct app_jwks_t *)req->conn->ctx->storage.entries[0].data;
    if(r_jwks_is_valid(jwks->handle) == RHN_OK) {
        decoded = perform_jwt_authentication(jwks->handle, encoded, encoded_sz);
        if(decoded && json_integer_value(json_object_get(decoded, "profile")) == 0) {
            h2o_error_printf("[auth] line:%d, jwt verified, missing usr profile ID\n", __LINE__);
            json_decref(decoded);
            decoded = NULL;
        } // this might be security vulnerability
    } else {
        // h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
        h2o_error_printf("[auth] line:%d, failed to import JWKS from %s \n", __LINE__, jwks->src_url);
    }
done:
    return decoded;
} // end of  app_auth_httphdr_decode_jwt


int app_authenticate_user(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    if(!self || !req || !node)
        goto error;
    json_t *decoded = app_auth_httphdr_decode_jwt (req);
    if(!decoded) // authentication failure
        goto error;
    ENTRY  e = {.key = "auth", .data = (void*)decoded };
    ENTRY *e_ret = NULL; // add new item to the given hash map
    if(hsearch_r(e, ENTER, &e_ret, node->data)) {
        // pass
    } else {
        h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
        h2o_error_printf("[auth] failed to save JWT claims (0x%lx) to given hash map \n",
                (unsigned long int)decoded );
        json_decref(decoded);
    }
    goto done;
error:
    h2o_send_error_401(req, "authentication failure", "", H2O_SEND_ERROR_KEEP_HEADERS);
    if(decoded) // TODO, de-initialize the jwt claims in asynchronous way
        json_decref(decoded);
done: // always switch to next middleware function ...
    app_run_next_middleware(self, req, node);
    return 0;
} // end of app_authenticate_user


int app_basic_permission_check(struct hsearch_data *hmap)
{
    int result = 1; // default to return error
    // type casting the array of permission codes : (const char *(*) [ARRAY_SIZE]) to (const char **)
    // the number of elements in expect_perms is unknown, the latest item has to be NULL
    const char **expect_perms = (const char **) app_fetch_from_hashmap(hmap, "expect_perm");
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(hmap, "auth");
    if(!expect_perms || !jwt_claims) {
        goto done;
    }
    // the claim is compilcated so it is unable to verify it simply using
    // `r_jwt_validate_claims(...)`
    json_t *perms = json_object_get(jwt_claims, "perms");
    if(!perms || !json_is_array(perms)) {
        goto done;
    }
    json_t *perm  = NULL;
    int idx = 0, jdx = 0;
    for(idx = 0; expect_perms[idx]; idx++) {
        int matched = 0;
        const char *expect_perm = expect_perms[idx];
        size_t  expect_perm_len = strlen(expect_perm);
        json_array_foreach(perms, jdx, perm) {
            int app_code = (int)json_integer_value(json_object_get(perm, "app_code"));
            if(app_code != APP_CODE) {
                continue;
            }
            const char *actual_perm = json_string_value(json_object_get(perm, "codename"));
            if(actual_perm && strncmp(actual_perm, expect_perm, expect_perm_len) == 0) {
                matched = 1;
                break;
            }
        } // end of iterating expected permissions
        if(!matched) {
            goto done;
        }
    } // end of iterating expected permissions
    result = 0; // done successfully
done:
    return result;
} // end of app_basic_permission_check


json_t * app_find_quota_arragement(json_t *jwt_claims, int app_code, int mat_code)
{
    json_t *qitem = NULL;
    if(!jwt_claims || app_code <= 0 || mat_code <= 0) {
        goto done;
    }
    json_t *quotas = json_object_get(jwt_claims, "quota");
    if(!quotas || !json_is_array(quotas)) {
        goto done;
    }
    int idx = 0;
    int app_code_read = 0;
    int mat_code_read = 0;
    json_array_foreach(quotas, idx, qitem) {
        app_code_read = (int)json_integer_value(json_object_get(qitem, "app_code"));
        if(app_code_read != app_code) {
            continue;
        }
        mat_code_read = (int)json_integer_value(json_object_get(qitem, "mat_code"));
        if(mat_code_read == mat_code) {
            break;
        }
    } // end of quota iteration
    if(idx == json_array_size(quotas)) {
        qitem = NULL;
    }
done:
    return qitem;
} // end of app_find_quota_arragement

// ---------------------------------------------------

// #define RECOVER_CORRUPTED_DATA(victom, origin)   if((victom)!=(origin)) { (victom) = (origin); }

int app_rotate_jwks_store(struct app_jwks_t *jwks) {
    int result = 1; // do nothing
    if(!jwks || !jwks->src_url) { goto done; }
    time_t last_update = jwks->last_update;
    time_t now_time = time(NULL);
    if(difftime(now_time, last_update) < (double)jwks->max_expiry_secs) {
        goto done; // key rotation NOT required
    }
    if(!atomic_flag_test_and_set_explicit(&jwks->is_rotating, memory_order_acquire))
    { // start of critical section
        const char *url = jwks->src_url;
        jwks_t *old_handle = jwks->handle;
        jwks_t *new_handle = NULL;
        r_jwks_init(&new_handle);
        ////jwks_t *new_handle_backup = new_handle;
        app_x5u_t  x5u = {.flags=0, .ca_path=jwks->ca_path, .ca_format=jwks->ca_format};
        int load_result = DEV_r_jwks_import_from_uri(new_handle, url, &x5u);
        //// RECOVER_CORRUPTED_DATA(new_handle, new_handle_backup);
        // FIXME : figure out when data corruption occurs ?
        if(load_result != RHN_OK)
        {
            h2o_error_printf("[parsing] failed to preload JWKS from given URI: %s \n", url);
            r_jwks_free(new_handle);
            goto end_of_cs;
        }
        if(r_jwks_is_valid(new_handle) != RHN_OK) {
            h2o_error_printf("[parsing] failed to decode to JWKS format, URI: %s \n", url);
            r_jwks_free(new_handle);
            goto end_of_cs;
        }
        jwks->handle = new_handle;
        time(&jwks->last_update);
        if(old_handle) {
            r_jwks_free(old_handle);
        }
        result = 0; // done successfully
end_of_cs:
        // other threads can contend next time when the JWKS expires
        atomic_flag_clear_explicit(&jwks->is_rotating, memory_order_release);
    } // end of critical section
    // otherwise, some other thread currently took the job and is still handling it, skip & let go
done:
    return result;
} // end of app_rotate_jwks_store

