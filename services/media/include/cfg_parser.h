#ifndef MEIDA__CFG_PARSER_H
#define MEIDA__CFG_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include <jansson.h>
#include "datatypes.h"

typedef int (*app_cfg_parsing_fn)(json_t *, app_cfg_t *);

int parse_cfg_max_conns(json_t *obj, app_cfg_t *_app_cfg);

int parse_cfg_databases(json_t *objs, app_cfg_t *app_cfg);

int parse_cfg_listener_ssl(struct app_cfg_security_t *security, const json_t *obj);

int parse_cfg_params(const char *cfg_file_path, app_cfg_t *_app_cfg);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__CFG_PARSER_H
