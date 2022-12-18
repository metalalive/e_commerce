#include "datatypes.h"
#include "acl.h"
#include "models/query.h"

#define  FILELVL_ACL_TABLE  "filelvl_access_ctrl"
#define  USRLVL_ACL_TABLE   "usrlvl_access_ctrl"
#define  SQL_BASE64_ENCODED_RESOURCE_ID   "FROM_BASE64('%s')"

#define  COPY_CFG_TO_CTX(_ctx, _cfg) { \
    _ctx->resource_id = _cfg->resource_id; \
    _ctx->usr_id   = _cfg->usr_id; \
    _ctx->callback = _cfg->callback; \
    _ctx->_num_usr_args  = _cfg->usr_args.size; \
    _ctx->fetch_acl = _cfg->fetch_acl; \
}

typedef struct {
    APP_ACL_CFG__COMMON_FIELDS
    aacl_result_t  result;
    uint16_t   _num_internal_args;
    uint16_t   _num_usr_args;
} _aacl_ctx_t;


static void  appacl_db_dummy_cb (db_query_t *q, db_query_result_t *rs)
{}

static void  appacl_db_common_trigger_callback (_aacl_ctx_t  *ctx, void  **usr_args)
{
    usr_args = ctx->_num_usr_args == 0 ? NULL: usr_args;
    aacl_result_t  *_result = &ctx->result;
    ctx->callback(_result, usr_args);
    if(_result->data.entries) {
        free(_result->data.entries);
        _result->data.entries = NULL;
    }
    free(ctx);
}

static void  appacl_loaddb_resultset_dealloc (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx =  q->cfg.usr_data.entry[0];
    void   **usr_args = &q->cfg.usr_data.entry[ctx->_num_internal_args];
    appacl_db_common_trigger_callback (ctx, usr_args);
}

static void  appacl_db_common_error_cb (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx =  q->cfg.usr_data.entry[0];
    void   **usr_args = &q->cfg.usr_data.entry[ctx->_num_internal_args];
    ctx->result.flag.error = 1;
    fprintf(stderr, "[acl] line:%d, error, resource ID:%s, user ID:%u \n", __LINE__,
            ctx->resource_id, ctx->usr_id);
    appacl_db_common_trigger_callback (ctx, usr_args);
}

static void  appacl_db_write_done_cb (db_query_t *q, db_query_result_t *rs)
{
    assert(rs->_final);
    _aacl_ctx_t  *ctx =  q->cfg.usr_data.entry[0];
    void   **usr_args = &q->cfg.usr_data.entry[ctx->_num_internal_args];
    ctx->result.flag.write_ok = 1;
    appacl_db_common_trigger_callback (ctx, usr_args);
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
    assert(row->num_cols == 3);
    uint32_t _usr_id = (uint32_t) strtoul(row->values[0], NULL, 10);
    uint8_t  _transcode_enable = (uint8_t) strtoul(row->values[1], NULL, 2);
    uint8_t  _edit_acl_enable = (uint8_t) strtoul(row->values[2], NULL, 2);
    _result->data.entries[curr_num_rows++] = (aacl_data_t) {.usr_id=_usr_id, .capability={
         .transcode=_transcode_enable, .edit_acl=_edit_acl_enable }};
    _result->data.size = curr_num_rows;
} // end of  appacl_loaddb_row_fetched


