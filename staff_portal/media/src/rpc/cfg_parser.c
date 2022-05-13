#include "routes.h"
#include "rpc/core.h"
#include "rpc/cfg_parser.h"

#define RPC_QUEUE_MIN_NUM_MSGS   300
#define RPC_QUEUE_MAX_NUM_MSGS  8000

typedef struct {
    arpc_cfg_bind_reply_t *cfg;
    const char *qname_fn_name;
    const char *corr_id_fn_name;
    uint8_t  all_fns_found:1;
} rpc_reply_cfg_internal_t;


static void app_rpc_cfg_binding_deinit(arpc_cfg_bind_t *cfg_list, size_t nitem) {
    for(size_t idx = 0; idx < nitem; idx++) {
        arpc_cfg_bind_t *cfg = &cfg_list[idx];
        if(cfg->q_name) {
            free(cfg->q_name);
        }
        if(cfg->exchange_name) {
            free(cfg->exchange_name);
        }
        if(cfg->routing_key) {
            free(cfg->routing_key);
        }
        if(cfg->reply.queue.name_pattern) {
            free(cfg->reply.queue.name_pattern);
        }
        if(cfg->reply.correlation_id.name_pattern) {
            free(cfg->reply.correlation_id.name_pattern);
        }
    }
    memset(cfg_list, 0x0, sizeof(arpc_cfg_bind_t) * nitem);
    free(cfg_list);
} // end of app_rpc_cfg_binding_deinit

int app_rpc_cfg_deinit(arpc_cfg_t *cfg) {
    if(!cfg) { return -1; }
    if(cfg->alias) {
        free(cfg->alias);
    }
    if(cfg->credential.username) {
        free(cfg->credential.username);
    }
    if(cfg->credential.password) {
        free(cfg->credential.password);
    }
    if(cfg->credential.host) {
        free(cfg->credential.host);
    }
    if(cfg->attributes.vhost) {
        free(cfg->attributes.vhost);
    }
    if(cfg->bindings.entries) {
        app_rpc_cfg_binding_deinit(&cfg->bindings.entries[0], cfg->bindings.capacity);
    }
    memset(cfg, 0x0, sizeof(arpc_cfg_t));
    return 0;
} // end of app_rpc_cfg_deinit


static int parse_cfg_rpc__broker_credential(json_t *in, arpc_cfg_t *out)
{
    const char *filepath = json_string_value(json_object_get(in, "filepath"));
    json_t  *hierarchy = json_object_get(in, "hierarchy");
    json_t  *root = NULL;
    json_t  *dst  = NULL;
    json_t  *hier_tag = NULL;
    size_t   idx = 0;
    if(!filepath || !hierarchy || !json_is_array(hierarchy)) {
        h2o_error_printf("[parsing] missing filepath parameters in message broker credential\n");
        goto error;
    }
    root = json_load_file(filepath, (size_t)0, NULL);
    if(!root) {
        h2o_error_printf("[parsing] failed to load message broker credential from file %s \n", filepath);
        goto error;
    }
    dst = root;
    json_array_foreach(hierarchy, idx, hier_tag) {
        if(json_is_string(hier_tag)) {
            const char *tag = json_string_value(hier_tag);
            dst = json_object_get(dst, tag);
        } else if (json_is_integer(hier_tag)) {
            int tag = (int) json_integer_value(hier_tag);
            dst = json_array_get(dst, tag);
        } else {
            h2o_error_printf("[parsing] invalid hierarchy in the AMQP-broker credential file : %s \n",filepath);
            goto error;
        }
        if(!dst) {
            h2o_error_printf("[parsing] invalid json object in the AMQP-broker credential file : %s \n", filepath);
            goto error;
        }
    } // end of loop
    const char *username = json_string_value(json_object_get(dst, "username"));
    const char *password = json_string_value(json_object_get(dst, "password"));
    const char *host = json_string_value(json_object_get(dst, "host"));
    uint16_t    port = (uint16_t) json_integer_value(json_object_get(dst, "port"));
    if(!username || !password || !host || port == 0) {
        h2o_error_printf("[parsing] invalid AMQP-broker credential: username(%s), password(%s), host(%s), port(%hu) \n",
                    (username?"not null":"null"), (password?"not null":"null"), host, port);
        goto error;
    }
    out->credential.username = strdup(username);
    out->credential.password = strdup(password);
    out->credential.host = strdup(host);
    out->credential.port = port;
    json_decref(root);
    return 0;
error:
    if(root) {
        json_decref(root);
    }
    return -1;
} // end of parse_cfg_rpc__broker_credential


