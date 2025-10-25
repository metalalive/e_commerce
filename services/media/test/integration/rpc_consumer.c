#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <sysexits.h>
#include <unistd.h>
#include <jansson.h>

#include "app_cfg.h"
#include "datatypes.h"
#include "utils.h"
#include "cfg_parser.h"
#include "rpc/cfg_parser.h"
#include "rpc/core.h"
#include "rpc/consumer.h"
#include "../test/integration/test.h"

static __attribute__((optimize("O0"))) void itest_rpc_handler__verify_usr_ids(arpc_receipt_t *receipt) {
    // this function mimics python celery consumer, which is currently applied
    // to user_management app
    app_cfg_t *acfg = app_get_global_cfg();
#define PY_CELERY_RESP_PATTERN "{\"status\":null,\"result\":[]}"
    json_error_t jerror = {0};
    const char  *sys_basepath = acfg->env_vars.sys_base_path;
#define RUNNER(fullpath) json_load_file(fullpath, 0, NULL)
    json_t *mock_db = PATH_CONCAT_THEN_RUN(sys_basepath, ITEST_USERMGT_MOCK_DATABASE, RUNNER);
#undef RUNNER
    json_t *_usr_id_list = json_object_get(mock_db, "usr_ids");
    json_t *api_req =
        json_loadb((const char *)receipt->msg_body.bytes, receipt->msg_body.len, (size_t)0, &jerror);
    json_t *resp_body = json_loadb(PY_CELERY_RESP_PATTERN, sizeof(PY_CELERY_RESP_PATTERN) - 1, 0, NULL);
    json_t *lookup_ids_item = NULL, *usr_id_item = NULL, *app_result = NULL;
    uint8_t _no_resp = (uint8_t)json_boolean_value(json_object_get(mock_db, "no_resp"));
    int     idx = 0;
    // send the first reply to indicate this consumer received the message
    json_object_set_new(resp_body, "status", json_string("STARTED"));
    fprintf(
        stderr, "[DEBUG][mock_rpc] line:%d, sent STARTED reply, corr_id:%.*s\n", __LINE__,
        (int)receipt->job_id.len, receipt->job_id.bytes
    );
    app_rpc_task_send_reply(receipt, resp_body, 0);
    if (!api_req || !mock_db) {
        fprintf(
            stderr, "[itest][rpc][consumer] line:%d, api_req:%p, mock_db:%p \n", __LINE__, api_req, mock_db
        );
        goto error;
    } else {
        json_t *item_o = json_array_get(api_req, 1);
        if (!item_o) {
            fprintf(stderr, "[itest][rpc][consumer] line:%d, api_req error \n", __LINE__);
            goto error;
        }
        lookup_ids_item = json_object_get(item_o, "ids");
        json_t *fields_item = json_object_get(item_o, "fields");
        if (!lookup_ids_item || !fields_item)
            goto error;
        if (!json_is_array(lookup_ids_item) || !json_is_array(fields_item))
            goto error;
        if (json_array_size(lookup_ids_item) == 0 || json_array_size(fields_item) == 0)
            goto error;
    }
    app_result = json_object_get(resp_body, "result");
    json_array_foreach(lookup_ids_item, idx, usr_id_item) {
        json_t  *db_item = NULL;
        int      jdx = 0;
        uint32_t id0 = json_integer_value(usr_id_item);
        json_array_foreach(_usr_id_list, jdx, db_item) {
            uint32_t id1 = json_integer_value(db_item);
            if (id0 == id1) {
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
    json_object_set_new(resp_body, "status", json_string("ERROR"));
    fprintf(
        stderr, "[itest][rpc][consumer] line:%d, error, original msg: %s \n", __LINE__,
        (const char *)receipt->msg_body.bytes
    );
done:
    // send the second reply to indicate this consumer has done the task and returned the final
    // output
    if (!_no_resp) {
        fprintf(
            stderr, "[DEBUG][mock_rpc] line:%d, sent final reply, status:%s, corr_id:%.*s\n", __LINE__,
            json_string_value(json_object_get(resp_body, "status")), (int)receipt->job_id.len,
            receipt->job_id.bytes
        );
        app_rpc_task_send_reply(receipt, resp_body, 1);
    }
    if (mock_db)
        json_decref(mock_db);
    if (api_req)
        json_decref(api_req);
    json_decref(resp_body);
#undef PY_CELERY_RESP_PATTERN
} // end of  itest_rpc_handler__verify_usr_ids

static int itest_parse_cfg_params(const char *cfg_file_path, app_cfg_t *app_cfg) {
    int          err = EX_OK;
    json_error_t jerror;
#define RUNNER(fullpath) json_load_file(fullpath, (size_t)0, &jerror);
    json_t *root = PATH_CONCAT_THEN_RUN(app_cfg->env_vars.sys_base_path, cfg_file_path, RUNNER);
#undef RUNNER
    if (!json_is_object(root)) {
        h2o_error_printf(
            "[parsing] decode error on JSON file %s at line %d, column %d\n", &jerror.source[0], jerror.line,
            jerror.column
        );
        err = EX_CONFIG;
        goto error;
    }
    json_t *err_log = json_object_get((const json_t *)root, "error_log");
    json_t *filepath = json_object_get((const json_t *)err_log, "rpc_consumer");
    err = appcfg_parse_errlog_path(filepath, app_cfg);
    if (err) {
        goto error;
    }
    err = parse_cfg_rpc_callee(json_object_get((const json_t *)root, "rpc"), app_cfg);
error:
    if (root) {
        json_decref(root);
    }
    return err;
} // end of itest_parse_cfg_params

int main(int argc, char *argv[]) {
    assert(argc > 2);
    const char *cfg_file_path = argv[argc - 1];
    // ensure relative path to executable program name ,
    // Note `argv[0]` is also the path to prgoram, however it might be full path
    // or relative path depending on system environment, to reduce such uncertainty
    // executable path is always retrieved from user-defined argument.
    const char *exe_path = argv[argc - 2];
    app_cfg_t  *acfg = app_get_global_cfg();
    app_global_cfg_set_exepath(exe_path);
    int err = itest_parse_cfg_params(cfg_file_path, acfg);
    if (!err) {
#define NUM_THREADS 1 // main thread included
        struct worker_init_data_t worker_data[NUM_THREADS];
        err = appcfg_start_workers(acfg, &worker_data[0], run_app_worker);
        appcfg_terminate_workers(acfg, &worker_data[0]);
#undef NUM_THREADS
    }
    deinit_app_cfg(acfg);
    return err;
}
