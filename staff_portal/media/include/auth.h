#ifndef MEIDA__AUTH_H
#define MEIDA__AUTH_H
#ifdef __cplusplus
extern "C" {
#endif

#include "middleware.h"

int app_deinit_auth_jwt_claims(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

int app_authenticate_user(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

int app_basic_permission_check(struct hsearch_data *hmap);

int app_rotate_jwks_store(struct app_jwks_t *jwks);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__AUTH_H
