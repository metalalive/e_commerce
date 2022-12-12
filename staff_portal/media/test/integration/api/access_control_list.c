#include <jansson.h>
#include "../test/integration/test.h"

#define  ITEST_REQ_ITEM_GEN(_usr_id, _tr, _ed) "{\"usr_id\":" #_usr_id "," \
    "\"access_control\":{\"transcode\":" #_tr ",\"edit_acl\":" #_ed "}}"

#define  REQ_ITEM_1   ITEST_REQ_ITEM_GEN(13, true,  true)
#define  REQ_ITEM_2   ITEST_REQ_ITEM_GEN(24, false, false)
#define  REQ_ITEM_3   ITEST_REQ_ITEM_GEN(38, false, true)
#define  REQ_ITEM_4   ITEST_REQ_ITEM_GEN(59, true,  false)
#define  REQ_ITEM_5   ITEST_REQ_ITEM_GEN(16, true,  true)
#define  REQ_ITEM_6   ITEST_REQ_ITEM_GEN(13, false, false)
#define  REQ_ITEM_7   ITEST_REQ_ITEM_GEN(38, true, false)
#define  REQ_ITEM_8   ITEST_REQ_ITEM_GEN(51, true, true)
#define  REQ_ITEM_9   ITEST_REQ_ITEM_GEN(66, false, false)
#define  REQ_ITEM_10  ITEST_REQ_ITEM_GEN(29, true, false)
#define  REQ_ITEM_11  ITEST_REQ_ITEM_GEN(71, false, true)
#define  REQ_ITEM_12  ITEST_REQ_ITEM_GEN(34, true, false)

#define  ULVL_ACL_URL_PATT  "https://localhost:8010/file/acl/usr?res_id=%s"
#define  FLVL_ACL_URL_PATT  "https://localhost:8010/file/acl?res_id=%s"

typedef struct {
    json_t  *upld_req;
    int  expect_resp_code;
    const char *req_body_serialtxt;
} itest_usrarg_t;

extern json_t *_app_itest_active_upload_requests;

static void _available_resource_lookup(json_t **upld_req, const char *lvl)
{
    json_t *req = NULL, *existing_acl = NULL;
    const char *res_id = NULL;
    int idx = 0;
    *upld_req = NULL;
    json_array_foreach(_app_itest_active_upload_requests, idx, req) {
        existing_acl = json_object_get(req, lvl);
        if(existing_acl)
            continue;
        res_id = json_string_value(json_object_get(req, "resource_id"));
        if(res_id) {
            *upld_req = req;
            break;
        }
    }
    if(!res_id)
        fprintf(stderr, "[edit_acl] line:%d, no resource available \n", __LINE__);
} // end of _available_resource_lookup


static void test_verify__usrlvl_acl_cb(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg)
{
    long actual_resp_code = 0;
    itest_usrarg_t  *usr_args = _usr_arg;
    CURLcode res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_args->expect_resp_code));
    if(usr_args->expect_resp_code == 200) {
        if(usr_args->req_body_serialtxt) {
            size_t  serialtxt_sz = strlen(usr_args->req_body_serialtxt);
            json_t *new_acl = json_loadb(usr_args->req_body_serialtxt, serialtxt_sz, 0, NULL);
            assert_that(new_acl, is_not_null);
            if(new_acl)
                json_object_set_new(usr_args->upld_req, "ulvl_acl", new_acl);
        } else {
            json_t *resp_obj = json_loadfd(privdata->fds.resp_body, 0, NULL);
            json_t *existing_acl = json_object_get(usr_args->upld_req, "acl");
            if(existing_acl && resp_obj) {
                json_t *item_i = NULL, *item_j = NULL;  int idx = 0, jdx = 0;
                json_array_foreach(resp_obj,idx,item_i) {
                    uint32_t  id0 = json_integer_value(json_object_get(item_i,"usr_id"));
                    assert_that(id0, is_not_equal_to(0));
                    json_t *i0_able = json_object_get(item_i,"access_control");
                    assert_that(i0_able, is_not_null);
                    json_array_foreach(existing_acl,jdx,item_j) {
                        uint32_t  id1 = json_integer_value(json_object_get(item_j,"usr_id"));
                        if(id0 == id1) {
                            json_t *i1_able = json_object_get(item_j,"access_control");
                            assert_that(i1_able, is_not_null);
                            uint8_t  transocde_flg_0 = json_boolean_value(json_object_get(i0_able,"transcode"));
                            uint8_t  transocde_flg_1 = json_boolean_value(json_object_get(i1_able,"transcode"));
                            assert_that(transocde_flg_0, is_equal_to(transocde_flg_1));
                            uint8_t  editacl_flg_0 = json_boolean_value(json_object_get(i0_able,"edit_acl"));
                            uint8_t  editacl_flg_1 = json_boolean_value(json_object_get(i1_able,"edit_acl"));
                            assert_that(editacl_flg_0, is_equal_to(editacl_flg_1));
                            break;
                        }
                    } // end of loop
                } // end of loop
            }
            if(resp_obj)
                json_decref(resp_obj);
        }
    } // resp status code == 200
} // end of  test_verify__usrlvl_acl_cb


