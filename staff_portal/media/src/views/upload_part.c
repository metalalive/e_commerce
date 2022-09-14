#include <openssl/sha.h>

#include "utils.h"
#include "views.h"
#include "multipart_parser.h"
#include "models/pool.h"
#include "models/query.h"
#include "storage/cfg_parser.h"
#include "storage/localfs.h"

#define APP_FILECHUNK_WR_BUF_SZ  128
#define UPLOAD_PART_NUM_SIZE  5

typedef struct {
    size_t rd_idx;
    size_t wr_idx;
    size_t tot_entity_sz;
    size_t tot_file_sz;
    size_t tot_wr_sz;
    char  *wr_buf;
    uint8_t num_parts;
    uint8_t end_flag:1;
    SHA_CTX  checksum;
} app_mpp_usrarg_t;


static void  upload_part__db_async_err(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
    h2o_send_error_503(req, "server temporarily unavailable", "", H2O_SEND_ERROR_KEEP_HEADERS);
    {
        char *md = app_fetch_from_hashmap(node->data, "chunk_checksum");
        if(md) {
            free(md);
            app_save_ptr_to_hashmap(node->data, "chunk_checksum", (void *)NULL);
        }
    }
    app_run_next_middleware(self, req, node);
} // end of upload_part__db_async_err


static void  upload_part__add_chunk_record_rs_rdy(db_query_t *target, db_query_result_t *rs)
{
    assert(rs->_final);
    assert(rs->app_result == DBA_RESULT_END_OF_ROWS_REACHED);
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint16_t part_num    = (uint16_t)app_fetch_from_hashmap(node->data, "part");
#pragma GCC diagnostic pop
    char *chunk_checksum = (char *)app_fetch_from_hashmap(node->data, "chunk_checksum");
    { // construct response body
        json_t *res_body = json_object();
        json_object_set_new(res_body, "checksum", json_string(chunk_checksum));
        json_object_set_new(res_body, "alg", json_string("sha1"));
        json_object_set_new(res_body, "part", json_integer(part_num));
        req->res.status = 200;
        req->res.reason = "OK";
#define  MAX_BYTES_RESP_BODY  128
        char body_raw[MAX_BYTES_RESP_BODY];
        size_t nwrite = json_dumpb((const json_t *)res_body, &body_raw[0],  MAX_BYTES_RESP_BODY, JSON_COMPACT);
        h2o_add_header(&req->pool, &req->res.headers, H2O_TOKEN_CONTENT_TYPE, NULL, H2O_STRLIT("application/json"));    
        h2o_send_inline(req, body_raw, nwrite);
        json_decref(res_body);
#undef   MAX_BYTES_RESP_BODY
    }
    free(chunk_checksum);
    app_save_ptr_to_hashmap(node->data, "chunk_checksum", (void *)NULL);
    app_run_next_middleware(self, req, node);
} // end of upload_part__add_chunk_record_rs_rdy


static DBA_RES_CODE upload_part__add_chunk_record_to_db(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{ // TODO, allow users to re-upload the same file chunk
#define SQL_PATTERN "INSERT INTO `upload_filechunk`(`usr_id`,`req_id`,`part`,`checksum`,`size_bytes`)" \
    " VALUES(%u, x'%08x', %hu, x'%s', %u);"
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    int usr_id = (int) json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    uint16_t part    = (uint16_t)app_fetch_from_hashmap(node->data, "part");
    uint32_t req_seq = (uint32_t)app_fetch_from_hashmap(node->data, "req_seq");
    char *chunk_checksum  = app_fetch_from_hashmap(node->data, "chunk_checksum");
    uint32_t chunk_nbytes = (uint32_t)app_fetch_from_hashmap(node->data, "chunk_nbytes");
#pragma GCC diagnostic pop
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(req_seq) +
            UPLOAD_PART_NUM_SIZE + strlen(chunk_checksum) + USR_ID_STR_SIZE;
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    snprintf(&raw_sql[0], raw_sql_sz, SQL_PATTERN, usr_id, req_seq, part, chunk_checksum, chunk_nbytes);
    void *db_async_usr_data[3] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = 3},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = upload_part__add_chunk_record_rs_rdy,
            .row_fetched = app_db_async_dummy_cb,
            .result_free = app_db_async_dummy_cb,
            .error = upload_part__db_async_err,
        }
    };
    return app_db_query_start(&cfg);
