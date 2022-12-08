#include <search.h>
#include <uuid/uuid.h>

#include "utils.h"
#include "base64.h"
#include "acl.h"
#include "views.h"
#include "rpc/core.h"
#include "rpc/reply.h"
#include "models/pool.h"

#define  MAX_NUM_TIMER_EVENTS       300
#define  TIMER_EVENT_INTERVAL_MS    15
#define  NUM_ACL_ITEMS__HARD_LIMIT   1000

typedef struct {
    h2o_req_t     *req;
    h2o_handler_t *hdlr;
    app_middleware_node_t  *node;
    json_t  *spec;
    json_t  *err_info;
    asa_op_base_cfg_t  *asaobj;
    void *rpcreply_ctx;
    json_t  *rpc_returned_usrprofs;
    uint32_t  num_timer_evt;
} api_usr_data_t;


static void  _api_edit_file_acl__deinit_primitives ( h2o_req_t *req, h2o_handler_t *hdlr,
        app_middleware_node_t *node, json_t *spec, json_t *err_info )
{
    h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
    json_t *resp_body =  json_object_size(err_info) > 0 ?  err_info: json_object_get(spec, "_http_resp_body");
    size_t  nb_required = json_dumpb(resp_body, NULL, 0, 0);
    if(req->res.status == 0) {
        req->res.status = 500;
        fprintf(stderr, "[api][edit_file_acl] line:%d \n", __LINE__ );
    }
    if(nb_required > 0) {
        char  body[nb_required + 1];
        size_t  nwrite = json_dumpb(resp_body, &body[0], nb_required, JSON_COMPACT);
        body[nwrite++] = 0x0;
        assert(nwrite <= nb_required);
        h2o_send_inline(req, body, strlen(&body[0]));
    } else {
        h2o_send_inline(req, "{}", 2);
    }
    char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
    if(_res_id_encoded) {
        free(_res_id_encoded);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)NULL);
    }
    json_decref(err_info);
    json_decref(spec);
    app_run_next_middleware(hdlr, req, node);
} // end of  _api_edit_file_acl__deinit_primitives


static void  _api_edit_file_acl__deinit_usrdata (api_usr_data_t *udata)
{
    asa_op_base_cfg_t  *_asaobj = udata->asaobj;
    json_t *rpc_usrprofs = udata->rpc_returned_usrprofs;
    if(_asaobj) {
        udata->asaobj = NULL;
        _asaobj->deinit(_asaobj);
    }
    if(rpc_usrprofs) {
        udata->rpc_returned_usrprofs = NULL;
        json_decref(rpc_usrprofs);
    }
    _api_edit_file_acl__deinit_primitives(udata->req, udata->hdlr, udata->node,
            udata->spec, udata->err_info);
    free(udata);
} // end of   _api_edit_file_acl__deinit_usrdata


static void api__edit_file_acl__db_async_err (db_query_t *target, db_query_result_t *rs)
{ // TODO, de-init api_usr_data_t object
    h2o_req_t     *req  = target->cfg.usr_data.entry[0];
    h2o_handler_t *hdlr = target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = target->cfg.usr_data.entry[2];
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec     = app_fetch_from_hashmap(node->data, "spec");
    json_object_set_new(err_info, "res_id", json_string("error happended during validation"));
    req->res.status = 500;
    _api_edit_file_acl__deinit_primitives (req, hdlr, node, spec, err_info);
}


static void  _api_save_acl__done_cb (aacl_result_t *result, void **usr_args)
{
    api_usr_data_t *usrdata = (api_usr_data_t *)usr_args[0];
    if(result->flag.error) {
        h2o_error_printf("[api][edit_acl] line:%d, error on saving ACL context \n", __LINE__);
    } else {
        usrdata->req->res.status = 200;
    }
    _api_edit_file_acl__deinit_usrdata (usrdata);
}


static void _api_load_saved_acl__done_cb (aacl_result_t *result, void **usr_args)
{
    api_usr_data_t *usrdata = (api_usr_data_t *)usr_args[0];
    int err = 0;
    if(result->flag.error) {
        h2o_error_printf("[api][edit_acl] line:%d, error on fetching saved ACL context \n", __LINE__);
        err = 1;
    } else {
        json_t *req_body = json_object_get(usrdata->spec, "_http_req_body");
        char *res_id_encoded = app_fetch_from_hashmap(usrdata->node->data, "res_id_encoded");
        void *usr_args[1] = {usrdata};
        aacl_cfg_t  aclcfg = {.usr_args={.entries=&usr_args[0], .size=1}, .db_pool=app_db_pool_get_pool("db_server_1"),
            .resource_id=res_id_encoded, .loop=usrdata->req->conn->ctx->loop, .callback=_api_save_acl__done_cb };
        err = app_resource_acl_save(&aclcfg, result, req_body);
    }
    if(err) {
        json_object_set_new(usrdata->err_info, "reason", json_string("internal error"));
        _api_edit_file_acl__deinit_usrdata (usrdata);
    }
} // end of  _api_load_saved_acl__done_cb