#define  ITEST_ACL_COMMON_CODE_SETUP(level_label)  \
    const char *_res_id = json_string_value(json_object_get(upld_req, "resource_id")); \
    char *_res_id_urlencoded = curl_easy_escape(NULL, _res_id, strlen(_res_id)); \
    size_t url_sz = strlen(_res_id_urlencoded) + sizeof(level_label##_ACL_URL_PATT); \
    char url[url_sz]; \
    size_t nwrite = snprintf(&url[0], url_sz, level_label##_ACL_URL_PATT, _res_id_urlencoded); \
    assert(nwrite < url_sz); \
    const char *codename_list[2] = {"edit_file_access_control", NULL}; \
    json_t *header_kv_serials = json_array(); \
    json_array_append_new(header_kv_serials, json_string("Content-Type:application/json")); \
    json_array_append_new(header_kv_serials, json_string("Accept:application/json")); \
    json_t *quota = json_array(); \
    add_auth_token_to_http_header(header_kv_serials, auth_usr_id, codename_list, quota);

#define  ITEST_ACL_COMMON_CODE_TEARDOWN  \
    json_decref(header_kv_serials); \
    json_decref(quota); \
    free(_res_id_urlencoded);


static void _itest_edit_usrlvl_acl__common (json_t *upld_req, uint32_t auth_usr_id,
        const char *req_body_serialtxt,  int  expect_resp_code)
{
    ITEST_ACL_COMMON_CODE_SETUP(ULVL)
    test_setup_pub_t  setup_data = {
        .method = "PUT", .verbose = 0,  .url=&url[0],  .headers = header_kv_serials,
        .req_body = {.serial_txt=req_body_serialtxt, .src_filepath=NULL},
    };
    itest_usrarg_t  usr_args = {.upld_req=upld_req, .expect_resp_code=expect_resp_code,
           .req_body_serialtxt=req_body_serialtxt };
    run_client_request(&setup_data, test_verify__usrlvl_acl_cb, &usr_args);
    ITEST_ACL_COMMON_CODE_TEARDOWN
} // end of  _itest_edit_usrlvl_acl__common


static void _itest_read_usrlvl_acl__common (json_t *upld_req, uint32_t auth_usr_id, int expect_resp_code)
{
    ITEST_ACL_COMMON_CODE_SETUP(ULVL)
    test_setup_pub_t  setup_data = {.method="GET", .verbose=0,  .url=&url[0], .headers=header_kv_serials};
    itest_usrarg_t  usr_args = {.upld_req=upld_req, .expect_resp_code=expect_resp_code};
    run_client_request(&setup_data, test_verify__usrlvl_acl_cb, &usr_args);
    ITEST_ACL_COMMON_CODE_TEARDOWN
} // end of  _itest_read_usrlvl_acl__common


Ensure(api_edit_usrlvl_acl__test_ok)
{
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "ulvl_acl");
    if(!upld_req)
        return;
    { // subcase 1
        uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
        uint32_t  other_usr_ids[4] = {59,13,38,24};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_1","REQ_ITEM_2","REQ_ITEM_3","REQ_ITEM_4"]"
        const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
        itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 4, 0);
        _itest_edit_usrlvl_acl__common (upld_req, res_owner_id, req_body_serialtxt, 200);
        _itest_read_usrlvl_acl__common (upld_req, res_owner_id, 200);
    } { // subcase 2, usr_id = 13, granted with edit-acl permission, should be able to edit ACL of the resource
        uint32_t  other_usr_ids[5] = {16,13,38,51,66};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_5","REQ_ITEM_6","REQ_ITEM_7","REQ_ITEM_8","REQ_ITEM_9"]"
        const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
        itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 5, 0);
        _itest_edit_usrlvl_acl__common (upld_req, 13, req_body_serialtxt, 200);
        _itest_read_usrlvl_acl__common (upld_req, 13, 200);
    } { // subcase 3, usr_id = 51, granted with edit-acl permission, should be able to edit ACL of the resource
        uint32_t  other_usr_ids[4] = {13,24,59,51};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_2","REQ_ITEM_4","REQ_ITEM_1","REQ_ITEM_8"]"
        const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
        itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 4, 0);
        _itest_edit_usrlvl_acl__common (upld_req, 51, req_body_serialtxt, 200);
        _itest_read_usrlvl_acl__common (upld_req, 51, 200);
    } { // subcase 4, edit ACL of another resource
        _available_resource_lookup(&upld_req, "ulvl_acl");
        uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
        uint32_t  other_usr_ids[3] = {29,71,34};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_10","REQ_ITEM_11","REQ_ITEM_12"]"
        const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
        itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 3, 0);
        _itest_edit_usrlvl_acl__common (upld_req, res_owner_id, req_body_serialtxt, 200);
        _itest_read_usrlvl_acl__common (upld_req, res_owner_id, 200);
    }
} // end of  api_edit_usrlvl_acl__test_ok


