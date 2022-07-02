#ifndef MEIDA__VIEWS_H
#define MEIDA__VIEWS_H
#ifdef __cplusplus
extern "C" {
#endif

#include <search.h>
#include "auth.h"
#include "middleware.h"
#include "models/datatypes.h"

// see media/migration/changelog_usermgt.log for detail
#define QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER  1

#define _API_MIDDLEWARE_CHAIN_initiate_multipart_upload  \
    4, app_authenticate_user, \
    PERMISSION_CHECK_initiate_multipart_upload,  \
    API_FINAL_HANDLER_initiate_multipart_upload, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_upload_part  \
    4, app_authenticate_user, \
    PERMISSION_CHECK_upload_part,  \
    API_FINAL_HANDLER_upload_part, \
    app_deinit_auth_jwt_claims
    
#define _API_MIDDLEWARE_CHAIN_complete_multipart_upload \
    4, app_authenticate_user, \
    PERMISSION_CHECK_complete_multipart_upload, \
    API_FINAL_HANDLER_complete_multipart_upload, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_abort_multipart_upload \
    4, app_authenticate_user, \
    PERMISSION_CHECK_abort_multipart_upload, \
    API_FINAL_HANDLER_abort_multipart_upload, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_single_chunk_upload \
    4, app_authenticate_user, \
    PERMISSION_CHECK_single_chunk_upload, \
    API_FINAL_HANDLER_single_chunk_upload, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_start_transcoding_file \
    4, app_authenticate_user, \
    PERMISSION_CHECK_start_transcoding_file, \
    API_FINAL_HANDLER_start_transcoding_file, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_discard_ongoing_job \
    4, app_authenticate_user, \
    PERMISSION_CHECK_discard_ongoing_job, \
    API_FINAL_HANDLER_discard_ongoing_job, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_monitor_job_progress \
    4, app_authenticate_user, \
    PERMISSION_CHECK_monitor_job_progress,  \
    API_FINAL_HANDLER_monitor_job_progress, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_fetch_entire_file \
    3, app_authenticate_user, \
    API_FINAL_HANDLER_fetch_entire_file, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_get_next_media_segment \
    3, app_authenticate_user, \
    API_FINAL_HANDLER_get_next_media_segment, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_discard_file \
    4, app_authenticate_user, \
    PERMISSION_CHECK_discard_file, \
    API_FINAL_HANDLER_discard_file,  \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_edit_file_acl \
    4, app_authenticate_user, \
    PERMISSION_CHECK_edit_file_acl, \
    API_FINAL_HANDLER_edit_file_acl, \
    app_deinit_auth_jwt_claims

#define _API_MIDDLEWARE_CHAIN_read_file_acl \
    4, app_authenticate_user, \
    PERMISSION_CHECK_read_file_acl, \
    API_FINAL_HANDLER_read_file_acl, \
    app_deinit_auth_jwt_claims


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
#define _RESTAPI_PERM_CODES_fetch_entire_file         NULL
#define _RESTAPI_PERM_CODES_get_next_media_segment    NULL
#define _RESTAPI_PERM_CODES_discard_file              "upload_files"
#define _RESTAPI_PERM_CODES_edit_file_acl             "edit_file_access_control"
#define _RESTAPI_PERM_CODES_read_file_acl             "edit_file_access_control"

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
        if(!app_basic_permission_check(node->data)) { \
            app_run_next_middleware(self, req, node); \
        } else { \
            h2o_send_error_403(req, "Not permitted to perform the action", "", 0); \
        } \
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


#define DATETIME_STR_SIZE    20
#define USR_ID_STR_SIZE      10
#define UPLOAD_INT2HEX_SIZE(x) (sizeof(x) << 1)
// TODO, synchronize following parameters with DB migration config file
#define APP_RESOURCE_ID_SIZE  8
#define APP_TRANSCODED_VERSION_SIZE  2

DBA_RES_CODE  app_validate_uncommitted_upld_req (
    RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node,
    const char *db_table, void (*err_cb)(db_query_t *, db_query_result_t *),
    app_middleware_fn success_cb,
    app_middleware_fn failure_cb
);

DBA_RES_CODE  app_verify_existence_resource_id (
    RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node,  void (*err_cb)(db_query_t *, db_query_result_t *),
    app_middleware_fn success_cb,  app_middleware_fn failure_cb
);

int  app_verify_printable_string(const char *str, size_t limit_sz);

void app_db_async_dummy_cb(db_query_t *target, db_query_result_t *detail);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__VIEWS_H
