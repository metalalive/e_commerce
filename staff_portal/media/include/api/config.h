#ifndef MEDIA__API_CONFIG_H
#define MEDIA__API_CONFIG_H
#ifdef __cplusplus
extern "C" {
#endif

#define _API_MIDDLEWARE_CHAIN_initiate_multipart_upload  \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_initiate_multipart_upload, 0,  \
    API_FINAL_HANDLER_initiate_multipart_upload, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_upload_part  \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_upload_part, 0, \
    API_FINAL_HANDLER_upload_part, 0, \
    app_deinit_auth_jwt_claims, 1
    
#define _API_MIDDLEWARE_CHAIN_complete_multipart_upload \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_complete_multipart_upload, 0, \
    API_FINAL_HANDLER_complete_multipart_upload, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_abort_multipart_upload \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_abort_multipart_upload, 0, \
    API_FINAL_HANDLER_abort_multipart_upload, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_single_chunk_upload \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_single_chunk_upload, 0, \
    API_FINAL_HANDLER_single_chunk_upload, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_start_transcoding_file \
    10, app_authenticate_user, 1, \
    PERMISSION_CHECK_start_transcoding_file, 0, \
    api_abac_pep__start_transcode, 0, \
    API_FINAL_HANDLER_start_transcoding_file, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_discard_ongoing_job \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_discard_ongoing_job, 0, \
    API_FINAL_HANDLER_discard_ongoing_job, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_monitor_job_progress \
    6, app_authenticate_user, 1, \
    API_FINAL_HANDLER_monitor_job_progress, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_initiate_file_nonstream \
    6, api_abac_pep__init_filefetch, 1, \
    API_FINAL_HANDLER_initiate_file_nonstream, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_initiate_file_stream \
    6, api_abac_pep__init_filefetch, 1, \
    API_FINAL_HANDLER_initiate_file_stream, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_fetch_file_streaming_element \
    2,  API_FINAL_HANDLER_fetch_file_streaming_element, 0

#define _API_MIDDLEWARE_CHAIN_discard_committed_file \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_discard_committed_file, 0, \
    API_FINAL_HANDLER_discard_committed_file, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_edit_filelvl_acl \
    10, app_authenticate_user, 1, \
    PERMISSION_CHECK_edit_filelvl_acl, 0,  \
    api_abac_pep__edit_acl, 0, \
    API_FINAL_HANDLER_edit_filelvl_acl, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_edit_usrlvl_acl \
    10, app_authenticate_user, 1, \
    PERMISSION_CHECK_edit_usrlvl_acl, 0,  \
    api_abac_pep__edit_acl, 0, \
    API_FINAL_HANDLER_edit_usrlvl_acl, 0, \
    app_deinit_auth_jwt_claims, 1

#define _API_MIDDLEWARE_CHAIN_read_usrlvl_acl \
    8, app_authenticate_user, 1, \
    PERMISSION_CHECK_read_usrlvl_acl, 0, \
    API_FINAL_HANDLER_read_usrlvl_acl, 0, \
    app_deinit_auth_jwt_claims, 1

// the macro definitions below represent a list of `permission codename` required
// when accessing an API endpoint
#define _RESTAPI_PERM_CODES_initiate_multipart_upload "upload_files"
#define _RESTAPI_PERM_CODES_upload_part               "upload_files"
#define _RESTAPI_PERM_CODES_complete_multipart_upload "upload_files"
#define _RESTAPI_PERM_CODES_abort_multipart_upload    "upload_files"
#define _RESTAPI_PERM_CODES_single_chunk_upload       "upload_files"
#define _RESTAPI_PERM_CODES_start_transcoding_file    "upload_files"
#define _RESTAPI_PERM_CODES_discard_ongoing_job       "upload_files"
#define _RESTAPI_PERM_CODES_monitor_job_progress      "upload_files" 
#define _RESTAPI_PERM_CODES_initiate_file_nonstream      NULL
#define _RESTAPI_PERM_CODES_initiate_file_stream         NULL
#define _RESTAPI_PERM_CODES_fetch_file_streaming_element    NULL
#define _RESTAPI_PERM_CODES_discard_committed_file    "upload_files"
#define _RESTAPI_PERM_CODES_edit_filelvl_acl          "edit_file_access_control"
#define _RESTAPI_PERM_CODES_edit_usrlvl_acl           "edit_file_access_control"
#define _RESTAPI_PERM_CODES_read_usrlvl_acl           "edit_file_access_control"

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of  MEDIA__API_CONFIG_H
