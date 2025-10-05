#include <cgreen/mocks.h>
#include "third_party/rhonabwy.h"

int r_global_init(void) { return (int)mock(); }

void r_global_close(void) {}

int DEV_r_jwks_import_from_uri(jwks_t *jwks, const char *uri, app_x5u_t *x5u) {
    return (int)mock(jwks, uri, x5u);
}

int r_jwks_init(jwks_t **jwks) { return (int)mock(jwks); }

void r_jwks_free(jwks_t *jwks) { mock(jwks); }

int r_jwks_is_valid(jwks_t *jwks) { return (int)mock(jwks); }

int r_jwt_init(jwt_t **jwt) { return (int)mock(jwt); }

void r_jwt_free(jwt_t *jwt) { mock(jwt); }

int r_jwt_parsen(jwt_t *jwt, const char *token, size_t token_len, int x5u_flags) {
    return (int)mock(jwt, token, token_len, x5u_flags);
}

json_t *r_jwt_get_claim_json_t_value(jwt_t *jwt, const char *key) { return (json_t *)mock(jwt, key); }

int r_jwt_get_type(jwt_t *jwt) { return (int)mock(jwt); }

const char *r_jwt_get_header_str_value(jwt_t *jwt, const char *key) { return (const char *)mock(jwt, key); }

json_t *r_jwt_get_full_claims_json_t(jwt_t *jwt) { return (json_t *)mock(jwt); }

jwk_t *r_jwks_get_by_kid(jwks_t *jwks, const char *kid) { return (jwk_t *)mock(jwks, kid); }

int r_jwk_is_valid(jwk_t *jwk) { return (int)mock(jwk); }

void r_jwk_free(jwk_t *jwk) {}

int r_jwt_verify_signature(jwt_t *jwt, jwk_t *pubkey, int x5u_flags) {
    return (int)mock(jwt, pubkey, x5u_flags);
}

int r_jwt_validate_claims(jwt_t *jwt, ...) { return (int)mock(jwt); }
