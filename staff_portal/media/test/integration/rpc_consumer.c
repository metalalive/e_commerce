#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <jansson.h>

#include "rpc/core.h"

#define  ITEST_USERMGT_MOCK_DATABASE  "tmp/log/test/media_rpc_usermgt_mock_db.json"

static  __attribute__((optimize("O0"))) void  itest_rpc_handler__verify_usr_ids (arpc_receipt_t *receipt)
{ // this function mimics python celery consumer, which is currently applied to user_management app
#define  PY_CELERY_RESP_PATTERN  "{\"status\":null,\"result\":[]}"
    json_error_t jerror = {0};
    json_t *mock_db = json_load_file(ITEST_USERMGT_MOCK_DATABASE, 0, NULL);
    json_t *_usr_id_list = json_object_get(mock_db, "usr_ids");
    json_t *api_req = json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    json_t *resp_body = json_loadb(PY_CELERY_RESP_PATTERN, sizeof(PY_CELERY_RESP_PATTERN) - 1, 0, NULL);
    json_t *lookup_ids_item = NULL, *usr_id_item = NULL, *app_result = NULL;
    uint8_t _no_resp = (uint8_t)json_boolean_value(json_object_get(mock_db, "no_resp"));
    int idx = 0;
    // send the first reply to indicate this consumer received the message
    json_object_set_new(resp_body, "status", json_string("STARTED")) ;
    app_rpc_task_send_reply(receipt, resp_body, 0);
    if(!api_req || !mock_db) {
        fprintf(stderr, "[itest][rpc][consumer] line:%d, api_req:%p, mock_db:%p \n",
                __LINE__, api_req, mock_db);
        goto error;
    } else {
        json_t *item_o = json_array_get(api_req, 1);
        if(!item_o) {
            fprintf(stderr, "[itest][rpc][consumer] line:%d, api_req error \n",__LINE__);
            goto error;
        }
        lookup_ids_item  = json_object_get(item_o, "ids");
        json_t *fields_item = json_object_get(item_o, "fields");
        if(!lookup_ids_item || !fields_item)
            goto error;
        if(!json_is_array(lookup_ids_item) || !json_is_array(fields_item))
            goto error;
        if(json_array_size(lookup_ids_item) == 0 || json_array_size(fields_item) == 0)
            goto error;
    }
    app_result = json_object_get(resp_body, "result");
    json_array_foreach(lookup_ids_item, idx, usr_id_item) {
        json_t *db_item = NULL;  int jdx = 0;
        uint32_t  id0 = json_integer_value(usr_id_item);
        json_array_foreach(_usr_id_list, jdx, db_item) {
            uint32_t  id1 = json_integer_value(db_item);
            if(id0 == id1) {
                json_t *item_o = json_object();
                json_object_set(item_o, "id", usr_id_item);
                json_array_append_new(app_result, item_o);
                break;
            }
        } // end of loop
    } // end of loop
    json_object_set_new(resp_body, "status", json_string("SUCCESS"));
    goto done;
error:
    json_object_set_new(resp_body, "status", json_string("ERROR")) ;
    fprintf(stderr, "[itest][rpc][consumer] line:%d, error, original msg: %s \n",
            __LINE__,  (const char *)receipt->msg_body.bytes);
done:
    // send the second reply to indicate this consumer has done the task and returned the final output
    if(!_no_resp)
        app_rpc_task_send_reply(receipt, resp_body, 1);
    if(mock_db)
        json_decref(mock_db);
    if(api_req)
        json_decref(api_req);
    json_decref(resp_body);
#undef  PY_CELERY_RESP_PATTERN
} // end of  itest_rpc_handler__verify_usr_ids


void  itest_rpc_usermgt__setup_usr_ids(uint32_t *in, size_t in_sz, uint8_t _no_resp)
{
    int idx = 0, target_fd = open(ITEST_USERMGT_MOCK_DATABASE, O_RDWR|O_CREAT, S_IRUSR|S_IWUSR);
    json_t *info = json_object(), *usr_id_list = json_array();
    for(idx = 0; idx < in_sz; idx++)
        json_array_append_new(usr_id_list, json_integer(in[idx]));
    json_object_set_new(info, "usr_ids", usr_id_list);
    json_object_set_new(info, "no_resp", json_boolean(_no_resp));
    ftruncate(target_fd, (off_t)0);
    lseek(target_fd, 0, SEEK_SET);
    json_dumpfd((const json_t *)info, target_fd, JSON_COMPACT);  // will call low-level write() without buffering this
    if(target_fd >= 0)
        close(target_fd);
    json_decref(info);
} // end of  itest_rpc_usermgt__setup_usr_ids
