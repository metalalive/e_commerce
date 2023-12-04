#ifndef  MEDIA__MODELS_CFG_PARSER_H
#define  MEDIA__MODELS_CFG_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include <sysexits.h>
#include <jansson.h>
#include "app_cfg.h"

int parse_cfg_databases(json_t *objs, app_cfg_t *app_cfg);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_CFG_PARSER_H
