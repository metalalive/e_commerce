#ifndef MEIDA__AUTH_H
#define MEIDA__AUTH_H
#ifdef __cplusplus
extern "C" {
#endif

#include "middleware.h"

int app_deinit_auth_jwt_claims(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

int app_authenticate_user(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

int app_basic_permission_check(struct hsearch_data *hmap);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__AUTH_H