Ensure(api_edit_usrlvl_acl__test_invalid_usr_in_reqbody)
{
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "ulvl_acl");
    if(!upld_req)
        return;
    uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    uint32_t  other_usr_ids[4] = {13,38,24,999};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_1","REQ_ITEM_2","REQ_ITEM_3","REQ_ITEM_4"]"
    const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
    itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 4, 0);
    _itest_edit_usrlvl_acl__common (upld_req, res_owner_id, req_body_serialtxt, 400);
}

Ensure(api_edit_usrlvl_acl__test_permission_denied)
{ // usr_id = 13, hasn't been added to ACL of the resource
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "ulvl_acl");
    if(!upld_req)
        return;
    uint32_t  other_usr_ids[3] = {13,38,24};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_1","REQ_ITEM_2","REQ_ITEM_3","REQ_ITEM_4"]"
    const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
    itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 3, 0);
    _itest_edit_usrlvl_acl__common (upld_req, 13, req_body_serialtxt, 403);
}

Ensure(api_edit_usrlvl_acl__test_rpc_no_response)
{
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "ulvl_acl");
    if(!upld_req)
        return;
    uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    uint32_t  other_usr_ids[2] = {13,38};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_1","REQ_ITEM_2"]"
    const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
    itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 2, 1);
    _itest_edit_usrlvl_acl__common (upld_req, res_owner_id, req_body_serialtxt, 503);
}

Ensure(api_edit_usrlvl_acl__test_ok2)
{
    json_t *upld_req = NULL;
    while(1) {
        _available_resource_lookup(&upld_req, "ulvl_acl");
        if(!upld_req)
            break;
        uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
        uint32_t  other_usr_ids[3] = {38,13,51};
#define  REQ_BODY_SERIALTXT  "["REQ_ITEM_3","REQ_ITEM_6","REQ_ITEM_8"]"
    const char *req_body_serialtxt = REQ_BODY_SERIALTXT;
#undef  REQ_BODY_SERIALTXT
        itest_rpc_usermgt__setup_usr_ids(&other_usr_ids[0], 3, 0);
        _itest_edit_usrlvl_acl__common (upld_req, res_owner_id, req_body_serialtxt, 200);
        _itest_read_usrlvl_acl__common (upld_req, res_owner_id, 200);
    } // end of loop
} // end of  api_edit_usrlvl_acl__test_ok2