static int _api_edit_acl__compare_usr_id_list (const void *i0, const void *i1)
{ return *(const uint32_t *)i0 - *(uint32_t *)i1 ; }

static  __attribute__((optimize("O0")))  int  _api_edit_acl_verify_otherusers_exist (api_usr_data_t *usrdata)
{
    h2o_req_t *req = usrdata->req;
    json_t  *valid_usrprofs = usrdata->rpc_returned_usrprofs, *err_info = usrdata->err_info;
    json_t  *req_body = json_object_get(usrdata->spec, "_http_req_body");
    size_t num_items_req = json_array_size(req_body);
    size_t num_items_rpc = json_array_size(valid_usrprofs);
    if(num_items_req == num_items_rpc) {
        uint32_t  _ids_req[num_items_rpc], _ids_rpc[num_items_rpc];
        for(int idx = 0; idx < num_items_rpc; idx++) {
            _ids_req[idx] = json_integer_value(json_object_get(json_array_get(valid_usrprofs,idx),"id"));
            _ids_rpc[idx] = json_integer_value(json_object_get(json_array_get(req_body,idx),"usr_id"));
        }
        qsort(&_ids_req[0], num_items_req, sizeof(uint32_t), _api_edit_acl__compare_usr_id_list);
        qsort(&_ids_rpc[0], num_items_rpc, sizeof(uint32_t), _api_edit_acl__compare_usr_id_list);
        int ret = memcmp(&_ids_req[0], &_ids_rpc[0], sizeof(uint32_t)*num_items_rpc);
        if(ret) {
            h2o_error_printf("[api][edit_acl] line:%d, unexpected result on user ID comparison: %d \n", __LINE__, ret);
            json_object_set_new(err_info, "reason", json_string("internal error"));
        }
    } else {
        json_object_set_new(err_info, "usr_id", json_string("some of them do not exist"));
        req->res.status = 400;
    }
    return json_object_size(err_info) > 0;
} // end of _api_edit_acl_verify_otherusers_exist


#define  VERIFY_USERS_OR_RESTART_RPC(_usrdata, _err, __continue) \
{ \
    if(_usrdata->rpc_returned_usrprofs) { \
        _err = _api_edit_acl_verify_otherusers_exist(_usrdata); \
        if(!_err) { \
            void *usr_args[1] = {_usrdata}; \
            char *res_id_encoded = app_fetch_from_hashmap(_usrdata->node->data, "res_id_encoded"); \
            aacl_cfg_t  aclcfg = {.usr_args={.entries=&usr_args[0], .size=1}, .resource_id=res_id_encoded, \
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=_usrdata->req->conn->ctx->loop, \
                .callback=_api_load_saved_acl__done_cb }; \
            _err = app_resource_acl_load(&aclcfg); \
        } \
        __continue = 0; \
    } else if (_usrdata->num_timer_evt++ > MAX_NUM_TIMER_EVENTS) { \
        h2o_error_printf("[api][edit_acl] line:%d, timeout, not receive RPC reply \n", __LINE__); \
        _usrdata->req ->res.status = 503; \
        __continue = 0; \
        _err = 1; \
    } else { \
        void *_out_ctx = apprpc_recv_reply_restart (_usrdata->rpcreply_ctx); \
        if(!_out_ctx) { \
            __continue = 0; \
            _err = 1; \
        } \
    } \
}