static int parse_cfg_rpc__broker_attributes(json_t *in, arpc_cfg_t *out)
{
    const char *vhost = json_string_value(json_object_get(in, "vhost"));
    size_t max_channels     = (size_t) json_integer_value(json_object_get(in, "max_channels"));
    size_t max_kb_per_frame = (size_t) json_integer_value(json_object_get(in, "max_kb_per_frame"));
    if(!vhost || max_channels == 0 || max_kb_per_frame == 0) {
        h2o_error_printf("[parsing] missing parameters in message broker attributes\n");
        goto error;
    }
    out->attributes.vhost = strdup(vhost);
    out->attributes.max_channels     = max_channels    ;
    out->attributes.max_kb_per_frame = max_kb_per_frame;
    return 0;
error:
    return -1;
} // end of parse_cfg_rpc__broker_attributes


static uint8_t _app_elf_gather_rpc_reply_render_fns_cb(char *fn_name, void *entry_point, void *cb_args)
{
    rpc_reply_cfg_internal_t *args = (rpc_reply_cfg_internal_t *) cb_args;
    arpc_cfg_bind_reply_t *cfg = args->cfg;
    if(args->qname_fn_name && strcmp(fn_name, args->qname_fn_name) == 0) {
        cfg->queue.render_fn = (arpc_replyq_render_fn)entry_point;
    }
    if(args->corr_id_fn_name && strcmp(fn_name, args->corr_id_fn_name) == 0) {
        cfg->correlation_id.render_fn = (arpc_replyq_render_fn)entry_point;
    }
    uint8_t queue_render_fn_found  = (!args->qname_fn_name) || (cfg->queue.render_fn);
    uint8_t corrid_render_fn_found = (!args->corr_id_fn_name) || (cfg->correlation_id.render_fn);
    uint8_t immediate_stop = queue_render_fn_found && corrid_render_fn_found;
    args->all_fns_found = immediate_stop;
    return immediate_stop;
} // end of _app_elf_gather_rpc_reply_render_fns_cb

// TODO, find out the function address by parsing elf file on consumer side
static  int parse_cfg_rpc__reply_producer(json_t *obj, arpc_cfg_bind_reply_t *cfg)
{
    if(!obj) {
        h2o_error_printf("[parsing] missing configuration for RPC reply \n");
        goto error;
    }
    json_t *qname_obj   = json_object_get(obj, "queue");
    json_t *corr_id_obj = json_object_get(obj, "correlation_id");
    if(!qname_obj || !corr_id_obj) {
        h2o_error_printf("[parsing] missing queue and correlation_id parameters for RPC reply \n");
        goto error;
    }
    const char *qname_patt   = json_string_value(json_object_get(qname_obj, "pattern"));
    const char *corr_id_patt = json_string_value(json_object_get(corr_id_obj, "pattern"));
    if(!qname_patt || !corr_id_patt) {
        h2o_error_printf("[parsing] missing pattern of queue name or correlation ID for RPC reply \n");
        goto error;
    }
    cfg->queue.render_fn = NULL;
    cfg->correlation_id.render_fn = NULL;
    {
        const char *qname_fn_name   = json_string_value(json_object_get(qname_obj, "render_fn"));
        const char *corr_id_fn_name = json_string_value(json_object_get(corr_id_obj, "render_fn"));
        rpc_reply_cfg_internal_t cb_args = {.cfg = cfg, .qname_fn_name = qname_fn_name,
            .corr_id_fn_name = corr_id_fn_name, .all_fns_found = 0};
        int err = app_elf_traverse_functions(app_get_global_cfg()->exe_path,
                _app_elf_gather_rpc_reply_render_fns_cb, (void *)&cb_args);
        if(!cb_args.all_fns_found) {
            h2o_error_printf("[parsing] missing rendering function for RPC reply \n");
            goto error;
        } else if(err != 0) {
            goto error;
        }
    }
    cfg->queue.name_pattern = strdup(qname_patt);
    cfg->correlation_id.name_pattern = strdup(corr_id_patt);
    cfg->ttl_sec = (uint32_t) json_integer_value(json_object_get(obj, "ttl_sec"));
    cfg->flags = (arpc_qcfg_flg_t) {.passive = 1, .exclusive = 0, .auto_delete = 0,
        .durable = (uint8_t) json_boolean_value(json_object_get(obj, "durable"))
    };
    return 0;
error:
    return -1;
} // end of parse_cfg_rpc__reply_producer

