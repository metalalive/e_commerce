#ifndef MEIDA__VIEWS_H
#define MEIDA__VIEWS_H
#ifdef __cplusplus
extern "C" {
#endif

#include <search.h>
#include "auth.h"
#include "middleware.h"
#include "models/datatypes.h"
#include "acl.h"

// see media/migration/changelog_usermgt.log for detail
#define QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER  1

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
// when an API endpoint is consumed
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

#define RESTAPI_PERMISSIONS_MAP(func_name)  _RESTAPI_PERM_CODES_##func_name
#define API_MIDDLEWARE_CHAIN(func_name)     _API_MIDDLEWARE_CHAIN_##func_name

// for any request whose method does NOT match the parameter `http_method`,  the handler function
// actually returns non-zero integer which means it passed the request to next handler function
// (if exists)
#define RESTAPI_ENDPOINT_HANDLER(func_name, http_method, hdlr_var, req_var) \
    static int PERMISSION_CHECK_##func_name(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) \
    { \
        const char *perm_codes[] = {RESTAPI_PERMISSIONS_MAP(func_name), NULL}; \
        ENTRY  e = {.key = "expect_perm", .data = (void*)&perm_codes[0] }; \
        ENTRY *e_ret = NULL; \
        hsearch_r(e, ENTER, &e_ret, node->data); \
        if(app_basic_permission_check(node->data)) \
            h2o_send_error_403(req, "Not permitted to perform the action", "", 0); \
        app_run_next_middleware(self, req, node); \
        return 0; \
    } \
    \
    static int API_FINAL_HANDLER_##func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var), app_middleware_node_t *node); \
    \
    static __attribute__((optimize("O0"))) int func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var)) \
    { \
        h2o_iovec_t *expect = &(req_var)->input.method; \
        int ret = strncmp((const char *)(#http_method), expect->base, expect->len); \
        if(ret == 0) {  \
            app_middleware_node_t* (*fp1) (size_t, ...); \
            fp1 = app_gen_middleware_chain; \
            app_middleware_node_t *head = (*fp1)(API_MIDDLEWARE_CHAIN(func_name)); \
            ret = head->fn((hdlr_var), (req_var), head); \
            return ret; \
        } else { \
            return -1; \
        } \
    } \
    static int API_FINAL_HANDLER_##func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var), app_middleware_node_t *node)


#define   MAX_BYTES_JOB_ID    90  // TODO, parameterize
#define   ASA_USRARG_INDEX__APIUSRDATA   0
#define   ASA_USRARG_INDEX__API_RPC_REPLY_DATA   1

DBA_RES_CODE  app_validate_uncommitted_upld_req (
    RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node,
    const char *db_table, void (*err_cb)(db_query_t *, db_query_result_t *),
    app_middleware_fn success_cb,
    app_middleware_fn failure_cb
);

int  app_verify_printable_string(const char *str, size_t limit_sz);

const char *app_resource_id__url_decode(json_t *spec, json_t *err_info);

int  api_http_resp_status__verify_resource_id (aacl_result_t *, json_t *err_info);

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail);

asa_op_base_cfg_t * api_job_progress_update__init_asaobj (void *loop, uint32_t usr_id, size_t num_usr_args);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__VIEWS_H