#undef SQL_PATTERN
} // end of upload_part__add_chunk_record_to_db


static size_t app_find_multipart_boundary(h2o_req_t *req, char **start)
{
    char *boundary = NULL;
    size_t len = 0;
    for(size_t idx = 0; idx < req->headers.size; idx++) {
        h2o_iovec_t *name  =  req->headers.entries[idx].name;
        h2o_iovec_t *value = &req->headers.entries[idx].value;
        if(strncmp("content-type", name->base, name->len) == 0) {
            boundary = strstr((const char *)value->base, "boundary=");
            if(boundary) {
                boundary += strlen("boundary=");
            } else {
                fprintf(stderr, "[error] missing boundary in multipart/form-data\r\n");
            }
            break;
        }
    }
    if(boundary) {
        if (start) {
            *start = boundary;
        }
        len = strlen(boundary);
    }
    return len;
} // end of app_find_multipart_boundary


static void upload_part__storage_error_handler(asa_op_base_cfg_t *cfg) {
    h2o_req_t     *req  = (void *)cfg->cb_args.entries[0];
    h2o_handler_t *self = (void *)cfg->cb_args.entries[1];
    app_middleware_node_t *node = (void *)cfg->cb_args.entries[2];
    h2o_send_error_500(req, "storage setup error", "", H2O_SEND_ERROR_KEEP_HEADERS);
    multipart_parser *mp = (multipart_parser *)app_fetch_from_hashmap(node->data, "multipart_parser");
    if(mp) {
        app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
        OPENSSL_cleanse(&usr_arg->checksum, sizeof(usr_arg->checksum));
        multipart_parser_free(mp);
        app_save_ptr_to_hashmap(node->data, "multipart_parser", NULL);
    }
    app_run_next_middleware(self, req, node);
    free(cfg);
} // end of upload_part__storage_error_handler


static  int upload_part__multipart_parser__on_part_data(multipart_parser *mp, const char *at, size_t len)
{
    int err = 0;
    app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
    size_t new_wr_idx = usr_arg->wr_idx + len;
    if (new_wr_idx <= usr_arg->tot_wr_sz) {
        char *dst = & usr_arg->wr_buf[ usr_arg->wr_idx ];
        const char *src = at;
        memcpy(dst, src, len);
        usr_arg->wr_idx = new_wr_idx;
    } else { // write buffer overflow, immediately abort
        err = 1;
    }
    return err;
} // end of upload_part__multipart_parser__on_part_data


static int  upload_part__multipart_parser__on_part_data_begin(multipart_parser *mp)
{ // this API view does NOT allow more than one encapsulted parts in multipart entity
    int err = 0;
    app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
    usr_arg->num_parts += 1;
    if(usr_arg->num_parts > 1) {
        err = 1;
    }
    return err;
} // end of upload_part__multipart_parser__on_part_data_begin

static int  upload_part__multipart_parser__on_body_end(multipart_parser *mp)
{
    app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
    usr_arg->end_flag = 1;
    return 0;
} // end of upload_part__multipart_parser__on_body_end