static int parse_cfg_rpc__broker_bindings(json_t *objs, arpc_cfg_t *cfg)
{
    json_t *obj = NULL;
    size_t  idx = 0;
    size_t  num_binds_setup = json_array_size(objs);
    {
        assert(cfg->bindings.entries == NULL);
        h2o_vector_reserve(NULL, &cfg->bindings, num_binds_setup);
        num_binds_setup = cfg->bindings.capacity;
        memset(cfg->bindings.entries, 0x0, sizeof(arpc_cfg_bind_t) * num_binds_setup);
    }
    cfg->bindings.size = 0;
    json_array_foreach(objs, idx, obj) {
        const char *queue = json_string_value(json_object_get(obj, "queue"));
        const char *exchange = json_string_value(json_object_get(obj, "exchange"));
        const char *routing_key = json_string_value(json_object_get(obj, "routing_key"));
        size_t   max_msgs_pending = (size_t) json_integer_value(json_object_get(obj, "max_msgs_pending"));
        uint8_t  durable = (uint8_t) json_boolean_value(json_object_get(obj, "durable"));
        if(!queue || !exchange || !routing_key) {
            h2o_error_printf("[parsing] missing binding parameters (idx=%lu) in message broker configuration (%s:%hu) \n",
                    idx, cfg->credential.host, cfg->credential.port);
            goto error;
        }
        if(max_msgs_pending > RPC_QUEUE_MAX_NUM_MSGS) {
            h2o_error_printf("[parsing] max_msgs_pending (%lu) exceeds system limit (%d) in queue (%s), binding configuration (idx=%lu)\n",
                    max_msgs_pending, RPC_QUEUE_MAX_NUM_MSGS, queue, idx);
            goto error;
        } else if(max_msgs_pending < RPC_QUEUE_MIN_NUM_MSGS) {
            max_msgs_pending = RPC_QUEUE_MIN_NUM_MSGS;
        }
        arpc_cfg_bind_t *bcfg = &cfg->bindings.entries[ cfg->bindings.size++ ];
        *bcfg = (arpc_cfg_bind_t) {.q_name = strdup(queue), .exchange_name = strdup(exchange),
            .routing_key = strdup(routing_key), .max_msgs_pending = max_msgs_pending };
        // * if passive = 0 and the queue exists, the queue should have the same values for durable
        //   , exclusive, auto-delete, and all other attributes associated with the queue
        //   , otherwise such comparison will NOT be performed if the queue does NOT exists.
        // * Also, since passive flag is unset, this application automatically create the queue with
        //   the given name on initialization if the queue does not exist.
        bcfg->flags = (arpc_qcfg_flg_t) {.durable=durable, .passive=0, .exclusive=0, .auto_delete=0};
    } // end of binding iteration
    return 0;
error:
    return -1;
} // end of parse_cfg_rpc__broker_bindings