static  uint8_t _api_verify_otherusers_exist__update_cb (arpc_reply_cfg_t *cfg, json_t *info, ARPC_STATUS_CODE result)
{
    json_t  *reply_usrprofs  = json_object_get(info, "rpc.media.get_usr_profile.corr_id.%s");
    json_t  *reply_transcode = json_object_get(info, "rpc.media.transcode.corr_id.%s");
    uint8_t _transcode_job_updated = (reply_transcode != NULL) && (json_array_size(reply_transcode) > 0);
    uint8_t _continue = 1;
    api_usr_data_t *usrdata = cfg->usr_data;
    json_t *valid_usrprofs = NULL;
    int err = app_rpc__pycelery_extract_replies(reply_usrprofs, &valid_usrprofs) != APPRPC_RESP_OK;
    if(err) {
        _continue = 0;
    } else if (valid_usrprofs) {
        if(usrdata->rpc_returned_usrprofs)
            json_decref(usrdata->rpc_returned_usrprofs);
        usrdata->rpc_returned_usrprofs = valid_usrprofs;
    }
    if(_transcode_job_updated) {
        assert(0); // TODO, write received job progress first
    } else {
        VERIFY_USERS_OR_RESTART_RPC(usrdata, err, _continue)
    }
    if(!_continue)
        usrdata->rpcreply_ctx = NULL;
    if(err)
        _api_edit_file_acl__deinit_usrdata (usrdata);
    return _continue;
} // end of  _api_verify_otherusers_exist__update_cb
#undef  VERIFY_USERS_OR_RESTART_RPC


static  void _api_verify_otherusers_exist__err_cb (arpc_reply_cfg_t *cfg, ARPC_STATUS_CODE result)
{
    api_usr_data_t *usrdata = cfg->usr_data;
    usrdata->req ->res.status = cfg->flags.replyq_nonexist ? 400: 503;
    usrdata->rpcreply_ctx = NULL;
    _api_edit_file_acl__deinit_usrdata (usrdata);
} // end of _api_verify_otherusers_exist__err_cb


static ARPC_STATUS_CODE  _api_rpc__start_verify_usr_ids (void *mq_conn, json_t *spec, uint32_t curr_usr_id)
{ // RPC to user-management app, Python Celery is used for RPC consumer
    json_object_set_new(spec, "usr_id", json_integer(curr_usr_id));
#define  PY_CELERY_MSGBODY   "[[], {\"ids\":[], \"fields\":[\"id\"]}, " \
    "{\"callbacks\": null, \"errbacks\": null, \"chain\": null, \"chord\": null}]"
    // Note the RPC in user-management is built in python celery
    json_t *msg_body = json_loadb(PY_CELERY_MSGBODY, sizeof(PY_CELERY_MSGBODY) - 1, 0, NULL);
    json_t *msg_ids  = json_object_get(json_array_get(msg_body, 1), "ids");
#undef   PY_CELERY_MSGBODY  
    json_t *item = NULL;
    int idx = 0;
    json_t *req_body = json_object_get(spec, "_http_req_body");
    json_array_foreach(req_body, idx, item) {
        json_array_append(msg_ids, json_object_get(item, "usr_id"));
    }
    size_t  msg_nb_required = json_dumpb(msg_body, NULL, 0, 0);
    char    msg_body_raw[msg_nb_required + 1];
    size_t  nwrite = json_dumpb(msg_body, &msg_body_raw[0], msg_nb_required, JSON_COMPACT);
    msg_body_raw[nwrite++] = 0x0;
    assert(nwrite < msg_nb_required);
    msg_nb_required = nwrite;
    // `id`, `task` fields are essential in header of version 1 protocol
    // https://docs.celeryq.dev/en/master/internals/protocol.html
#define  UUID_STR_SZ   36
#define  PY_CELERY_ID_PATTERN       "celery.media.get_usr_profile.%d.%s"
#define  PY_CELERY_TSK_HDLR_HIER    "user_management.async_tasks.get_profile"
    uuid_t  cel_uuid;
    size_t celery_id_str_sz = sizeof(PY_CELERY_ID_PATTERN) + USR_ID_STR_SIZE + UUID_STR_SZ;
    char celery_id_str[celery_id_str_sz], cel_uuid_str[UUID_STR_SZ+1] = {0};
    uuid_generate_random(cel_uuid);
    uuid_unparse(cel_uuid, &cel_uuid_str[0]);
    assert(strlen(&cel_uuid_str[0]) == UUID_STR_SZ);
    nwrite = snprintf(&celery_id_str[0], celery_id_str_sz, PY_CELERY_ID_PATTERN, curr_usr_id, &cel_uuid_str[0]);
    assert(nwrite < celery_id_str_sz);
    celery_id_str_sz = nwrite;
    arpc_kv_t  _extra_headers[2] = {
        {.value={.len=celery_id_str_sz, .bytes=&celery_id_str[0]}, .key={.len=2, .bytes="id"}},
        {.value={.len=sizeof(PY_CELERY_TSK_HDLR_HIER)-1, .bytes=PY_CELERY_TSK_HDLR_HIER}, .key={.len=4, .bytes="task"}}
    };
#undef   PY_CELERY_TSK_HDLR_HIER   
#undef   PY_CELERY_ID_PATTERN
#undef   UUID_STR_SZ
    char job_id_raw[MAX_BYTES_JOB_ID] = {0};
    arpc_exe_arg_t  rpc_arg = {
        .conn=mq_conn, .alias="app_mqbroker_1",  .job_id={.bytes=&job_id_raw[0], .len=MAX_BYTES_JOB_ID},
        .msg_body={.len=msg_nb_required, .bytes=&msg_body_raw[0]}, .routing_key="rpc.user_management.get_profile",
        .usr_data=(void *)spec, .headers={.size=2, .entries=&_extra_headers[0]}
    };
    json_decref(msg_body);
    return app_rpc_start(&rpc_arg);
} // end of  _api_rpc__start_verify_usr_ids


