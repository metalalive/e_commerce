#include "datatypes.h"
#include "acl.h"
#include "models/query.h"

#define  NUM_USR_ARGS  1
#define  SQL_TABLE_NAME   "file_access_control"
#define  SQL_BASE64_ENCODED_RESOURCE_ID   "FROM_BASE64('%s')"

#define  COPY_CFG_TO_CTX(_ctx, _cfg) { \
    _ctx->resource_id = _cfg->resource_id; \
    _ctx->usr_id   = _cfg->usr_id; \
    _ctx->usrdata  = _cfg->usrdata; \
    _ctx->callback = _cfg->callback; \
}

typedef struct {
    APP_ACL_CFG__COMMON_FIELDS
    aacl_result_t  result;
} _aacl_ctx_t;


static void  appacl_db_dummy_cb (db_query_t *q, db_query_result_t *rs)
{}

static void  appacl_db_common_trigger_callback (_aacl_ctx_t  *ctx)
{
    aacl_result_t  *_result = &ctx->result;
    ctx->callback(_result, ctx->usrdata);
    if(_result->data.entries) {
        free(_result->data.entries);
        _result->data.entries = NULL;
    }
    free(ctx);
}

static void  appacl_loaddb_resultset_dealloc (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
    appacl_db_common_trigger_callback (ctx);
}

static void  appacl_db_common_error_cb (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
    ctx->result.flag.error = 1;
    appacl_db_common_trigger_callback (ctx);
}

static void  appacl_db_write_done_cb (db_query_t *q, db_query_result_t *rs)
{
    assert(rs->_final);
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
    ctx->result.flag.write_ok = 1;
    appacl_db_common_trigger_callback (ctx);
}

static void  appacl_loaddb_row_fetched (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
    aacl_result_t  *_result = &ctx->result;
    size_t  curr_capacity = _result->data.capacity;
    size_t  curr_num_rows = _result->data.size;
    if(curr_num_rows == curr_capacity) {
        curr_capacity += 8;
        h2o_vector_reserve(NULL, &_result->data, curr_capacity);
    }
    db_query_row_info_t *row = (db_query_row_info_t *) &rs->data[0];
    assert(row->num_cols == 4);
    uint32_t _usr_id = (uint32_t) strtoul(row->values[0], NULL, 10);
    uint8_t  _transcode_enable = (uint8_t) strtoul(row->values[1], NULL, 2);
    uint8_t  _renew_enable    = (uint8_t) strtoul(row->values[2], NULL, 2);
    uint8_t  _edit_acl_enable = (uint8_t) strtoul(row->values[3], NULL, 2);
    _result->data.entries[curr_num_rows++] = (aacl_data_t) {.usr_id=_usr_id, .capability={
        .renew=_renew_enable, .transcode=_transcode_enable, .edit_acl=_edit_acl_enable }};
    _result->data.size = curr_num_rows;
} // end of  appacl_loaddb_row_fetched


int  app_resource_acl_load(aacl_cfg_t *cfg)
{
    _aacl_ctx_t  *ctx = calloc(1, sizeof(_aacl_ctx_t));
    COPY_CFG_TO_CTX(ctx, cfg)
#define  SQL_PATTERN_1  "EXECUTE IMMEDIATE 'SELECT `usr_id`,`transcode_flg`,`renew_flg`,`edit_acl_flg` FROM" \
    " `"SQL_TABLE_NAME"`  WHERE `file_id`=?' USING "SQL_BASE64_ENCODED_RESOURCE_ID";"
#define  SQL_PATTERN_2  "EXECUTE IMMEDIATE 'SELECT `usr_id`,`transcode_flg`,`renew_flg`,`edit_acl_flg` FROM" \
    " `"SQL_TABLE_NAME"`  WHERE `file_id`=? AND `usr_id`=?' USING "SQL_BASE64_ENCODED_RESOURCE_ID", %u;"
    const char *sql_patt = NULL;
    size_t res_id_sz = strlen(ctx->resource_id), rawsql_sz = 1 + res_id_sz;
    if(ctx->usr_id == 0) {
        sql_patt = SQL_PATTERN_1;
        rawsql_sz += sizeof(SQL_PATTERN_1);
    } else {
        sql_patt = SQL_PATTERN_2;
        rawsql_sz += sizeof(SQL_PATTERN_2) + USR_ID_STR_SIZE;
    }
    char rawsql[rawsql_sz];
    if(ctx->usr_id == 0) {
       size_t nwrite = snprintf(&rawsql[0], rawsql_sz, sql_patt, ctx->resource_id);
       assert(nwrite < rawsql_sz);
    } else {
       size_t nwrite = snprintf(&rawsql[0], rawsql_sz, sql_patt, ctx->resource_id, ctx->usr_id);
       assert(nwrite < rawsql_sz);
    }
#undef   SQL_PATTERN_1
#undef   SQL_PATTERN_2
    void *db_async_usr_args[NUM_USR_ARGS] = {ctx};
    db_query_cfg_t  db_cfg = { .loop=cfg->loop, .pool=cfg->db_pool,
        .usr_data={.entry=(void **)&db_async_usr_args[0], .len=NUM_USR_ARGS},
        .callbacks = {.result_rdy=appacl_db_dummy_cb, .row_fetched=appacl_loaddb_row_fetched,
            .result_free=appacl_loaddb_resultset_dealloc,  .error=appacl_db_common_error_cb },
        .statements = {.entry=&rawsql[0], .num_rs=1}
    };
    DBA_RES_CODE  db_result = app_db_query_start(&db_cfg);
    int err = db_result != DBA_RESULT_OK;
    if(err)
        free(ctx);
    return err;
} // end of  app_resource_acl_load


