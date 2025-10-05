#ifndef MEDIA__APP_SERVER_H
#define MEDIA__APP_SERVER_H
#ifdef __cplusplus
extern "C" {
#endif

#include "datatypes.h"

int start_application(const char *cfg_file_path, const char *exe_path);

int init_security(void);

int app_server_ready(void);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__APP_SERVER_H