int  app_resource_acl_load(aacl_cfg_t *cfg)
{
    _aacl_ctx_t  *ctx = calloc(1, sizeof(_aacl_ctx_t));
    COPY_CFG_TO_CTX(ctx, cfg)
#define  SQL_PATTERN_1  "EXECUTE IMMEDIATE 'SELECT `usr_id`,`transcode_flg`,`edit_acl_flg` FROM" \
    " `"USRLVL_ACL_TABLE"`  WHERE `file_id`=?' USING "SQL_BASE64_ENCODED_RESOURCE_ID";"
#define  SQL_PATTERN_2  "EXECUTE IMMEDIATE 'SELECT `usr_id`,`transcode_flg`,`edit_acl_flg` FROM" \
    " `"USRLVL_ACL_TABLE"`  WHERE `file_id`=? AND `usr_id`=?' USING "SQL_BASE64_ENCODED_RESOURCE_ID", %u;"
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
    ctx->_num_internal_args = 1;
    uint16_t tot_num_usr_args = ctx->_num_internal_args + ctx->_num_usr_args;
    void *db_async_usr_args[tot_num_usr_args];
    db_async_usr_args[0] = ctx;
    memcpy(&db_async_usr_args[ctx->_num_internal_args], cfg->usr_args.entries, sizeof(void *) * ctx->_num_usr_args);
    db_query_cfg_t  db_cfg = { .loop=cfg->loop, .pool=cfg->db_pool,
        .usr_data={.entry=(void **)&db_async_usr_args[0], .len=tot_num_usr_args},
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


static void _app_acl_resource_id__rs_free (db_query_t *q, db_query_result_t *rs)
{
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t num_rows_read = (size_t) q->cfg.usr_data.entry[1];
    uint32_t resource_owner_id = (uint32_t) q->cfg.usr_data.entry[2];
    uint32_t last_upld_req     = (uint32_t) q->cfg.usr_data.entry[3];
    aacl_result_t  *_result = &ctx->result;
    if(num_rows_read == 1) {
        _result->owner_usr_id = resource_owner_id;
        _result->upld_req = last_upld_req;
        _result->flag.res_id_exists = 1;
        if(ctx->fetch_acl)
            _result->flag.acl_visible = ((uint8_t) q->cfg.usr_data.entry[4]) & 0x1;
    } else if(num_rows_read > 1) {
        _result->flag.res_id_dup = 1;
    }
#pragma GCC diagnostic pop
    void   **usr_args = &q->cfg.usr_data.entry[ ctx->_num_internal_args ];
    appacl_db_common_trigger_callback (ctx, usr_args);
} // end of  _app_acl_resource_id__rs_free


static void  _app_acl_resource_id__row_fetch (db_query_t *q, db_query_result_t *rs)
{
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
    _aacl_ctx_t  *ctx = q->cfg.usr_data.entry[0];
    db_query_row_info_t *row = (db_query_row_info_t *)&rs->data[0];
    if(row->values[0]) // resource owner ID
        q->cfg.usr_data.entry[2] = (void *) strtoul(row->values[0], NULL, 10);
    if(row->values[1]) // last_upld_req
        q->cfg.usr_data.entry[3] = (void *) strtoul(row->values[1], NULL, 16);
    if(row->values[2]) // resource type
        memcpy(&ctx->result.type[0], row->values[2], sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ctx->fetch_acl) { // visible flag
        assert(row->num_cols == 4);
        if(row->values[3]) {
            q->cfg.usr_data.entry[4] = (void *) strtoul(row->values[3], NULL, 2);
            ctx->result.flag.acl_exists = 1;
        }
    }
    size_t num_rows_read = (size_t) q->cfg.usr_data.entry[1];
    q->cfg.usr_data.entry[1] = (void *) (num_rows_read + 1);
#pragma GCC diagnostic pop
} // end of  _app_acl_resource_id__row_fetch


int  app_acl_verify_resource_id (aacl_cfg_t *cfg)
{
    int err = 1;
    if(!cfg || !cfg->resource_id || !cfg->callback)
        return err;
#define SQL1_PATTERN "EXECUTE IMMEDIATE 'SELECT `usr_id`,HEX(`last_upld_req`),`type` FROM `uploaded_file` WHERE `id`=?' USING FROM_BASE64('%s');"
#define SQL2_PATTERN "EXECUTE IMMEDIATE 'SELECT `uf`.`usr_id`, HEX(`uf`.`last_upld_req`),`uf`.`type`,`fac`.`visible_flg`" \
    " FROM `uploaded_file` AS `uf` LEFT JOIN `"FILELVL_ACL_TABLE"` AS `fac` ON `uf`.`id`=`fac`.`file_id`" \
    " WHERE `uf`.`id`=?' USING FROM_BASE64('%s');"
    size_t raw_sql_sz = strlen(cfg->resource_id) + (cfg->fetch_acl?sizeof(SQL2_PATTERN):sizeof(SQL1_PATTERN));
    char raw_sql[raw_sql_sz];
    {
        const char *chosen_sql_patt = (cfg->fetch_acl?SQL2_PATTERN:SQL1_PATTERN);
        size_t nwrite = snprintf(&raw_sql[0], raw_sql_sz, chosen_sql_patt, cfg->resource_id);
        raw_sql[nwrite++] = 0x0;
        assert(nwrite < raw_sql_sz);
    }
#undef SQL1_PATTERN
#undef SQL2_PATTERN
    _aacl_ctx_t  *ctx = calloc(1, sizeof(_aacl_ctx_t));
    COPY_CFG_TO_CTX(ctx, cfg)
    ctx->_num_internal_args = ctx->fetch_acl? 5: 4; // plus number of flags in file-lvl access control table
    uint16_t tot_num_usr_args = ctx->_num_internal_args + ctx->_num_usr_args;
    void *db_async_usr_args[tot_num_usr_args];
    memset(&db_async_usr_args[0], 0x0, sizeof(void *) * ctx->_num_internal_args);
    db_async_usr_args[0] = ctx;
    memcpy(&db_async_usr_args[ctx->_num_internal_args], cfg->usr_args.entries, sizeof(void *) * ctx->_num_usr_args);
    db_query_cfg_t  db_cfg = {
        .statements = {.entry=&raw_sql[0], .num_rs=1}, .pool=cfg->db_pool, .loop=cfg->loop,
        .usr_data = {.entry=(void **)&db_async_usr_args[0], .len=tot_num_usr_args},
        .callbacks = { .result_rdy=appacl_db_dummy_cb,  .error=appacl_db_common_error_cb,
            .row_fetched = _app_acl_resource_id__row_fetch, .result_free = _app_acl_resource_id__rs_free,
        }};
    DBA_RES_CODE  result = app_db_query_start(&db_cfg);
    err = result != DBA_RESULT_OK;
    return err;
} // end of app_acl_verify_resource_id


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
                .edit_acl = (uint8_t) json_boolean_value(json_object_get(new_detail, "edit_acl")),
                .transcode = (uint8_t) json_boolean_value(json_object_get(new_detail, "transcode"))
            }
        };
    } // end of new data iteration
    *num_update = _num_update;
    *num_deletion = _num_deletion;
    *num_insertion = _num_insertion;
} // end of app_acl__build_update_lists