static multipart_parser * upload_part__init_multipart_parser(asa_op_base_cfg_t *cfg)
{
    h2o_req_t  *req  = (void *)cfg->cb_args.entries[0];
    char  *boundary_start = NULL;
    size_t boundary_len = app_find_multipart_boundary(req, &boundary_start);
    char boundary_cpy[boundary_len + 1];
    if(!boundary_start || boundary_len == 0) {
        goto error;
    }
    memcpy(boundary_cpy, boundary_start, boundary_len);
    boundary_cpy[boundary_len] = 0;
    app_mpp_usrarg_t  mpp_usr_arg = {.tot_entity_sz=req->entity.len, .tot_file_sz=0, .rd_idx=0,
        .wr_idx=0,  .wr_buf=cfg->op.write.src, .tot_wr_sz=cfg->op.write.src_max_nbytes,
        .num_parts=0, .end_flag=0, .checksum = {0}};
    multipart_parser_settings  settings = {
        .usr_args = {.sz = sizeof(app_mpp_usrarg_t) , .entry = (void *)&mpp_usr_arg},
        .cbs = {.on_part_data = upload_part__multipart_parser__on_part_data,
            .on_part_data_begin = upload_part__multipart_parser__on_part_data_begin,
            .on_body_end = upload_part__multipart_parser__on_body_end,
        }
    };
    multipart_parser *out = multipart_parser_init(&boundary_cpy[0], &settings);
    if(out) {
        app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)out->settings.usr_args.entry;
        if(!SHA1_Init(&usr_arg->checksum)) {
            multipart_parser_free(out);
            goto error;
        }
    }
    return out;
error:
    return NULL;
} // upload_part__init_multipart_parser


static  ASA_RES_CODE upload_part__write_filechunk_start(asa_op_base_cfg_t *cfg)
{ // TODO, figure out how to test this function
    ASA_RES_CODE asa_result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    h2o_req_t *req  = (void *)cfg->cb_args.entries[0];
    app_middleware_node_t *node = (void *)cfg->cb_args.entries[2];
    multipart_parser *mp = (multipart_parser *)app_fetch_from_hashmap(node->data, "multipart_parser");
    app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
    usr_arg->wr_idx = 0; // reset write index before parsing new portion of data
    while(usr_arg->wr_idx == 0) {
        if (usr_arg->tot_entity_sz <= usr_arg->rd_idx) {
            asa_result = ASTORAGE_RESULT_DATA_ERROR;
            break;
        }
        size_t exp_rd_sz = usr_arg->tot_entity_sz - usr_arg->rd_idx - 1;
        if(exp_rd_sz > APP_FILECHUNK_WR_BUF_SZ) {
            // due to restriction of multipart parser, don't use entire write buffer usr_arg->tot_wr_sz
            exp_rd_sz = APP_FILECHUNK_WR_BUF_SZ; // usr_arg->tot_wr_sz
        }
        size_t actual_nread = multipart_parser_execute(mp, &req->entity.base[ usr_arg->rd_idx ], exp_rd_sz);
        if(actual_nread == exp_rd_sz || usr_arg->end_flag) {
            usr_arg->rd_idx += exp_rd_sz;
            if(usr_arg->wr_idx > 0) {
                cfg->op.write.src_sz = usr_arg->wr_idx;
                usr_arg->tot_file_sz += usr_arg->wr_idx;
                SHA1_Update(&usr_arg->checksum, cfg->op.write.src, cfg->op.write.src_sz);
                asa_result = cfg->storage->ops.fn_write(cfg);
            } else if(usr_arg->end_flag) {
                // implicitly means wr_idx == 0, nothing to write in the final parsed chunk.
                asa_result = cfg->storage->ops.fn_close(cfg);
                break;
            }
            // Eventually multipaart-parser will reach the 2 statements above as it
            // traversed all bytes of the request body.
        } else { // TODO, logging possible error
            asa_result = ASTORAGE_RESULT_OS_ERROR;
            break;
        }
    } // end of loop
    return asa_result;
} // end of upload_part__write_filechunk_start


static void upload_part__write_filechunk_evt_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result, size_t nwrite)
{
    if(app_result != ASTORAGE_RESULT_COMPLETE) {
        goto error;
    }
    app_middleware_node_t *node = (void *)cfg->cb_args.entries[2];
    multipart_parser *mp = (multipart_parser *)app_fetch_from_hashmap(node->data, "multipart_parser");
    app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
    if(usr_arg->end_flag) { // close file and add chunk record to database
        app_result = cfg->storage->ops.fn_close(cfg);
    } else {
        app_result = upload_part__write_filechunk_start(cfg);
    }
    if(app_result != ASTORAGE_RESULT_ACCEPT) {
        goto error;
    }
    return;
error:
    upload_part__storage_error_handler(cfg);
} // end of upload_part__write_filechunk_evt_cb


