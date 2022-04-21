#ifndef MEIDA__ROUTES_H
#define MEIDA__ROUTES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>
#include <jansson.h>

typedef uint8_t (*app_elf_fn_traverse_cb)(char *fn_name, void *entry_point, void *cb_args);

int app_elf_traverse_functions(const char *exe_path, app_elf_fn_traverse_cb cb, void *cb_args);

int app_setup_apiview_routes(h2o_hostconf_t *host, json_t *routes_cfg, const char *exe_path);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__ROUTES_H