void  app_acl__build_update_lists (aacl_result_t *existing_data, json_t *new_data,
         aacl_data_t **data_update, size_t *num_update,
         aacl_data_t **data_delete, size_t *num_deletion,
         aacl_data_t  *data_insert, size_t *num_insertion )
{
    size_t  _num_insertion = 0, _num_deletion = 0, _num_update = 0;
    size_t  num_new_reqs = json_array_size(new_data);
    json_t *new_item = NULL;
    int idx = 0, jdx = 0;
    for(idx = 0; idx < existing_data->data.size; idx++) {
        aacl_data_t *origin = &existing_data->data.entries[idx];
        json_array_foreach(new_data, jdx, new_item) {
             size_t curr_usr_id = json_integer_value(json_object_get(new_item, "usr_id"));
             if(curr_usr_id == origin->usr_id) {
                 json_t *new_detail = json_object_get(new_item, "access_control");
                 origin->capability.renew = (uint8_t) json_boolean_value(json_object_get(new_detail, "renew"));
                 origin->capability.edit_acl = (uint8_t) json_boolean_value(json_object_get(new_detail, "edit_acl"));
                 origin->capability.transcode = (uint8_t) json_boolean_value(json_object_get(new_detail, "transcode"));
                 json_object_set_new(new_item, "_is_updating", json_true());
                 data_update[_num_update++] = origin;
                 break;
             }
        } // end of new data iteration 
        if(jdx == num_new_reqs)
            data_delete[_num_deletion++] = origin;
    } // end of existing data iteration
    json_array_foreach(new_data, jdx, new_item) {
        uint8_t is_updating = json_boolean_value(json_object_get(new_item, "_is_updating"));
        if(is_updating)
            continue;
        json_t *new_detail = json_object_get(new_item, "access_control");
        data_insert[_num_insertion++] = (aacl_data_t) {
            .usr_id = (uint32_t) json_integer_value(json_object_get(new_item, "usr_id")),
            .capability = {
                .renew = (uint8_t) json_boolean_value(json_object_get(new_detail, "renew")),
                .edit_acl = (uint8_t) json_boolean_value(json_object_get(new_detail, "edit_acl")),
                .transcode = (uint8_t) json_boolean_value(json_object_get(new_detail, "transcode"))
            }
        };
    } // end of new data iteration
    *num_update = _num_update;
    *num_deletion = _num_deletion;
    *num_insertion = _num_insertion;
} // end of app_acl__build_update_lists


#define  PREP_STMT_LABEL_INSERT   "app_media_file_acl_insert"
#define  PREP_STMT_LABEL_UPDATE   "app_media_file_acl_update"
#define  PREP_STMT_LABEL_DELETE   "app_media_file_acl_delete"
#define  SQL_EXEC_INSERT  "EXECUTE `"PREP_STMT_LABEL_INSERT"` USING %1u,%1u,%1u,"SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  SQL_EXEC_UPDATE  "EXECUTE `"PREP_STMT_LABEL_UPDATE"` USING %1u,%1u,%1u,"SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  SQL_EXEC_DELETE  "EXECUTE `"PREP_STMT_LABEL_DELETE"` USING "SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  FINAL_SQL_PATTERN  \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `"PREP_STMT_LABEL_INSERT"` FROM 'INSERT INTO `"SQL_TABLE_NAME"`(`transcode_flg`,`renew_flg`,`edit_acl_flg`,`file_id`,`usr_id`) VALUES (?,?,?,?,?)';" \
    "  PREPARE `"PREP_STMT_LABEL_UPDATE"` FROM 'UPDATE `"SQL_TABLE_NAME"` SET `transcode_flg`=?,`renew_flg`=?,`edit_acl_flg`=? WHERE `file_id`=? AND `usr_id`=?';" \
    "  PREPARE `"PREP_STMT_LABEL_DELETE"` FROM 'DELETE FROM `"SQL_TABLE_NAME"` WHERE `file_id`=? AND `usr_id`=?';" \
    "  START TRANSACTION;" \
    "    %s %s %s" \
    "  COMMIT;" \
    "END;"
