#ifndef MEIDA__ROUTES_H
#define MEIDA__ROUTES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>
#include <jansson.h>

int setup_routes(h2o_hostconf_t *host, json_t *routes_cfg, const char *exe_path);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__ROUTES_H
