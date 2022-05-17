#include <assert.h>
#include "rpc/cfg_parser.h"
#include "rpc/core.h"
#include "rpc/consumer.h"

static int parse_cfg_num_workers(json_t *obj, app_cfg_t *app_cfg) {
    if(!obj) {
        goto error;
    } // In this application, number of worker threads excludes the main thread
    int new_capacity = (int) json_integer_value(obj);
    if (new_capacity < 0) {
        goto error;
    }
    h2o_vector_reserve(NULL, &app_cfg->workers, (size_t)new_capacity);
    // preserve space first, update thread ID later
    app_cfg->workers.size = new_capacity;
    return 0;
error:
    return -1;
}

static int parse_cfg_params(const char *cfg_file_path, app_cfg_t *app_cfg)
{
    int err = 0;
    json_error_t jerror;
    json_t  *root = NULL;
    root = json_load_file(cfg_file_path, (size_t)0, &jerror);
    if (!json_is_object(root)) {
        h2o_error_printf("[parsing] decode error on JSON file %s at line %d, column %d\n",
               &jerror.source[0], jerror.line, jerror.column);
        err = -1;
        goto error;
    }
    err = parse_cfg_num_workers(json_object_get((const json_t *)root, "num_rpc_consumers"), app_cfg);
    if (err) {  goto error; }
    err = parse_cfg_rpc_callee(json_object_get((const json_t *)root, "rpc"), app_cfg);
    json_decref(root);
    return 0;
error:
    if (!root) {
        json_decref(root);
    }
    return -1;
} // end of parse_cfg_params

static void run_loop(void *data) {
    int err = 0;
    //err = app_rpc_request_handler();
    return err;
} // end of run_loop

static int start_workers(app_cfg_t *app_cfg) {
    int err = 0;
    run_loop(NULL);
    return err;
} // end of start_workers

int start_application(const char *cfg_file_path, const char *exe_path)
{
    int err = 0;
    app_global_cfg_set_exepath(exe_path);
    err = parse_cfg_params(cfg_file_path, app_get_global_cfg());
    if(err) {goto done;}
    err = start_workers(app_get_global_cfg());
done:
    return err;
}