static void  api_edit_acl__verify_otherusers_exist (h2o_handler_t *hdlr, h2o_req_t *req,
        app_middleware_node_t *node, json_t *spec, json_t *err_info)
{ // check whether currnet user owns the resource file
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
    ARPC_STATUS_CODE  result = _api_rpc__start_verify_usr_ids (req->conn->ctx->storage.entries[1].data , spec, curr_usr_id);
    if(result != APPRPC_RESP_ACCEPTED) {
        h2o_error_printf("[api][edit_acl] line:%d, failed to publish RPC message:%d \n", __LINE__, result );
        json_object_set_new(err_info, "unknown", json_string("internal error"));
        req->res.status = 503;
    } else { // check RPC reply queue
        api_usr_data_t *usrdata = calloc(1, sizeof(api_usr_data_t));
        *usrdata = (api_usr_data_t) {.req=req, .hdlr=hdlr, .node=node, .spec=spec, .err_info=err_info};
        arpc_reply_cfg_t   rpc_cfg = { .usr_id = curr_usr_id, .loop=req->conn->ctx->loop,
              .conn=req->conn->ctx->storage.entries[1].data,  .usr_data=usrdata,  .max_num_msgs_fetched=3,
              .get_reply_fn=app_rpc_fetch_replies, .timeout_ms=TIMER_EVENT_INTERVAL_MS,
              .on_error=_api_verify_otherusers_exist__err_cb,  .on_update=_api_verify_otherusers_exist__update_cb,
        };
        void *rpc_reply_ctx = apprpc_recv_reply_start (&rpc_cfg);
        if(rpc_reply_ctx) {
            usrdata->rpcreply_ctx = rpc_reply_ctx;
        } else {
            free(usrdata);
            json_object_set_new(err_info, "reason", json_string("essential service not available"));
        }
    }
} // end of  api_edit_acl__verify_otherusers_exist


static void _api_abac_pdp__try_match_rule (aacl_result_t *result, void **usr_args)
{ // this function is the second part of Policy Decision Point (PDP) of a ABAC implementation
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *spec     = usr_args[3];
    json_t *err_info = usr_args[4];
    if(result->flag.error) {
        req->res.status = 503;
    } else if(result->data.size != 1 || !result->data.entries) {
        req->res.status = 403;
        json_object_set_new(err_info, "usr_id", json_string("missing access-control setup"));
    } else {
        aacl_data_t *d = &result->data.entries[0];
        if(!d->capability.edit_acl) {
            req->res.status = 403;
            json_object_set_new(err_info, "usr_id", json_string("operation denied"));
        }
    }
    if(req->res.status == 0) {
        app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
        app_save_ptr_to_hashmap(node->data, "spec", (void *)spec);
        app_run_next_middleware(hdlr, req, node);
    } else {
        _api_edit_file_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    }
} // end of  _api_abac_pdp__try_match_rule


static void _api_abac_pdp__verify_resource_owner (aacl_result_t *result, void **usr_args)
{ // this function is the first part of Policy Decision Point (PDP) of a ABAC implementation
    h2o_req_t     *req  = usr_args[0];
    h2o_handler_t *hdlr = usr_args[1];
    app_middleware_node_t *node = usr_args[2];
    json_t *spec     = usr_args[3];
    json_t *err_info = usr_args[4];
    req->res.status = api_http_resp_status__verify_resource_id (result, err_info);
    if(json_object_size(err_info) == 0) {
        json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
        uint32_t curr_usr_id = (uint32_t) json_integer_value(json_object_get(jwt_claims, "profile"));
        if(curr_usr_id == result->owner_usr_id) {
            app_save_ptr_to_hashmap(node->data, "err_info", (void *)err_info);
            app_save_ptr_to_hashmap(node->data, "spec", (void *)spec);
            app_run_next_middleware(hdlr, req, node);
        } else {
            char *_res_id_encoded = app_fetch_from_hashmap(node->data, "res_id_encoded");
            void *usr_args[5] = {req, hdlr, node, spec, err_info};
            aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=5}, .resource_id=_res_id_encoded,
                    .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                    .usr_id=curr_usr_id, .callback=_api_abac_pdp__try_match_rule };
            int err = app_resource_acl_load(&cfg); // seen as Policy Information Point in ABAC
            if(err)
                _api_edit_file_acl__deinit_primitives (req, hdlr, node, spec, err_info);
        }
    } else {
        _api_edit_file_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    }
} // end of  _api_abac_pdp__verify_resource_owner


