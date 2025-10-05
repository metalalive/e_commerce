#include "routes.h"
#include "models/cfg_parser.h"
#include "models/pool.h"

typedef struct {
    db_3rdparty_ops_t *cfg;
    const char        *fn_label;
    uint8_t            done;
} app_internal_cb_arg_t;

static uint8_t _app_elf_gather_db_operation_cb(char *fn_name, void *entry_point, void *cb_args) {
    app_internal_cb_arg_t *args = (app_internal_cb_arg_t *)cb_args;
    uint8_t                immediate_stop = strcmp(args->fn_label, fn_name) == 0;
    if (immediate_stop) {
        void (*entry_fn)(db_3rdparty_ops_t *) = (void (*)(db_3rdparty_ops_t *))entry_point;
        entry_fn(args->cfg);
        args->done = 1;
    }
    return immediate_stop;
} // end of _app_elf_gather_db_operation_cb

static void parse_cfg_free_db_conn_detail(db_conn_cfg_t *detail) {
    if (detail->db_user) {
        free(detail->db_user);
        detail->db_user = NULL;
    }
    if (detail->db_passwd) {
        free(detail->db_passwd);
        detail->db_passwd = NULL;
    }
    if (detail->db_host) {
        free(detail->db_host);
        detail->db_host = NULL;
    }
}

static int parse_cfg_db_credential(json_t *in, db_conn_cfg_t *out) {
    const char *filepath = json_string_value(json_object_get(in, "filepath"));
    json_t     *hierarchy = json_object_get(in, "hierarchy");
    json_t     *root = NULL;
    json_t     *dst = NULL;
    json_t     *hier_tag = NULL;
    int         idx = 0;
    if (!filepath || !hierarchy || !json_is_array(hierarchy)) {
        h2o_error_printf("[parsing] missing filepath parameters in database credential\n");
        goto error;
    }
    root = json_load_file(filepath, (size_t)0, NULL);
    if (!root) {
        h2o_error_printf("[parsing] failed to load database credential from file %s \n", filepath);
        goto error;
    }
    dst = root;
    json_array_foreach(hierarchy, idx, hier_tag) {
        const char *tag = json_string_value(hier_tag);
        if (!tag) {
            h2o_error_printf("[parsing] invalid hierarchy in the database credential file : %s \n", filepath);
            goto error;
        }
        dst = json_object_get(
            dst, tag
        ); // valgrind may yell at here for strange memory error `Invalid read of size 4`
        if (!dst || !json_is_object(dst)) {
            h2o_error_printf(
                "[parsing] invalid json object in the database credential file : %s \n", filepath
            );
            goto error;
        }
    } // end of loop
    const char *db_user = json_string_value(json_object_get(dst, "USER"));
    const char *db_passwd = json_string_value(json_object_get(dst, "PASSWORD"));
    const char *db_host = json_string_value(json_object_get(dst, "HOST"));
    json_int_t  db_port = json_integer_value(json_object_get(dst, "PORT"));
    int         invalid_port_num = (db_port >= 0xFFFF) || (db_port <= 0);
    if (!db_user || !db_passwd || !db_host || invalid_port_num) {
        h2o_error_printf(
            "[parsing] invalid database credential: db_user(%s), db_passwd(%s), db_host(%s), "
            "db_port(%lld) \n",
            (db_user ? "not null" : "null"), (db_passwd ? "not null" : "null"), db_host, db_port
        );
        goto error;
    }
    out->db_user = strdup(db_user);
    out->db_passwd = strdup(db_passwd);
    out->db_host = strdup(db_host);
    out->db_port = (uint16_t)(db_port & 0xFFFF);
    json_decref(root);
    return 0;
error:
    if (root) {
        json_decref(root);
    }
    return EX_CONFIG;
} // end of parse_cfg_db_credential

int parse_cfg_databases(json_t *objs, app_cfg_t *app_cfg) {
    json_t *obj = NULL;
    int     idx = 0;
    size_t  num_pools = (size_t)json_array_size(objs);
    if (!objs || !json_is_array(objs) || num_pools == 0) {
        h2o_error_printf("[parsing][db] missing config\n");
        goto error;
    }
    json_array_foreach(objs, idx, obj) {
        const char *alias = json_string_value(json_object_get(obj, "alias"));
        const char *db_name = json_string_value(json_object_get(obj, "db_name"));
        const char *init_cfg_ops_label = json_string_value(json_object_get(obj, "init_cfg_ops"));
        uint32_t    max_conns = (uint32_t)json_integer_value(json_object_get(obj, "max_connections"));
        uint32_t    idle_timeout = (uint32_t)json_integer_value(json_object_get(obj, "idle_timeout"));
        uint32_t    bulk_query_limit_kb =
            (uint32_t)json_integer_value(json_object_get(obj, "bulk_query_limit_kb"));
        uint8_t skip_tls = (uint32_t)json_boolean_value(json_object_get(obj, "skip_tls"));
        json_t *credential = json_object_get(obj, "credential");
        if (!alias || !db_name || !init_cfg_ops_label || max_conns == 0 || idle_timeout == 0 ||
            bulk_query_limit_kb == 0) {
            h2o_error_printf("[parsing][db] missing general parameters in config\n");
            goto error;
        } else if (!credential || !json_is_object(credential)) {
            h2o_error_printf("[parsing][db] missing credential parameters\n");
            goto error;
        }
        db_pool_cfg_t cfg_opts = {
            .alias = (char *)alias,
            .capacity = max_conns,
            .idle_timeout = idle_timeout,
            .bulk_query_limit_kb = bulk_query_limit_kb,
            .conn_detail = {.db_name = (char *)db_name},
            .ops = {0},
            .skip_tls = skip_tls
        };
        app_internal_cb_arg_t probe_args = {.cfg = &cfg_opts.ops, .done = 0, .fn_label = init_cfg_ops_label};
        int                   err = app_elf_traverse_functions(
            app_cfg->exe_path, _app_elf_gather_db_operation_cb, (void *)&probe_args
        );
        if (err != 0) { // parsing error
            goto error;
        } else if (!probe_args.done) {
            h2o_error_printf(
                "[parsing][db][cfg] idx=%d, alias=%s, failed to look up all \
                    necessary operations \n",
                idx, alias
            );
            goto error;
        }
        if (parse_cfg_db_credential(credential, &cfg_opts.conn_detail)) {
            h2o_error_printf(
                "[parsing][db][cfg] idx=%d, alias=%s, failed to parse \
                    credential \n",
                idx, alias
            );
            parse_cfg_free_db_conn_detail(&cfg_opts.conn_detail);
            goto error;
        }
        if (app_db_pool_init(&cfg_opts) != DBA_RESULT_OK) {
            h2o_error_printf("[parsing][db][cfg] idx=%d, alias=%s, failed to init pool \n", idx, alias);
            parse_cfg_free_db_conn_detail(&cfg_opts.conn_detail);
            goto error;
        }
        parse_cfg_free_db_conn_detail(&cfg_opts.conn_detail);
    } // end of database configuration iteration
    return 0;
error:
    app_db_pool_map_deinit();
    return EX_CONFIG;
} // end of parse_cfg_databases