#define  SAVE_ACL__RUN_SQL__COMMON_CODE \
    _aacl_ctx_t  *ctx = calloc(1, sizeof(_aacl_ctx_t)); \
    COPY_CFG_TO_CTX(ctx, cfg) \
    ctx->_num_internal_args = 1; \
    uint16_t tot_num_usr_args = ctx->_num_internal_args + ctx->_num_usr_args; \
    void *db_async_usr_args[tot_num_usr_args]; \
    db_async_usr_args[0] = ctx; \
    memcpy(&db_async_usr_args[ctx->_num_internal_args], cfg->usr_args.entries, sizeof(void *) * ctx->_num_usr_args); \
    db_query_cfg_t  db_cfg = { .loop=cfg->loop, .pool=cfg->db_pool, \
        .usr_data={.entry=(void **)&db_async_usr_args[0], .len=tot_num_usr_args}, \
        .callbacks = {.result_rdy=appacl_db_write_done_cb, .row_fetched=appacl_db_dummy_cb, \
            .result_free=appacl_db_dummy_cb,  .error=appacl_db_common_error_cb }, \
        .statements = {.entry=&final_sql[0], .num_rs=1} \
    }; \
    DBA_RES_CODE  db_result = app_db_query_start(&db_cfg); \
    int err = db_result != DBA_RESULT_OK; \
    if(err) \
        free(ctx); \
    return err;


