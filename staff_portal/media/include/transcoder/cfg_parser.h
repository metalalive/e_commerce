#ifndef MEDIA__TRANSCODER__CFG_PARSER_H
#define MEDIA__TRANSCODER__CFG_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include "app_cfg.h"
#include "transcoder/datatypes.h"

void app_transcoder_cfg_deinit(aav_cfg_transcode_t *cfg);

int parse_cfg_transcoder(json_t *objs, app_cfg_t *app_cfg);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__CFG_PARSER_H
