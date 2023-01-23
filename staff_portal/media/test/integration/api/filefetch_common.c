#include <string.h>
#include <time.h>
#include <jansson.h>
#include <cgreen/cgreen.h>

extern json_t *_app_itest_active_upload_requests;

json_t * itest_filefetch_avail_resource_lookup(uint8_t public_access, const char *fsubtype_in)
{
    json_t *res_id_item = NULL, *chosen_upld_req = NULL, *upld_req = NULL,
           *async_jobs = NULL,  *job_item = NULL;
    int idx = 0, jdx = 0;
    json_array_foreach(_app_itest_active_upload_requests, idx, upld_req) {
        res_id_item  = json_object_get(upld_req, "resource_id");
        async_jobs   = json_object_get(upld_req, "async_job_ids");
        const char *_fsubtype_saved  = json_string_value(json_object_get(upld_req, "subtype"));
        if(!res_id_item || !async_jobs || !_fsubtype_saved)
            continue;
        uint8_t  type_matched = strncmp(_fsubtype_saved, fsubtype_in, strlen(_fsubtype_saved)) == 0;
        if(!type_matched)
            continue;
        if(json_object_get(upld_req, "streaming"))
            continue;
        uint8_t  transcoded_done_flag = 0;
        json_array_foreach(async_jobs, jdx, job_item) {
            uint8_t done_flag = (uint8_t) json_boolean_value(json_object_get(job_item, "done"));
            uint8_t err_flag = (uint8_t) json_boolean_value(json_object_get(job_item, "error"));
            transcoded_done_flag = done_flag && !err_flag;
            if(transcoded_done_flag)
                break;
        }
        if(!transcoded_done_flag) 
            continue;
        json_t *flvl_acl_item = json_object_get(upld_req, "flvl_acl");
        uint8_t  pub_visible = (uint8_t) json_boolean_value(json_object_get(flvl_acl_item, "visible"));
        uint8_t  take = (public_access && pub_visible) || (!public_access && !pub_visible);
        if(take) {
            chosen_upld_req = upld_req;
            break;
        }
    } // end of iteration of upload requests
    return  chosen_upld_req;
} // end of itest_filefetch_avail_resource_lookup


uint32_t  itest_fileftech__get_approved_usr_id (json_t *upld_req)
{
    uint32_t out = 0;
    json_t *_ulvl_acl = json_object_get(upld_req, "ulvl_acl");
    size_t  num_apprv_usrs = json_array_size(_ulvl_acl);
    assert_that(_ulvl_acl, is_not_null);
    assert_that(num_apprv_usrs, is_greater_than(0));
    if(_ulvl_acl && num_apprv_usrs > 0) {
        struct tm brokendown = {0};
        time_t curr_time = time(NULL);
        gmtime_r((const time_t *)&curr_time, &brokendown);
        int idx = (brokendown.tm_sec % num_apprv_usrs);
        json_t *item = json_array_get(_ulvl_acl, idx);
        out = (uint32_t) json_integer_value(json_object_get(item, "usr_id"));
        assert_that(out, is_greater_than(0));
    }
    return out;
} // end of  itest_fileftech__get_approved_usr_id
