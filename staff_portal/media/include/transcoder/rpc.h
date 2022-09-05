#ifndef  MEDIA__TRANSCODER_RPC_H
#define  MEDIA__TRANSCODER_RPC_H
#ifdef __cplusplus
extern "C" {
#endif
#include "transcoder/file_processor.h"

#define   APP_ENCODED_RD_BUF_SZ       2048
#define   APP_ENCODED_WR_BUF_SZ       1280

#define   ASA_USRARG_INDEX__AFTP         ATFP_INDEX__IN_ASA_USRARG
#define   ASA_USRARG_INDEX__ASAOBJ_MAP   ASAMAP_INDEX__IN_ASA_USRARG
// for all file processors of each API request
#define   ASA_USRARG_INDEX__RPC_RECEIPT  (ASA_USRARG_INDEX__ASAOBJ_MAP + 1)
#define   ASA_USRARG_INDEX__API_REQUEST  (ASA_USRARG_INDEX__ASAOBJ_MAP + 2)
#define   ASA_USRARG_INDEX__VERSION_LABEL   (ASA_USRARG_INDEX__ASAOBJ_MAP + 3)
#define   ASA_USRARG_INDEX__ERROR_INFO      (ASA_USRARG_INDEX__ASAOBJ_MAP + 4)
#define   ASA_USRARG_INDEX__STORAGE_CONFIG  (ASA_USRARG_INDEX__ASAOBJ_MAP + 5)

#define   NUM_USRARGS_ASA_LOCALTMP  (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)
#define   NUM_USRARGS_ASA_SRC       (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)
#define   NUM_USRARGS_ASA_DST       (ASA_USRARG_INDEX__STORAGE_CONFIG + 1)

void  api_rpc_transcoding__storagemap_deinit(atfp_asa_map_t *_map);
void  api_rpc_transcode__finalize (atfp_asa_map_t *map);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER_RPC_H