static void upload_part__open_file_evt_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result)
{
    if(app_result != ASTORAGE_RESULT_COMPLETE) {
        goto error;
    }
    app_middleware_node_t *node = (void *)cfg->cb_args.entries[2];
    multipart_parser *mp = upload_part__init_multipart_parser(cfg);
    if(!mp) { goto error; }
    int success = app_save_ptr_to_hashmap(node->data, "multipart_parser", (void *)mp);
    if(!success) { goto error; }
    app_result = upload_part__write_filechunk_start(cfg);
    if(app_result != ASTORAGE_RESULT_ACCEPT) {
        goto error;
    }
    return;
error:
    upload_part__storage_error_handler(cfg);
} // end of upload_part__open_file_evt_cb


static void  upload_part__close_file_evt_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result)
{
    if(app_result != ASTORAGE_RESULT_COMPLETE) {
        goto error;
    }
    char *md_hex = NULL;
    h2o_req_t     *req  = (void *)cfg->cb_args.entries[0];
    h2o_handler_t *self = (void *)cfg->cb_args.entries[1];
    app_middleware_node_t *node = (void *)cfg->cb_args.entries[2];
    { // de-init storage object and multipart parser object
        multipart_parser *mp = (multipart_parser *)app_fetch_from_hashmap(node->data, "multipart_parser");
        app_mpp_usrarg_t *usr_arg = (app_mpp_usrarg_t *)mp->settings.usr_args.entry;
        {
            size_t md_hex_sz = SHA_DIGEST_LENGTH << 1;
            char md[SHA_DIGEST_LENGTH] = {0};
            SHA1_Final((unsigned char *)&md[0], &usr_arg->checksum);
            md_hex = malloc(sizeof(char) * (md_hex_sz + 1)); // 20 * 2 + NULL bytes
            app_chararray_to_hexstr(&md_hex[0], md_hex_sz, &md[0], SHA_DIGEST_LENGTH);
            md_hex[md_hex_sz] = 0x0;
        }
        OPENSSL_cleanse(&usr_arg->checksum, sizeof(usr_arg->checksum));
        app_save_ptr_to_hashmap(node->data, "chunk_checksum", (void *)md_hex);
        app_save_int_to_hashmap(node->data, "chunk_nbytes", (int)usr_arg->tot_file_sz);
        app_save_ptr_to_hashmap(node->data, "multipart_parser", NULL);
        multipart_parser_free(mp);
        free(cfg);
    }
    DBA_RES_CODE db_result = upload_part__add_chunk_record_to_db(self, req, node);
    if(db_result != DBA_RESULT_OK) {
        free(md_hex);
        goto error;
    } // TODO, may delete the file if database error happens later, or let it obsolete ?
    return;
error:
    upload_part__storage_error_handler(cfg);
} // end of upload_part__close_file_evt_cb


static void upload_part__create_folder_evt_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE app_result)
{
    uint8_t err = 0;
    if(app_result == ASTORAGE_RESULT_COMPLETE) {
        cfg->op.write.cb = upload_part__write_filechunk_evt_cb;
        cfg->op.close.cb = upload_part__close_file_evt_cb;
        cfg->op.open.cb = upload_part__open_file_evt_cb;
        cfg->op.open.mode  = S_IRUSR | S_IWUSR;
        cfg->op.open.flags = O_CREAT | O_WRONLY;
        app_result = cfg->storage->ops.fn_open(cfg);
        if(app_result != ASTORAGE_RESULT_ACCEPT) {
            err = 1;
        }
    } else {
        err = 1;
    }
    if(err) {
        upload_part__storage_error_handler(cfg);
    }
} // end of upload_part__create_folder_evt_cb