static int parse_cfg_rpc_common(json_t *objs, app_cfg_t *app_cfg)
{
    int err = 0;
    json_t *obj = NULL;
    size_t  idx = 0;
    size_t  prev_num_hosts_setup = app_cfg->rpc.capacity;
    size_t  num_hosts_setup = json_array_size(objs);
    if(num_hosts_setup > prev_num_hosts_setup) {
        h2o_vector_reserve(NULL, &app_cfg->rpc, num_hosts_setup);
        num_hosts_setup = app_cfg->rpc.capacity;
        size_t sz = sizeof(arpc_cfg_t) * (num_hosts_setup - prev_num_hosts_setup);
        void *ptr = &app_cfg->rpc.entries[prev_num_hosts_setup];
        memset(ptr, 0x0, sz);
    }
    app_cfg->rpc.size = 0;
    json_array_foreach(objs, idx, obj) {
        const char *alias = json_string_value(json_object_get(obj, "alias"));
        json_t  *credential = json_object_get(obj, "credential");
        json_t  *attributes = json_object_get(obj, "attributes");
        if(!credential || !attributes || !alias) {
            h2o_error_printf("[parsing] missing credential or attributes message broker configuration (idx=%lu)\n", idx);
            goto error;
        }
        if(!json_is_object(credential) || !json_is_object(attributes)) {
            h2o_error_printf("[parsing] invalid format in credential or attributes of message broker configuration (idx=%lu)\n", idx);
            goto error;
        }
        arpc_cfg_t *cfg = &app_cfg->rpc.entries[app_cfg->rpc.size++];
        app_rpc_cfg_deinit(cfg);
        err = parse_cfg_rpc__broker_credential(credential, cfg);
        if(err) { goto error; }
        err = parse_cfg_rpc__broker_attributes(attributes, cfg);
        if(err) { goto error; }
        cfg->alias = strdup(alias);
    } // end of host-config iteration
    return 0;
error:
    return -1;
} // end of parse_cfg_rpc_common


int parse_cfg_rpc_caller(json_t *objs, app_cfg_t *app_cfg)
{
    if(!objs || !app_cfg || !json_is_array(objs)) {
        goto error;
    }
    size_t  idx = 0, jdx = 0;
    json_t *obj = NULL, *bind_obj = NULL;
    json_array_foreach(objs, idx, obj) {
        json_t  *bindings   = json_object_get(obj, "bindings");
        if(!bindings) {
            h2o_error_printf("[parsing] missing bindings in message broker configuration (idx=%lu)\n", idx);
            goto error;
        }
        if(!json_is_array(bindings) || json_array_size(bindings) == 0) {
            h2o_error_printf("[parsing] invalid format in binding field of message broker configuration (idx=%lu)\n", idx);
            goto error;
        }
    } // end of host-config iteration
    int err = parse_cfg_rpc_common(objs, app_cfg);
    if(err) { goto error; }
    json_array_foreach(objs, idx, obj) {
        arpc_cfg_t *cfg = &app_cfg->rpc.entries[idx];
        json_t  *bindings   = json_object_get(obj, "bindings");
        err = parse_cfg_rpc__broker_bindings(bindings, cfg);
        if(err) { goto error; }
        json_array_foreach(bindings, jdx, bind_obj) {
            json_t  *reply = json_object_get(bind_obj, "reply");
            arpc_cfg_bind_t *bcfg = &cfg->bindings.entries[jdx];
            err = parse_cfg_rpc__reply_producer(reply, &bcfg->reply);
            if(err) { goto error; }
        }
    } // end of host-config iteration
    return 0;
error:
    for(idx = 0; idx < app_cfg->rpc.size; idx++) {
        app_rpc_cfg_deinit(&app_cfg->rpc.entries[idx]);
    }
    free(app_cfg->rpc.entries);
    app_cfg->rpc.entries = NULL;
    app_cfg->rpc.capacity = 0;
    return -1;
} // end of parse_cfg_rpc_caller

int parse_cfg_rpc_callee(json_t *objs, app_cfg_t *app_cfg)
{
    return 0;
} // end of parse_cfg_rpc_callee
