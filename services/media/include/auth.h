#ifndef MEIDA__AUTH_H
#define MEIDA__AUTH_H
#ifdef __cplusplus
extern "C" {
#endif

#include "middleware.h"

int app_deinit_auth_jwt_claims(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

json_t *app_auth_httphdr_decode_jwt(h2o_req_t *);

int app_authenticate_user(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

int app_basic_permission_check(struct hsearch_data *hmap);

json_t *app_find_quota_arragement(json_t *jwt_claims, int app_code, int mat_code);

int app_rotate_jwks_store(struct app_jwks_t *jwks);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__AUTH_H