static void test_verify__filelvl_acl_cb(CURL *handle, test_setup_priv_t *privdata, void *_usr_arg)
{
    long actual_resp_code = 0;
    itest_usrarg_t  *usr_args = _usr_arg;
    CURLcode res = curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &actual_resp_code);
    assert_that(res, is_equal_to(CURLE_OK));
    assert_that(actual_resp_code, is_equal_to(usr_args->expect_resp_code));
    if(usr_args->expect_resp_code == 200) {
        if(usr_args->req_body_serialtxt) {
            size_t  serialtxt_sz = strlen(usr_args->req_body_serialtxt);
            json_t *new_acl = json_loadb(usr_args->req_body_serialtxt, serialtxt_sz, 0, NULL);
            assert_that(new_acl, is_not_null);
            if(new_acl)
                json_object_set_new(usr_args->upld_req, "flvl_acl", new_acl);
        }
    }
} // end of  test_verify__filelvl_acl_cb

static void _itest_edit_filelvl_acl__common (json_t *upld_req, uint32_t auth_usr_id,
        const char *req_body_serialtxt,  int expect_resp_code)
{
    ITEST_ACL_COMMON_CODE_SETUP(FLVL)
    test_setup_pub_t  setup_data = {
        .method = "PATCH", .verbose = 0,  .url=&url[0], .headers = header_kv_serials,
        .req_body = {.serial_txt=req_body_serialtxt, .src_filepath=NULL},
    };
    itest_usrarg_t  usr_args = {.upld_req=upld_req, .expect_resp_code=expect_resp_code,
           .req_body_serialtxt=req_body_serialtxt };
    run_client_request(&setup_data, test_verify__filelvl_acl_cb, &usr_args);
    ITEST_ACL_COMMON_CODE_TEARDOWN
}


#define  UTEST_FLVL_ACL_REQ_BODY_1   "{\"visible\":true}"
#define  UTEST_FLVL_ACL_REQ_BODY_2   "{\"visible\":false}"
Ensure(api_edit_filelvl_acl__test_permission_denied) 
{
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "flvl_acl");
    if(!upld_req)
        return;
    uint32_t  other_usr_id = 999;
    _itest_edit_filelvl_acl__common (upld_req, other_usr_id, UTEST_FLVL_ACL_REQ_BODY_1,  403);
}// end of api_edit_filelvl_acl__test_permission_denied


Ensure(api_edit_filelvl_acl__test_ok) {
    json_t *upld_req = NULL;
    _available_resource_lookup(&upld_req, "flvl_acl");
    if(!upld_req)
        return;
    uint32_t  res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_1,  200);
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_2,  200);
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_1,  200);
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_2,  200);
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_1,  200);
    _available_resource_lookup(&upld_req, "flvl_acl");
    res_owner_id  = json_integer_value(json_object_get(upld_req, "usr_id"));
    _itest_edit_filelvl_acl__common (upld_req, res_owner_id, UTEST_FLVL_ACL_REQ_BODY_2,  200);
}// end of api_edit_filelvl_acl__test_ok
#undef   UTEST_FLVL_ACL_REQ_BODY_1
#undef   UTEST_FLVL_ACL_REQ_BODY_2


TestSuite *api_file_acl_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, api_edit_filelvl_acl__test_permission_denied);
    add_test(suite, api_edit_filelvl_acl__test_ok);
    add_test(suite, api_edit_usrlvl_acl__test_invalid_usr_in_reqbody);
    add_test(suite, api_edit_usrlvl_acl__test_permission_denied);
    add_test(suite, api_edit_usrlvl_acl__test_rpc_no_response);
    add_test(suite, api_edit_usrlvl_acl__test_ok);
    add_test(suite, api_edit_usrlvl_acl__test_ok2);
    return suite;
} // end of  api_file_acl_tests