int  app_resource_acl_save(aacl_cfg_t *cfg, aacl_result_t *existing_data, json_t *new_data)
{
    size_t  max_num_insertion = json_array_size(new_data), max_num_deletion = existing_data->data.size,
            max_num_update = max_num_deletion, num_insertion = 0, num_deletion = 0, num_update = 0;
    aacl_data_t  data_insert[max_num_insertion], *data_update[max_num_update],  *data_delete[max_num_deletion];
    app_acl__build_update_lists (existing_data, new_data, &data_update[0], &num_update,
            &data_delete[0], &num_deletion, &data_insert[0], &num_insertion);
    assert(num_insertion <= max_num_insertion);
    assert(num_deletion <= max_num_deletion);
    assert(num_update <= max_num_update);
    size_t  res_id_sz = strlen(cfg->resource_id);
    size_t  single_insert_sql_sz = sizeof(SQL_EXEC_INSERT) + 3 + res_id_sz + USR_ID_STR_SIZE;
    size_t  single_update_sql_sz = sizeof(SQL_EXEC_UPDATE) + 3 + res_id_sz + USR_ID_STR_SIZE;
    size_t  single_delete_sql_sz = sizeof(SQL_EXEC_DELETE) + res_id_sz + USR_ID_STR_SIZE;
    size_t  insert_sql_tot_sz = single_insert_sql_sz * num_insertion;
    size_t  update_sql_tot_sz = single_update_sql_sz * num_update;
    size_t  delete_sql_tot_sz = single_delete_sql_sz * num_deletion;
    size_t  final_sql_sz = sizeof(FINAL_SQL_PATTERN) + delete_sql_tot_sz + update_sql_tot_sz + insert_sql_tot_sz;
    char  final_sql[final_sql_sz];
    { // start sql queries construction, TODO: verify whether the SQL statement is corrupted
        int idx = 0;
        char insert_sqls[insert_sql_tot_sz + 1], update_sqls[update_sql_tot_sz + 1],
               delete_sqls[delete_sql_tot_sz + 1], *ptr = NULL;
        if (num_insertion > 0) {
            size_t  avail_buf_sz = insert_sql_tot_sz + 1, nwrite = 0;
            ptr = &insert_sqls[0];
            for(idx = 0; idx < num_insertion; idx++) {
                aacl_data_t *data = &data_insert[idx];
                nwrite = snprintf(ptr, avail_buf_sz, SQL_EXEC_INSERT, data->capability.transcode, data->capability.renew,
                       data->capability.edit_acl, cfg->resource_id, data->usr_id);
                assert(nwrite < avail_buf_sz);
                ptr += nwrite; avail_buf_sz -= nwrite;
            }
        } else {
            insert_sqls[0] = 0;
        }
        if (num_update > 0) {
            size_t  avail_buf_sz = update_sql_tot_sz + 1, nwrite = 0;
            ptr = &update_sqls[0];
            for(idx = 0; idx < num_update; idx++) {
                aacl_data_t *data = data_update[idx];
                nwrite = snprintf(ptr, avail_buf_sz, SQL_EXEC_UPDATE, data->capability.transcode, data->capability.renew,
                       data->capability.edit_acl, cfg->resource_id, data->usr_id);
                assert(nwrite < avail_buf_sz);
                ptr += nwrite; avail_buf_sz -= nwrite;
            }
        } else {
            update_sqls[0] = 0;
        }
        if (num_deletion > 0) {
            size_t  avail_buf_sz = delete_sql_tot_sz + 1, nwrite = 0;
            ptr = &delete_sqls[0];
            for(idx = 0; idx < num_deletion; idx++) {
                aacl_data_t *data = data_delete[idx];
                nwrite = snprintf(ptr, avail_buf_sz, SQL_EXEC_DELETE, cfg->resource_id, data->usr_id);
                assert(nwrite < avail_buf_sz);
                ptr += nwrite; avail_buf_sz -= nwrite;
            }
        } else {
            delete_sqls[0] = 0;
        }
        size_t tot_nwrite = snprintf(&final_sql[0], final_sql_sz, FINAL_SQL_PATTERN,
                &delete_sqls[0], &update_sqls[0], &insert_sqls[0]);
        assert(tot_nwrite < final_sql_sz);
    } // end of list of sql queries construction
    _aacl_ctx_t  *ctx = calloc(1, sizeof(_aacl_ctx_t));
    COPY_CFG_TO_CTX(ctx, cfg)
    void *db_async_usr_args[NUM_USR_ARGS] = {ctx};
    db_query_cfg_t  db_cfg = { .loop=cfg->loop, .pool=cfg->db_pool,
        .usr_data={.entry=(void **)&db_async_usr_args[0], .len=NUM_USR_ARGS},
        .callbacks = {.result_rdy=appacl_db_write_done_cb, .row_fetched=appacl_db_dummy_cb,
            .result_free=appacl_db_dummy_cb,  .error=appacl_db_common_error_cb },
        .statements = {.entry=&final_sql[0], .num_rs=1}
    };
    DBA_RES_CODE  db_result = app_db_query_start(&db_cfg);
    int err = db_result != DBA_RESULT_OK;
    if(err)
        free(ctx);
    return err;
} // end of app_resource_acl_save
#undef   FINAL_SQL_PATTERN
#undef   SQL_EXEC_INSERT
#undef   SQL_EXEC_UPDATE
#undef   SQL_EXEC_DELETE
#undef   PREP_STMT_LABEL_INSERT
#undef   PREP_STMT_LABEL_UPDATE
#undef   PREP_STMT_LABEL_DELETE