static void upload_part__create_folder_start(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    int usr_id = (int) json_integer_value(json_object_get(jwt_claims, "profile"));
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    int req_seq = (int)app_fetch_from_hashmap(node->data, "req_seq");
    int part    = (int)app_fetch_from_hashmap(node->data, "part");
#pragma GCC diagnostic pop
    // application can select which storage configuration to use
    asa_cfg_t *storage = app_storage_cfg_lookup("localfs");
    size_t dirpath_sz = strlen(storage->base_path) + 1 + USR_ID_STR_SIZE + 1 +
         UPLOAD_INT2HEX_SIZE(req_seq) + 1; // assume NULL-terminated string
    size_t filepath_sz = (dirpath_sz - 1) + 1 + UPLOAD_INT2HEX_SIZE(part) + 1;
    size_t cb_args_tot_sz = sizeof(void *) * 3; // for self, req, node
    size_t mp_boundary_len = app_find_multipart_boundary(req, NULL);
    size_t wr_src_buf_sz = APP_FILECHUNK_WR_BUF_SZ + 4 + mp_boundary_len;
    size_t asa_cfg_sz = sizeof(asa_op_localfs_cfg_t) + (dirpath_sz << 1) + filepath_sz +
            wr_src_buf_sz + cb_args_tot_sz;
    asa_op_localfs_cfg_t  *asa_cfg = calloc(1, asa_cfg_sz);
    { // start of storage object setup
        char *ptr = (char *)asa_cfg + sizeof(asa_op_localfs_cfg_t);
        memset(asa_cfg, 0x0, asa_cfg_sz);
        asa_cfg->loop = req->conn->ctx->loop;
        asa_cfg->super.storage = storage;
        asa_cfg->super.cb_args.size = 3;
        asa_cfg->super.cb_args.entries = (void **) ptr;
        asa_cfg->super.cb_args.entries[0] = (void *)req;
        asa_cfg->super.cb_args.entries[1] = (void *)self;
        asa_cfg->super.cb_args.entries[2] = (void *)node;
        ptr += cb_args_tot_sz;
        asa_cfg->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_cfg->super.op.mkdir.cb = upload_part__create_folder_evt_cb;
        asa_cfg->super.op.mkdir.path.origin = ptr;
        ptr += dirpath_sz;
        asa_cfg->super.op.mkdir.path.curr_parent = ptr;
        ptr += dirpath_sz;
        {
            char dirpath[dirpath_sz];
            snprintf(&dirpath[0], dirpath_sz, "%s/%d/%08x", storage->base_path, usr_id, req_seq);
            dirpath[dirpath_sz - 1] = 0x0; // NULL-terminated
            memcpy(asa_cfg->super.op.mkdir.path.origin, dirpath, dirpath_sz);
        }
        asa_cfg->super.op.open.dst_path = ptr;
        ptr += filepath_sz;
        {
            char filepath[filepath_sz];
            snprintf(&filepath[0], filepath_sz, "%s/%d/%08x/%d", storage->base_path, usr_id, req_seq, part);
            filepath[filepath_sz - 1] = 0x0;
            memcpy(asa_cfg->super.op.open.dst_path, filepath, filepath_sz);
        }
        asa_cfg->super.op.write.offset = APP_STORAGE_USE_CURRENT_FILE_OFFSET;
        asa_cfg->super.op.write.src_max_nbytes = wr_src_buf_sz;
        // will be updated after file chunk is read sequentially from multipart entity
        asa_cfg->super.op.write.src_sz = 0;
        asa_cfg->super.op.write.src = (char *)ptr;
        ptr += wr_src_buf_sz;
        assert((size_t)(ptr - (char *)asa_cfg) == asa_cfg_sz);
    } // end of storage object setup
    ASA_RES_CODE asa_result = storage->ops.fn_mkdir((asa_op_base_cfg_t *)asa_cfg, 1);
    if (asa_result != ASTORAGE_RESULT_ACCEPT) {
        upload_part__storage_error_handler((asa_op_base_cfg_t *)asa_cfg);
    }
} // end of  upload_part__create_folder_start


