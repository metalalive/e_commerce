#ifndef MEDIA__RPC_CONSUMER_H
#define MEDIA__RPC_CONSUMER_H
#ifdef __cplusplus
extern "C" {
#endif

#include <stdio.h>
#include <stdatomic.h>
#include <h2o.h>

#include "rpc/datatypes.h"

void run_app_worker(void *usr_data);

int start_application(const char *cfg_file_path, const char *exe_path);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__RPC_CONSUMER_H
