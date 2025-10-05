#include "routes.h"
#include "storage/cfg_parser.h"

typedef struct {
    asa_cfg_t *app_cfg;
    json_t    *json_ops;
} asa_internal_cb_arg_t;

#define STORAGE_OPS_FN_MAPPING(attribute, args, fn_name, fn_entry) \
    { \
        const char *target_fn_name = json_string_value(json_object_get((args)->json_ops, #attribute)); \
        if (!strcmp(target_fn_name, (fn_name))) { \
            (args)->app_cfg->ops.fn_##attribute = (fn_entry); \
            goto done; \
        } \
    }

#define STORAGE_ALL_OPS_READY(flg, attribute, args) (flg) && ((args)->app_cfg->ops.fn_##attribute != NULL)

static uint8_t _app_elf_gather_storage_operation_cb(char *fn_name, void *entry_point, void *cb_args) {
    uint8_t                immediate_stop = 1;
    asa_internal_cb_arg_t *args = (asa_internal_cb_arg_t *)cb_args;
    STORAGE_OPS_FN_MAPPING(mkdir, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(rmdir, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(scandir, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(scandir_next, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(rename, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(unlink, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(open, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(close, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(write, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(read, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(seek, args, fn_name, entry_point);
    STORAGE_OPS_FN_MAPPING(typesize, args, fn_name, entry_point);
done:
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, mkdir, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, rmdir, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, scandir, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, scandir_next, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, rename, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, unlink, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, open, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, close, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, write, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, read, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, seek, args);
    immediate_stop = STORAGE_ALL_OPS_READY(immediate_stop, typesize, args);
    return immediate_stop;
} // end of _app_elf_gather_storage_operation_cb
#undef STORAGE_OPS_FN_MAPPING
#undef STORAGE_ALL_OPS_READY

int parse_cfg_storages(json_t *objs, app_cfg_t *app_cfg) {
    json_t *obj = NULL;
    uint8_t realloc_mem = 0;
    int     idx = 0;
    if (!objs || !app_cfg || !app_cfg->exe_path) {
        goto error;
    }
    size_t prev_num_storage_setup = app_cfg->storages.capacity;
    size_t num_storage_setup = json_array_size(objs);
    if (prev_num_storage_setup < num_storage_setup) {
        h2o_vector_reserve(NULL, &app_cfg->storages, num_storage_setup);
        num_storage_setup = app_cfg->storages.capacity; // h2o_vector_reserve() may allocates extra space
        realloc_mem = 1;
        size_t sz = sizeof(asa_cfg_t) * (num_storage_setup - prev_num_storage_setup);
        void  *ptr = &app_cfg->storages.entries[prev_num_storage_setup];
        memset(ptr, 0x0, sz);
    }
    app_cfg->storages.size = 0;
    json_array_foreach(objs, idx, obj) {
        const char *alias = json_string_value(json_object_get(obj, "alias"));
        const char *base_path = json_string_value(json_object_get(obj, "base_path"));
        if (!alias || !base_path) {
            h2o_error_printf("[parsing] storage (idx=%d) missing alias or base_path \n", idx);
            goto error;
        }
        json_t *ops = json_object_get(obj, "ops");
        if (!ops || !json_is_object(ops) || !json_object_get(ops, "open") || !json_object_get(ops, "close") ||
            !json_object_get(ops, "seek") || !json_object_get(ops, "write") ||
            !json_object_get(ops, "read") || !json_object_get(ops, "scandir") ||
            !json_object_get(ops, "scandir_next") || !json_object_get(ops, "rename") ||
            !json_object_get(ops, "unlink") || !json_object_get(ops, "mkdir") ||
            !json_object_get(ops, "rmdir") || !json_object_get(ops, "typesize")) {
            h2o_error_printf(
                "[parsing] storage (idx=%d) must include function names for all operations \n", idx
            );
            goto error;
        }
        // TODO, check duplicate alias , return error if found
        asa_cfg_t *_asa_cfg = &app_cfg->storages.entries[app_cfg->storages.size++];
        if (_asa_cfg->alias) {
            free(_asa_cfg->alias);
        }
        if (_asa_cfg->base_path) {
            free(_asa_cfg->base_path);
        }
        _asa_cfg->alias = strdup(alias);
        _asa_cfg->base_path = strdup(base_path);
        _asa_cfg->ops = (asa_cfg_ops_t){0};
        asa_internal_cb_arg_t cb_args = {.app_cfg = _asa_cfg, .json_ops = ops};
        int                   err = app_elf_traverse_functions(
            app_cfg->exe_path, _app_elf_gather_storage_operation_cb, (void *)&cb_args
        );
        if (err != 0) {
            goto error;
        }
        if (!_asa_cfg->ops.fn_open || !_asa_cfg->ops.fn_close || !_asa_cfg->ops.fn_seek ||
            !_asa_cfg->ops.fn_write || !_asa_cfg->ops.fn_read || !_asa_cfg->ops.fn_scandir_next ||
            !_asa_cfg->ops.fn_scandir || !_asa_cfg->ops.fn_rename || !_asa_cfg->ops.fn_unlink ||
            !_asa_cfg->ops.fn_mkdir || !_asa_cfg->ops.fn_rmdir || !_asa_cfg->ops.fn_typesize) {
            h2o_error_printf("[parsing] storage (idx=%d) failed to look up all necessary operations \n", idx);
            goto error;
        } // TODO, check whether any operation function is missing
    } // end of storage configuration iteration
    return 0;
error:
    if (realloc_mem && app_cfg->storages.entries) {
        app_storage_cfg_deinit(app_cfg);
    }
    return -1;
} // end of parse_cfg_storages

void app_storage_cfg_deinit(app_cfg_t *app_cfg) {
    size_t idx = 0;
    for (idx = 0; idx < app_cfg->storages.size; idx++) {
        asa_cfg_t *_asa_cfg = &app_cfg->storages.entries[idx];
        if (_asa_cfg->alias) {
            free(_asa_cfg->alias);
        }
        if (_asa_cfg->base_path) {
            free(_asa_cfg->base_path);
        }
    }
    free(app_cfg->storages.entries);
    app_cfg->storages.entries = NULL;
    app_cfg->storages.capacity = 0;
    app_cfg->storages.size = 0;
} // end of app_storage_cfg_deinit

asa_cfg_t *app_storage_cfg_lookup(const char *alias) {
    asa_cfg_t *out = NULL;
    app_cfg_t *app_cfg = app_get_global_cfg();
    for (int idx = 0; !out && (idx < app_cfg->storages.size); idx++) {
        asa_cfg_t *item = &app_cfg->storages.entries[idx];
        int        ret = strncmp(alias, item->alias, strlen(item->alias));
        if (!ret)
            out = item;
    }
    return out;
}

asa_op_base_cfg_t *app_storage__init_asaobj_helper(
    asa_cfg_t *storage, uint8_t num_cb_args, uint32_t rd_buf_bytes, uint32_t wr_buf_bytes
) {
    asa_op_base_cfg_t *out = NULL;
    size_t             cb_args_tot_sz = sizeof(void *) * num_cb_args;
    size_t             asaobj_sz = storage->ops.fn_typesize();
    size_t             asaobj_tot_sz = asaobj_sz + cb_args_tot_sz + rd_buf_bytes + wr_buf_bytes;
    out = calloc(1, asaobj_tot_sz);
    char *ptr = (char *)out + asaobj_sz;
    out->cb_args.size = num_cb_args;
    out->cb_args.entries = (void **)ptr;
    out->storage = storage;
    ptr += cb_args_tot_sz;
    out->op.read.offset = 0;
    out->op.read.dst_max_nbytes = rd_buf_bytes;
    out->op.read.dst_sz = 0;
    if (rd_buf_bytes > 0)
        out->op.read.dst = (char *)ptr;
    ptr += rd_buf_bytes;
    out->op.write.offset = 0;
    out->op.write.src_max_nbytes = wr_buf_bytes;
    out->op.write.src_sz = 0;
    if (wr_buf_bytes > 0)
        out->op.write.src = (char *)ptr;
    ptr += wr_buf_bytes;
    assert((size_t)(ptr - (char *)out) == asaobj_tot_sz);
    return out;
} // end of app_storage__init_asaobj_helper
