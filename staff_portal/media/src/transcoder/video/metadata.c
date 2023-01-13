#include <jansson.h>
#include "models/pool.h"
#include "models/query.h"
#include "transcoder/video/common.h"

#define UINT32_STR_SIZE     10
#define UINT8_STR_SIZE      3

static void _db_async_dummy_cb(db_query_t *target, db_query_result_t *detail)
{ (void *)detail; }

static  void  atfp_video__dst_update_metadata__db_async_err(db_query_t *target, db_query_result_t *detail)
{
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    json_object_set_new(processor->data.error, "model",
            json_string("[video] failed to update metadata of transcoded video"));
    fprintf(stderr, "[transcoder][video][metadata] line:%d, job_id:%s, unknown db error \n",
            __LINE__, processor->data.rpc_receipt->job_id.bytes);
    processor->data.callback(processor);
} // end of atfp_video__dst_update_metadata__db_async_err


static void  atfp_video__dst_update_metadata__rs_rdy(db_query_t *target, db_query_result_t *detail)
{
    atfp_t *processor = (atfp_t *) target->cfg.usr_data.entry[0];
    atfp_storage__commit_new_version(processor);
} // end of atfp_video__dst_update_metadata__rs_rdy


#define SQL_PATTERN__METADATA_INSERT  \
    "EXECUTE IMMEDIATE 'INSERT INTO `transcoded_video_metadata`(`file_id`,`version`,`size_bytes`" \
    ",`height_pixel`,`width_pixel`,`framerate`) VALUES (?,?,?,?,?,?)' USING FROM_BASE64('%s'),'%s',%u,%u,%u,%u;"

#define SQL_PATTERN__METADATA_UPDATE  \
    "EXECUTE IMMEDIATE 'UPDATE `transcoded_video_metadata` SET `height_pixel`=?,`width_pixel`=?,`framerate`=?" \
    " ,`size_bytes`=? WHERE `file_id`=? AND `version`=?' USING %u,%u,%u,%u,FROM_BASE64('%s'),'%s';"

void  atfp_video__dst_update_metadata(atfp_t *processor, void *loop)
{
    uv_loop_t *_loop = (uv_loop_t *) loop;
    uint32_t  total_nbytes = processor->transfer.transcoded_dst.tot_nbytes_file;
    json_t  *req_spec = processor->data.spec;
    const char *version = processor->data.version;
    const char *res_id_encoded = json_string_value(json_object_get(req_spec, "res_id_encoded"));
    const char *metadata_db_label = json_string_value(json_object_get(req_spec, "metadata_db"));
    json_t  *output   = json_object_get(json_object_get(req_spec, "outputs"), version);
    json_t  *elm_st_map = json_object_get(req_spec, "elementary_streams");
    uint32_t height = 0, width = 0;
    uint8_t  framerate = 0;
    ATFP_VIDEO__READ_SPEC(output, elm_st_map, height, width, framerate);
    size_t  res_id_encoded_sz = strlen(res_id_encoded), version_sz = strlen(version);
    size_t  raw_sql_sz = UINT32_STR_SIZE * 3 + UINT8_STR_SIZE + version_sz + res_id_encoded_sz;
    size_t  nb_rawsql_used = 0;
    uint8_t  _is_update =  processor->transfer.transcoded_dst.flags.version_exists;
    raw_sql_sz += (_is_update) ? sizeof(SQL_PATTERN__METADATA_UPDATE): sizeof(SQL_PATTERN__METADATA_INSERT);
    char raw_sql[raw_sql_sz];
    if(_is_update) {
        nb_rawsql_used = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN__METADATA_UPDATE,
                height, width, framerate, total_nbytes, res_id_encoded, version);
    } else {
        nb_rawsql_used = snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN__METADATA_INSERT,
                res_id_encoded, version, total_nbytes, height, width, framerate );
    }
    assert(nb_rawsql_used <= raw_sql_sz);
    void *db_async_usr_data[2] = {(void *)processor, loop};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)db_async_usr_data, .len = 2},
        .pool = app_db_pool_get_pool(metadata_db_label),
        .loop = _loop,
        .callbacks = {
            .result_rdy  = atfp_video__dst_update_metadata__rs_rdy,
            .row_fetched = _db_async_dummy_cb,
            .result_free = _db_async_dummy_cb,
            .error = atfp_video__dst_update_metadata__db_async_err,
        }
    };
    DBA_RES_CODE  db_result = app_db_query_start(&cfg);
    if(db_result != DBA_RESULT_OK) {
        json_object_set_new(processor->data.error, "model", json_string("failed to send SQL command for transcoded video"));
        fprintf(stderr, "[transcoder][video][metadata] line:%d, job_id:%s, db_result:%d \n",
                __LINE__, processor->data.rpc_receipt->job_id.bytes, db_result);
    }
} // end of atfp_video__dst_update_metadata

#undef  SQL_PATTERN__METADATA_INSERT 
#undef  SQL_PATTERN__METADATA_UPDATE 