static void  upload_part__validate_quota_rs_rdy(db_query_t *target, db_query_result_t *rs)
{
    h2o_req_t     *req  = (h2o_req_t *) target->cfg.usr_data.entry[0];
    h2o_handler_t *self = (h2o_handler_t *) target->cfg.usr_data.entry[1];
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t total_used_bytes = (size_t)app_fetch_from_hashmap(node->data, "total_uploaded_bytes");
#pragma GCC diagnostic pop
    // TODO, provide more accurate file size, since the request body contains extra
    //  bytes such as boundary delimiter for multipart entity.
    total_used_bytes +=  req->entity.len;
    // this API view requires quota arrangement `QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER`
    // to be present in auth JWT payload
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    json_t *quota = app_find_quota_arragement(jwt_claims, APP_CODE, QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER);
    size_t max_limit_bytes = (size_t) json_integer_value(json_object_get(quota, "maxnum"));
    max_limit_bytes  = max_limit_bytes << 10;
    if(total_used_bytes > max_limit_bytes) {
        char body_raw[] = "{\"quota\":\"bytes of uploaded file exceed the limit\"}";
        req->res.status = 403;
        h2o_send_inline(req, body_raw, strlen(body_raw));
        app_run_next_middleware(self, req, node);
    } else {
        upload_part__create_folder_start(self, req, node);
    }
} // end of upload_part__validate_quota_rs_rdy

static void  upload_part__validate_quota_fetch_row(db_query_t *target, db_query_result_t *rs)
{
    app_middleware_node_t *node = (app_middleware_node_t *) target->cfg.usr_data.entry[2];
#pragma GCC diagnostic ignored "-Wpointer-to-int-cast"
    size_t total_used_bytes = (size_t)app_fetch_from_hashmap(node->data, "total_uploaded_bytes");
#pragma GCC diagnostic pop
    db_query_row_info_t *row = (db_query_row_info_t *) &rs->data[0];
    size_t chunk_bytes = (size_t) strtoul(row->values[0], NULL, 10);
    total_used_bytes += chunk_bytes;
    app_save_int_to_hashmap(node->data, "total_uploaded_bytes", total_used_bytes);
} // end of upload_part__validate_quota_fetch_row

static DBA_RES_CODE upload_part__validate_quota_start(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
#define SQL_PATTERN  "SELECT `size_bytes` FROM `upload_filechunk` WHERE `usr_id` = %u;"
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    int usr_id = (int) json_integer_value(json_object_get(jwt_claims, "profile"));
    size_t raw_sql_sz = sizeof(SQL_PATTERN) + USR_ID_STR_SIZE;
    char raw_sql[raw_sql_sz];
    memset(&raw_sql[0], 0x0, raw_sql_sz);
    sprintf(&raw_sql[0], SQL_PATTERN, usr_id);
    void *db_async_usr_data[3] = {(void *)req, (void *)self, (void *)node};
    db_query_cfg_t  cfg = {
        .statements = {.entry = &raw_sql[0], .num_rs = 1},
        .usr_data = {.entry = (void **)&db_async_usr_data, .len = 3},
        .pool = app_db_pool_get_pool("db_server_1"),
        .loop = req->conn->ctx->loop,
        .callbacks = {
            .result_rdy  = app_db_async_dummy_cb,
            .row_fetched = upload_part__validate_quota_fetch_row,
            .result_free = upload_part__validate_quota_rs_rdy,
            .error = upload_part__db_async_err,
        }
    };
    return app_db_query_start(&cfg);
#undef SQL_PATTERN
} // end of upload_part__validate_quota_start