static int api_abac_pep__edit_acl (h2o_handler_t *hdlr, h2o_req_t *req, app_middleware_node_t *node)
{ // this middleware can be seen as Policy Enforcement Point (PEP) of an ABAC implementation
    json_t *err_info = json_object(), *spec = json_object();
    app_url_decode_query_param(&req->path.base[req->query_at + 1], spec);
    const char *resource_id = app_resource_id__url_decode(spec, err_info);
    if(!resource_id || (json_object_size(err_info) > 0)) {
        req->res.status = 400;
    } else {
        size_t out_len = 0;
        unsigned char *_res_id_encoded = base64_encode((const unsigned char *)resource_id,
                 strlen(resource_id), &out_len);
        app_save_ptr_to_hashmap(node->data, "res_id_encoded", (void *)_res_id_encoded);
        void *usr_args[5] = {req, hdlr, node, spec, err_info};
        aacl_cfg_t  cfg = {.usr_args={.entries=&usr_args[0], .size=5}, .resource_id=(char *)_res_id_encoded,
                .db_pool=app_db_pool_get_pool("db_server_1"), .loop=req->conn->ctx->loop,
                .callback=_api_abac_pdp__verify_resource_owner };
        int err = app_acl_verify_resource_id (&cfg);
        if(err)
            json_object_set_new(err_info, "reason", json_string("internal error"));
    }
    if(json_object_size(err_info) > 0)
        _api_edit_file_acl__deinit_primitives (req, hdlr, node, spec, err_info);
    return 0;
} // end of  api_abac_pep__edit_acl


RESTAPI_ENDPOINT_HANDLER(edit_file_acl, PUT, self, req)
{
    int idx = 0;
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec  = app_fetch_from_hashmap(node->data, "spec");
    json_t *item = NULL,  *req_body = json_loadb(req->entity.base, req->entity.len, 0, NULL);
    if(req_body) {
        size_t  num_acl_items = json_array_size(req_body);
        if(!json_is_array(req_body)) {
            json_object_set_new(err_info, "body", json_string("not array"));
        } else if (num_acl_items == 0) {
            json_object_set_new(err_info, "body", json_string("empty"));
        } else if (num_acl_items > NUM_ACL_ITEMS__HARD_LIMIT) {
            json_object_set_new(err_info, "body", json_string("limit exceeding"));
        } else {
            json_object_set_new(spec, "_http_req_body", req_body);
        }
    } else {
        json_object_set_new(err_info, "body", json_string("decode error"));
    }
    if(json_object_size(err_info) > 0) {
        req->res.status = 400;
        goto done;
    }
    json_array_foreach(req_body, idx, item) {
        int usr_id   = (int) json_integer_value(json_object_get(item, "usr_id"));
        json_t *acl = json_object_get(item, "access_control");
        if(usr_id == 0)
            json_object_set_new(err_info, "usr_id", json_string("zero"));
        if(acl && json_object_size(acl) > 0) {
            uint8_t  bool_transcode = json_is_boolean(json_object_get(acl, "transcode"));
            uint8_t  bool_renew     = json_is_boolean(json_object_get(acl, "renew"));
            uint8_t  bool_edit_acl  = json_is_boolean(json_object_get(acl, "edit_acl"));
            if(!bool_transcode)
                json_object_set_new(err_info, "transcode", json_string("invalid value"));
            if(!bool_renew)
                json_object_set_new(err_info, "renew", json_string("invalid value"));
            if(!bool_edit_acl)
                json_object_set_new(err_info, "edit_acl", json_string("invalid value"));
        } else {
            json_object_set_new(err_info, "access_control", json_string("non-existent"));
        }
        if(json_object_size(err_info) > 0) {
            req->res.status = 400;
            goto done;
        }
    } // end of request body iteration
    api_edit_acl__verify_otherusers_exist (self, req, node, spec, err_info);
done:
    if(json_object_size(err_info) > 0)
        _api_edit_file_acl__deinit_primitives (req, self, node, spec, err_info);
    return 0;
} // end of edit_file_acl