int  app_usrlvl_acl_save(aacl_cfg_t *cfg, aacl_result_t *existing_data, json_t *new_data)
{
#define  PREP_STMT_LABEL_INSERT   "app_media_usrlvl_acl_insert"
#define  PREP_STMT_LABEL_UPDATE   "app_media_usrlvl_acl_update"
#define  PREP_STMT_LABEL_DELETE   "app_media_usrlvl_acl_delete"
#define  SQL_EXEC_INSERT  "EXECUTE `"PREP_STMT_LABEL_INSERT"` USING %1u,%1u,"SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  SQL_EXEC_UPDATE  "EXECUTE `"PREP_STMT_LABEL_UPDATE"` USING %1u,%1u,"SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  SQL_EXEC_DELETE  "EXECUTE `"PREP_STMT_LABEL_DELETE"` USING "SQL_BASE64_ENCODED_RESOURCE_ID",%u;"
#define  FINAL_SQL_PATTERN  \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `"PREP_STMT_LABEL_INSERT"` FROM 'INSERT INTO `"USRLVL_ACL_TABLE"`(`transcode_flg`,`edit_acl_flg`,`file_id`,`usr_id`) VALUES (?,?,?,?)';" \
    "  PREPARE `"PREP_STMT_LABEL_UPDATE"` FROM 'UPDATE `"USRLVL_ACL_TABLE"` SET `transcode_flg`=?,`edit_acl_flg`=? WHERE `file_id`=? AND `usr_id`=?';" \
    "  PREPARE `"PREP_STMT_LABEL_DELETE"` FROM 'DELETE FROM `"USRLVL_ACL_TABLE"` WHERE `file_id`=? AND `usr_id`=?';" \
    "  START TRANSACTION;" \
    "    %s %s %s" \
    "  COMMIT;" \
    "END;"
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
                nwrite = snprintf(ptr, avail_buf_sz, SQL_EXEC_INSERT, data->capability.transcode,
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
                nwrite = snprintf(ptr, avail_buf_sz, SQL_EXEC_UPDATE, data->capability.transcode,
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
    SAVE_ACL__RUN_SQL__COMMON_CODE
#undef   FINAL_SQL_PATTERN
#undef   SQL_EXEC_INSERT
#undef   SQL_EXEC_UPDATE
#undef   SQL_EXEC_DELETE
#undef   PREP_STMT_LABEL_INSERT
#undef   PREP_STMT_LABEL_UPDATE
#undef   PREP_STMT_LABEL_DELETE
} // end of app_usrlvl_acl_save


int  app_filelvl_acl_save(aacl_cfg_t *cfg, json_t *existing_data, json_t *new_data)
{
#define  SQL_INSERT_PATTERN   "EXECUTE IMMEDIATE 'INSERT INTO `"FILELVL_ACL_TABLE"`(`visible_flg`,`file_id`) VALUES(?,?)'" \
    " USING %1u,"SQL_BASE64_ENCODED_RESOURCE_ID";"
#define  SQL_UPDATE_PATTERN   "EXECUTE IMMEDIATE 'UPDATE `"FILELVL_ACL_TABLE"` SET `visible_flg`=? WHERE `file_id`=?'" \
    " USING %1u,"SQL_BASE64_ENCODED_RESOURCE_ID";"
    uint8_t visible_new = json_boolean_value(json_object_get(new_data, "visible"));
    if(existing_data) {
        uint8_t visible_old = json_boolean_value(json_object_get(existing_data, "visible"));
        if(visible_old == visible_new)
            return 1;
    }
    size_t  final_sql_sz = strlen(cfg->resource_id) + 1 + 
        (existing_data ? sizeof(SQL_UPDATE_PATTERN): sizeof(SQL_INSERT_PATTERN));
    char  final_sql[final_sql_sz];
    {
        const char *chosen_sql_patt = (existing_data ? SQL_UPDATE_PATTERN: SQL_INSERT_PATTERN);
        size_t nwrite = snprintf(&final_sql[0], final_sql_sz, chosen_sql_patt, visible_new, cfg->resource_id);
        assert(nwrite < final_sql_sz);
    }
    SAVE_ACL__RUN_SQL__COMMON_CODE
#undef   SQL_INSERT_PATTERN
#undef   SQL_UPDATE_PATTERN
} // end of app_filelvl_acl_save