static int upload_part__validate_reqseq_success(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) 
{
    DBA_RES_CODE result = upload_part__validate_quota_start(self, req, node);
    if(result == DBA_RESULT_OK) {
        app_save_int_to_hashmap(node->data, "total_uploaded_bytes", 0);
    } else {
        void *args[3] = {(void *)req, (void *)self, (void *)node};
        db_query_t  fake_q = {.cfg = {.usr_data = {.entry = (void **)&args[0], .len=3}}};
        upload_part__db_async_err(&fake_q, NULL);
    }
    return 0;
}

static int upload_part__validate_reqseq_failure(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node) 
{
    char body_raw[] = "{\"req_seq\":\"request not exists\"}";
    req->res.status = 400;
    h2o_send_inline(req, body_raw, strlen(body_raw));
    app_run_next_middleware(self, req, node);
    return 0;
}


static  uint8_t upload_part__validate_uri_query_param(char *raw_qparams, char *res_body,
        size_t *res_body_sz, app_middleware_node_t *node)
{
    uint8_t err = 0;
    json_t *err_res_obj = json_object();
    json_t *qparams = json_object();
    app_url_decode_query_param(raw_qparams, qparams);
    const char *part_str = json_string_value(json_object_get(qparams, "part"));
    const char *req_seq_str = json_string_value(json_object_get(qparams, "req_seq"));
    int part    = part_str ? (int)strtol(part_str, NULL, 10) : -1;
    int req_seq = req_seq_str ? (int)strtol(req_seq_str, NULL, 10) : -1;
    if(part < 0 || (part >> 16) != 0) {
        json_object_set_new(err_res_obj, "part", json_string("invalid part number"));
        err = 1;
    } // part number is supposed to be 16-bit short integer
    if(req_seq == -1) {
        json_object_set_new(err_res_obj, "req_seq", json_string("missing request ID"));
        err = 1;
    }
    if(err) {
        size_t nwrite = json_dumpb((const json_t *)err_res_obj, res_body, *res_body_sz, JSON_COMPACT);
        assert(*res_body_sz >= nwrite);
        *res_body_sz = nwrite;
    } else { // save the validated value for later use
        app_save_int_to_hashmap(node->data, "part", part);
        app_save_int_to_hashmap(node->data, "req_seq", req_seq);
    }
    json_decref(qparams);
    json_decref(err_res_obj);
    return err;
} // end of upload_part__validate_uri_query_param


// TODO
// * upgrade libh2o , figure out how streaming request work
// * stream request body, parse the chunk with multipart/form parser,
RESTAPI_ENDPOINT_HANDLER(upload_part, POST, self, req)
{
#define MAX_BYTES_RESP_BODY 64
    { // validate the untrusted values
        char body_raw[MAX_BYTES_RESP_BODY] = {0};
        size_t nwrite = MAX_BYTES_RESP_BODY;
        uint8_t validation_error = upload_part__validate_uri_query_param(
                &req->path.base[req->query_at + 1], &body_raw[0], &nwrite, node);
        if(validation_error) {
            req->res.status = 400;
            h2o_send_inline(req, body_raw, nwrite);
            goto error;
        }
    }
    json_t *jwt_claims = (json_t *)app_fetch_from_hashmap(node->data, "auth");
    json_t *quota = app_find_quota_arragement(jwt_claims, APP_CODE, QUOTA_MATERIAL__MAX_UPLOAD_KBYTES_PER_USER);
    if(!quota) {
        h2o_send_error_403(req, "quota required", "", H2O_SEND_ERROR_KEEP_HEADERS);
        goto error;
    }
#undef MAX_BYTES_RESP_BODY
    // now check existence of req_seq, will access database asynchronously
    DBA_RES_CODE db_result = app_validate_uncommitted_upld_req(
            self, req, node, "upload_request", upload_part__db_async_err,
            upload_part__validate_reqseq_success, upload_part__validate_reqseq_failure
        );
    if(db_result == DBA_RESULT_OK) {
        goto done;
    } else {
        h2o_send_error_500(req, "internal error", "", H2O_SEND_ERROR_KEEP_HEADERS);
    }
error:
    app_run_next_middleware(self, req, node);
done:
    return 0;
} // end of upload_part()

