#include <jansson.h>
#include "models/pool.h"
#include "models/query.h"
#include "transcoder/image/common.h"

#define  UINT32_STR_SIZE  10
#define  UINT16_STR_SIZE  5
#define  MAX_SZ_MSK_PATT_LABEL  32

static void _db_async_dummy_cb(db_query_t *target, db_query_result_t *detail)
{
    (void *)detail;
    (void *)target;
}

static  void  atfp_image__dst_update_metadata__db_async_err(db_query_t *target, db_query_result_t *detail)
{
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    json_object_set_new(processor->data.error, "model",
            json_string("[image] failed to update metadata of output file"));
    fprintf(stderr, "[transcoder][image][metadata] line:%d, job_id:%s, unknown db error \n",
            __LINE__, processor->data.rpc_receipt->job_id.bytes);
    processor->data.callback(processor);
} // end of atfp_image__dst_update_metadata__db_async_err

static void  atfp_image__dst_update_metadata__rs_rdy(db_query_t *target, db_query_result_t *detail)
{
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    atfp_storage__commit_new_version(processor);
} // end of atfp_image__dst_update_metadata__rs_rdy


#define  DB_TABLE_NAME   "transformed_image_metadata"
#define  PREP_STMT_LABEL_INSERT  "app_media_transform_img_insert"
#define  PREP_STMT_LABEL_UPDATE  "app_media_transform_img_update"

#define  SQL_PATTERN_INSERT \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `"PREP_STMT_LABEL_INSERT"` FROM 'INSERT INTO `"DB_TABLE_NAME"`(`file_id`,`version`,`size_bytes`" \
    "    ,`scale_h`,`scale_w`,`crop_h`,`crop_w`,`crop_x`,`crop_y`,`mask_patt`) VALUES (?,?,?,?,?,?,?,?,?,?)';" \
    "  EXECUTE `"PREP_STMT_LABEL_INSERT"` USING FROM_BASE64('%s'),'%s',%u,%hu,%hu,%hu,%hu,%hu,%hu,'%s';" \
    "END;"

#define SQL_PATTERN_UPDATE \
    "BEGIN NOT ATOMIC" \
    "  PREPARE `"PREP_STMT_LABEL_UPDATE"` FROM 'UPDATE `"DB_TABLE_NAME"` SET `size_bytes`=?,`scale_h`=?" \
    "    ,`scale_w`=?,`crop_h`=?,`crop_w`=?,`crop_x`=?,`crop_y`=?,`mask_patt`=? WHERE `file_id`=? AND `version`=?';" \
    "  EXECUTE `"PREP_STMT_LABEL_UPDATE"` USING %u,%hu,%hu,%hu,%hu,%hu,%hu,'%s',FROM_BASE64('%s'),'%s';" \
    "END;"

void  atfp_image__dst_update_metadata(atfp_t *processor, void *loop)
{
    uv_loop_t *_loop = (uv_loop_t *) loop;
    json_t *err_info = processor->data.error;
    json_t *req_spec = processor->data.spec;
    uint32_t  total_nbytes = processor->transfer.transcoded_dst.tot_nbytes_file;
    const char *_version = processor->data.version;
    const char *res_id_encoded = json_string_value(json_object_get(req_spec, "res_id_encoded"));
    const char *metadata_db_label = json_string_value(json_object_get(req_spec, "metadata_db"));
    json_t  *filt_spec = json_object_get(json_object_get(req_spec, "outputs"), _version);
    json_t *_mask_item  = json_object_get(filt_spec, "mask");
    json_t *_crop_item  = json_object_get(filt_spec, "crop");
    json_t *_scale_item = json_object_get(filt_spec, "scale");
    uint16_t  scale_height = (uint16_t)json_integer_value(json_object_get(_scale_item, "height"));
    uint16_t  scale_width  = (uint16_t)json_integer_value(json_object_get(_scale_item, "width"));
    uint16_t  crop_height  = (uint16_t)json_integer_value(json_object_get(_crop_item, "height"));
    uint16_t  crop_width   = (uint16_t)json_integer_value(json_object_get(_crop_item, "width"));
    uint16_t  crop_x = (uint16_t)json_integer_value(json_object_get(_crop_item, "x"));
    uint16_t  crop_y = (uint16_t)json_integer_value(json_object_get(_crop_item, "y"));
    const char *msk_patt_label = json_string_value(json_object_get(_mask_item, "pattern"));

    uint8_t  _is_update =  processor->transfer.transcoded_dst.flags.version_exists;
    size_t  res_id_encoded_sz = strlen(res_id_encoded), version_sz = strlen(_version);
    size_t  raw_sql_sz = UINT32_STR_SIZE + UINT16_STR_SIZE * 6 + MAX_SZ_MSK_PATT_LABEL +
        version_sz + res_id_encoded_sz;
    raw_sql_sz += (_is_update) ? sizeof(SQL_PATTERN_UPDATE): sizeof(SQL_PATTERN_INSERT);
    char raw_sql[raw_sql_sz];
    size_t  nb_rawsql_used = 0;
    if(_is_update) {
        nb_rawsql_used = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN_UPDATE, total_nbytes,
                scale_height, scale_width, crop_height, crop_width, crop_x, crop_y, msk_patt_label,
                res_id_encoded, _version);
    } else {
        nb_rawsql_used = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN_INSERT,  res_id_encoded,
                _version, total_nbytes, scale_height, scale_width, crop_height, crop_width,
                crop_x, crop_y, msk_patt_label );
    }
    assert(nb_rawsql_used <= raw_sql_sz);
    void *db_async_usr_args[2] = {(void *)processor, loop};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1}, .loop = _loop,
        .usr_data = {.entry = (void **)db_async_usr_args, .len = 2},
        .pool = app_db_pool_get_pool(metadata_db_label),
        .callbacks = {
            .result_rdy  = atfp_image__dst_update_metadata__rs_rdy,
            .row_fetched = _db_async_dummy_cb,
            .result_free = _db_async_dummy_cb,
            .error = atfp_image__dst_update_metadata__db_async_err,
        }
    };
    DBA_RES_CODE  db_result = app_db_query_start(&cfg);
    if(db_result != DBA_RESULT_OK) {
        json_object_set_new(err_info, "model", json_string("[image] failed to update metadata"));
        fprintf(stderr, "[transcoder][video][metadata] line:%d, job_id:%s, db_result:%d \n",
            __LINE__, processor->data.rpc_receipt->job_id.bytes, db_result);
    }
} // end of  atfp_image__dst_update_metadata
