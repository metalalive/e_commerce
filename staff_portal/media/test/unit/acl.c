#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <uv.h>
#include "datatypes.h"
#include "utils.h"
#include "acl.h"

#define  UTEST_DB_ALIAS      "ut_rand_db"
#define  UTEST_RESOURCE_ID   "your_resource_id"

static uint8_t  utest_dbpool__is_conn_closing (db_pool_t *_pool)
{ return (uint8_t)mock(_pool); }

static  db_conn_t *utest_dbpool__acquire_free_conn(db_pool_t *_pool)
{ return (db_conn_t *)mock(_pool); }

static  DBA_RES_CODE utest_dbpool__release_used_conn(db_conn_t *_conn)
{ return (DBA_RES_CODE)mock(_conn); }

static DBA_RES_CODE  utest_dbconn__add_new_query(db_conn_t *_conn, db_query_t *q)
{
    db_llnode_t *q_node = H2O_STRUCT_FROM_MEMBER(db_llnode_t, data, q);
    _conn->processing_queries = q_node;
    return (DBA_RES_CODE)mock(_conn, q);
}

static DBA_RES_CODE  utest_dbconn__try_process_queries (db_conn_t *_conn, uv_loop_t *loop)
{
    db_llnode_t *q_node = _conn->processing_queries;
    db_query_t  *query = (db_query_t *)&q_node->data[0];
    db_query_result_t  *q_result = NULL, **q_result_p = &q_result;
    uint8_t  exp_num_rows = 0, expect_err = 0, expect_write_flg = 0, *exp_num_rows_p = &exp_num_rows,
             *expect_err_p = &expect_err, *expect_write_flg_p = &expect_write_flg;
    DBA_RES_CODE result = (DBA_RES_CODE)mock(_conn, query, exp_num_rows_p, expect_err_p,
            expect_write_flg_p, q_result_p);
    if (expect_write_flg) {
        if(expect_err) {
            query->cfg.callbacks.error(query, q_result);
        } else {
            query->cfg.callbacks.result_rdy (query, q_result);
        }
    } else {
        for(uint8_t idx = 0; idx < exp_num_rows; idx++) {
            q_result = (db_query_result_t *)mock();
            query->cfg.callbacks.row_fetched(query, q_result);
        }
        if(expect_err) {
            query->cfg.callbacks.error(query, q_result);
        } else {
            query->cfg.callbacks.result_free (query, q_result);
        }
    }
    if(q_node) {
        uv_close((uv_handle_t *)&query->notification, NULL);
        uv_run(loop, UV_RUN_ONCE);
        free(q_node);
    }
    return result;
} // end of  utest_dbconn__try_process_queries


static void utest_acl__operation_done_cb (aacl_result_t *_result, void *_usr_data)
{
    size_t  actual_capacity = _result->data.capacity;
    size_t  actual_num_rows = _result->data.size;
    aacl_data_t  *actual_entry_ptr = _result->data.entries;
    uint8_t flg_err   = _result->flag.error;
    uint8_t flg_wr_ok = _result->flag.write_ok;
    mock(actual_capacity, actual_num_rows, actual_entry_ptr, flg_err, flg_wr_ok);
    if(_usr_data) {
        aacl_data_t *expect_entry = _usr_data;
        for(size_t idx = 0; idx < actual_num_rows; idx++) {
            assert_that(actual_entry_ptr[idx].usr_id , is_equal_to(expect_entry[idx].usr_id));
            assert_that(actual_entry_ptr[idx].capability.renew, is_equal_to(expect_entry[idx].capability.renew));
            assert_that(actual_entry_ptr[idx].capability.transcode, is_equal_to(expect_entry[idx].capability.transcode));
            assert_that(actual_entry_ptr[idx].capability.edit_acl,  is_equal_to(expect_entry[idx].capability.edit_acl));
        }
    }
} // end of  utest_acl__operation_done_cb


#define  UTEST_ACL_COMMON_SETUP(rawsql_max_nkbytes) \
    db_pool_t  mock_db_pool = { \
        .cfg = {.bulk_query_limit_kb=rawsql_max_nkbytes}, \
        .is_closing_fn=utest_dbpool__is_conn_closing, \
        .acquire_free_conn_fn=utest_dbpool__acquire_free_conn, \
        .release_used_conn_fn=utest_dbpool__release_used_conn \
    }; \
    db_conn_t  mock_conn = {.ops={.add_new_query=utest_dbconn__add_new_query, \
        .try_process_queries=utest_dbconn__try_process_queries}}; \
    aacl_cfg_t mock_acl_cfg = {.usrdata=NULL, .db_pool=&mock_db_pool, .resource_id=UTEST_RESOURCE_ID, \
                .loop=uv_default_loop(), .callback=utest_acl__operation_done_cb };


#define  UTEST_ACL_LOAD_SETUP(rawsql_max_nkbytes, exp_num_rows, _num_cols) \
    UTEST_ACL_COMMON_SETUP(rawsql_max_nkbytes) \
    uint8_t  expect_num_rows = exp_num_rows, idx = 0; \
    db_query_result_t  *expect_row[exp_num_rows] = {0}; \
    { \
        size_t  single_row_sz = sizeof(db_query_result_t) + sizeof(db_query_row_info_t) + \
               _num_cols * sizeof(char *); \
        char *ptr = calloc(expect_num_rows, single_row_sz); \
        for(idx = 0; idx < expect_num_rows; ptr+=single_row_sz, idx++) { \
            expect_row[idx] = (db_query_result_t *)ptr; \
            db_query_row_info_t *rowinfo = (db_query_row_info_t *) &expect_row[idx]->data[0]; \
            rowinfo->num_cols = _num_cols; \
            char *data_start = &rowinfo->data[0]; \
            rowinfo->values = (char **)data_start; \
        } \
    }

#define  UTEST_ACL_LOAD_TEARDOWN \
    free(expect_row[0]);


#define  UTEST_EXPECT_NUM_ROWS  3
#define  UTEST_EXPECT_NUM_COLS  4
Ensure(app_acl_test__load_ok)
{
    UTEST_ACL_LOAD_SETUP(1, UTEST_EXPECT_NUM_ROWS, UTEST_EXPECT_NUM_COLS)
    const char *expect_row_data[UTEST_EXPECT_NUM_ROWS][UTEST_EXPECT_NUM_COLS] = {
            {"93804", "1", "0", "0"},  {"4095", "0", "0", "0"},  {"133847", "0", "1", "0"}  };
    aacl_data_t  expect_row_data_int[UTEST_EXPECT_NUM_ROWS] = {
        {.usr_id=93804,  .capability={.transcode=1, .renew=0, .edit_acl=0}},
        {.usr_id=4095,   .capability={.transcode=0, .renew=0, .edit_acl=0}},
        {.usr_id=133847, .capability={.transcode=0, .renew=1, .edit_acl=0}}
    };
    mock_acl_cfg.usrdata = (void *)&expect_row_data_int[0];
    expect(utest_dbpool__is_conn_closing, will_return(0), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbpool__acquire_free_conn, will_return(&mock_conn), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbconn__add_new_query, when(_conn, is_equal_to(&mock_conn)), when(q, is_not_equal_to(NULL)));
    expect(utest_dbpool__release_used_conn, will_return(DBA_RESULT_OK), when(_conn, is_equal_to(&mock_conn)));
    expect(utest_dbconn__try_process_queries, will_return(DBA_RESULT_OK),
                will_set_contents_of_parameter(exp_num_rows_p, &expect_num_rows, sizeof(uint8_t))  );
    for(idx = 0; idx < UTEST_EXPECT_NUM_ROWS; idx++) {
        db_query_row_info_t *rowinfo = (db_query_row_info_t *) &expect_row[idx]->data[0];
        for(uint8_t jdx = 0; jdx < UTEST_EXPECT_NUM_COLS; jdx++)
            rowinfo->values[jdx] = (char *) expect_row_data[idx][jdx];
        expect(utest_dbconn__try_process_queries, will_return(expect_row[idx]));
    } // end of loop
    expect(utest_acl__operation_done_cb, when(flg_err, is_equal_to(0)),  when(flg_wr_ok, is_equal_to(0)),
               when(actual_num_rows, is_equal_to(UTEST_EXPECT_NUM_ROWS)) );
    int err =  app_resource_acl_load(&mock_acl_cfg);
    assert_that(err, is_equal_to(0));
    UTEST_ACL_LOAD_TEARDOWN
} // end of app_acl_test__load_ok
#undef  UTEST_EXPECT_NUM_ROWS
#undef  UTEST_EXPECT_NUM_COLS


#define  UTEST_EXPECT_NUM_ROWS  1
#define  UTEST_EXPECT_NUM_COLS  4
Ensure(app_acl_test__load_error)
{
    UTEST_ACL_LOAD_SETUP(1, UTEST_EXPECT_NUM_ROWS, UTEST_EXPECT_NUM_COLS)
    uint8_t  expect_error = 1;
    const char *expect_row_data[UTEST_EXPECT_NUM_ROWS][UTEST_EXPECT_NUM_COLS] = {{"351", "0", "1", "1"}};
    expect(utest_dbpool__is_conn_closing, will_return(0), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbpool__acquire_free_conn, will_return(&mock_conn), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbconn__add_new_query, when(_conn, is_equal_to(&mock_conn)), when(q, is_not_equal_to(NULL)));
    expect(utest_dbpool__release_used_conn, will_return(DBA_RESULT_OK), when(_conn, is_equal_to(&mock_conn)));
    expect(utest_dbconn__try_process_queries, will_return(DBA_RESULT_OK),
                will_set_contents_of_parameter(exp_num_rows_p, &expect_num_rows, sizeof(uint8_t)),
                will_set_contents_of_parameter(expect_err_p, &expect_error, sizeof(uint8_t)),
          );
    for(idx = 0; idx < UTEST_EXPECT_NUM_ROWS; idx++) {
        db_query_row_info_t *rowinfo = (db_query_row_info_t *) &expect_row[idx]->data[0];
        for(uint8_t jdx = 0; jdx < UTEST_EXPECT_NUM_COLS; jdx++)
            rowinfo->values[jdx] = (char *) expect_row_data[idx][jdx];
        expect(utest_dbconn__try_process_queries, will_return(expect_row[idx]));
    } // end of loop
    expect(utest_acl__operation_done_cb, when(flg_err, is_equal_to(1)),  when(flg_wr_ok, is_equal_to(0)),
               when(actual_num_rows, is_equal_to(UTEST_EXPECT_NUM_ROWS)) );
    int err =  app_resource_acl_load(&mock_acl_cfg);
    assert_that(err, is_equal_to(0));
    UTEST_ACL_LOAD_TEARDOWN
} // end of app_acl_test__load_error
#undef  UTEST_EXPECT_NUM_ROWS
#undef  UTEST_EXPECT_NUM_COLS


Ensure(app_acl_test__build_update_list_1)
{
#define  UTEST_NUM_ITEMS_EXISTING  6
#define  UTEST_NUM_ITEMS_NEW       7
#define  UTEST_NEW_ITEM_RAWDATA   \
    "[{\"usr_id\":6178,\"access_control\":{\"transcode\":false,\"renew\":true, \"edit_acl\":true}}," \
     "{\"usr_id\":8190,\"access_control\":{\"transcode\":true, \"renew\":true, \"edit_acl\":true}}," \
     "{\"usr_id\":9384,\"access_control\":{\"transcode\":false,\"renew\":false,\"edit_acl\":true}}," \
     "{\"usr_id\":1103,\"access_control\":{\"transcode\":false,\"renew\":false,\"edit_acl\":true}}," \
     "{\"usr_id\":1615,\"access_control\":{\"transcode\":true, \"renew\":true, \"edit_acl\":false}}," \
     "{\"usr_id\":9204,\"access_control\":{\"transcode\":false,\"renew\":false,\"edit_acl\":false}}," \
     "{\"usr_id\":1885,\"access_control\":{\"transcode\":true, \"renew\":false,\"edit_acl\":false}}]"
    aacl_data_t mock_existing_data[UTEST_NUM_ITEMS_EXISTING] = {
        {.usr_id=9384,  .capability={.transcode=1, .renew=0, .edit_acl=0}},
        {.usr_id=3801,  .capability={.transcode=0, .renew=1, .edit_acl=0}},
        {.usr_id=8046,  .capability={.transcode=1, .renew=0, .edit_acl=1}},
        {.usr_id=416,   .capability={.transcode=1, .renew=1, .edit_acl=0}},
        {.usr_id=1615,  .capability={.transcode=0, .renew=1, .edit_acl=1}},
        {.usr_id=6178,  .capability={.transcode=1, .renew=1, .edit_acl=1}},
    };
#define  EXPECTED_NUM_UPDATE    3
#define  EXPECTED_NUM_DELETE    3
#define  EXPECTED_NUM_INSERT    4
    aacl_data_t  expect_data_update[EXPECTED_NUM_UPDATE] = {
        {.usr_id=6178,  .capability={.transcode=0, .renew=1, .edit_acl=1}},
        {.usr_id=9384,  .capability={.transcode=0, .renew=0, .edit_acl=1}},
        {.usr_id=1615,  .capability={.transcode=1, .renew=1, .edit_acl=0}},
    };
    aacl_data_t  expect_data_delete[EXPECTED_NUM_DELETE] = {
        {.usr_id=3801}, {.usr_id=8046}, {.usr_id=416},
    };
    aacl_data_t  expect_data_insert[EXPECTED_NUM_INSERT] = {
        {.usr_id=8190,  .capability={.transcode=1, .renew=1, .edit_acl=1}},
        {.usr_id=1103,  .capability={.transcode=0, .renew=0, .edit_acl=1}},
        {.usr_id=9204,  .capability={.transcode=0, .renew=0, .edit_acl=0}},
        {.usr_id=1885,  .capability={.transcode=1, .renew=0, .edit_acl=0}},
    };
    aacl_result_t  mock_saved_result = {.data={.entries=&mock_existing_data[0],
        .size=UTEST_NUM_ITEMS_EXISTING, .capacity=UTEST_NUM_ITEMS_EXISTING}};
    json_t *mock_new_data = json_loadb(UTEST_NEW_ITEM_RAWDATA, sizeof(UTEST_NEW_ITEM_RAWDATA) - 1, 0, NULL);
    assert_that(mock_new_data, is_not_null);
    aacl_data_t *actual_data_update[UTEST_NUM_ITEMS_EXISTING] = {0}, *actual_data_delete[UTEST_NUM_ITEMS_EXISTING] = {0},
                actual_data_insert[UTEST_NUM_ITEMS_NEW] = {0};
    size_t  actual_num_update = 0, actual_num_deletion = 0, actual_num_insertion = 0;
    app_acl__build_update_lists (&mock_saved_result, mock_new_data, &actual_data_update[0], &actual_num_update,
          &actual_data_delete[0], &actual_num_deletion, &actual_data_insert[0], &actual_num_insertion);
    assert_that(actual_num_update   , is_equal_to(EXPECTED_NUM_UPDATE));
    assert_that(actual_num_deletion , is_equal_to(EXPECTED_NUM_DELETE));
    assert_that(actual_num_insertion, is_equal_to(EXPECTED_NUM_INSERT));
#define  VERIFY_CODE(exp_len, actual_len, exp_data, actual_data, cap_flg_chk) \
{ \
    int idx = 0, jdx = 0; \
    for(idx = 0; idx < exp_len; idx++) { \
        aacl_data_t *expected = &exp_data[idx]; \
        int found = 0; \
        for(jdx = 0; (!found) && (jdx < actual_len); jdx++) { \
            aacl_data_t *actual = actual_data[jdx]; \
            if(actual->usr_id == expected->usr_id) { \
                if(cap_flg_chk) { \
                    assert_that(actual->capability.renew, is_equal_to(expected->capability.renew)); \
                    assert_that(actual->capability.transcode, is_equal_to(expected->capability.transcode)); \
                    assert_that(actual->capability.edit_acl, is_equal_to(expected->capability.edit_acl)); \
                } \
                found = 1; \
            } \
        } \
        assert_that(found, is_equal_to(1)); \
    } \
}
    VERIFY_CODE(EXPECTED_NUM_UPDATE, actual_num_update, expect_data_update, actual_data_update, 1)
    VERIFY_CODE(EXPECTED_NUM_INSERT, actual_num_insertion, expect_data_insert, &actual_data_insert, 1)
    VERIFY_CODE(EXPECTED_NUM_DELETE, actual_num_deletion, expect_data_delete, actual_data_delete, 0)
    json_decref(mock_new_data);
#undef  EXPECTED_NUM_UPDATE
#undef  EXPECTED_NUM_DELETE
#undef  EXPECTED_NUM_INSERT
#undef  UTEST_NEW_ITEM_RAWDATA
#undef  UTEST_NUM_EXISTING_ITEMS
#undef  UTEST_NUM_ITEMS_NEW
} // end of app_acl_test__build_update_list_1


Ensure(app_acl_test__build_update_list_2)
{
#define  UTEST_NUM_ITEMS_NEW       3
#define  UTEST_NEW_ITEM_RAWDATA   \
    "[{\"usr_id\":6178,\"access_control\":{\"transcode\":false,\"renew\":true, \"edit_acl\":true}}," \
     "{\"usr_id\":1615,\"access_control\":{\"transcode\":true, \"renew\":true, \"edit_acl\":false}}," \
     "{\"usr_id\":1885,\"access_control\":{\"transcode\":true, \"renew\":false,\"edit_acl\":false}}]"
#define  EXPECTED_NUM_INSERT    UTEST_NUM_ITEMS_NEW
    aacl_data_t  expect_data_insert[EXPECTED_NUM_INSERT] = {
        {.usr_id=6178,  .capability={.transcode=0, .renew=1, .edit_acl=1}},
        {.usr_id=1615,  .capability={.transcode=1, .renew=1, .edit_acl=0}},
        {.usr_id=1885,  .capability={.transcode=1, .renew=0, .edit_acl=0}},
    };
    aacl_result_t  mock_saved_result = {.data={0}};
    json_t *mock_new_data = json_loadb(UTEST_NEW_ITEM_RAWDATA, sizeof(UTEST_NEW_ITEM_RAWDATA) - 1, 0, NULL);
    assert_that(mock_new_data, is_not_null);
    aacl_data_t *actual_data_update[1] = {0}, *actual_data_delete[1] = {0}, actual_data_insert[UTEST_NUM_ITEMS_NEW] = {0};
    size_t  actual_num_update = 0, actual_num_deletion = 0, actual_num_insertion = 0;
    app_acl__build_update_lists (&mock_saved_result, mock_new_data, &actual_data_update[0], &actual_num_update,
          &actual_data_delete[0], &actual_num_deletion, &actual_data_insert[0], &actual_num_insertion);
    assert_that(actual_num_update   , is_equal_to(0));
    assert_that(actual_num_deletion , is_equal_to(0));
    assert_that(actual_num_insertion, is_equal_to(EXPECTED_NUM_INSERT));
    VERIFY_CODE(EXPECTED_NUM_INSERT, actual_num_insertion, expect_data_insert, &actual_data_insert, 1)
    json_decref(mock_new_data);
#undef  VERIFY_CODE
#undef  EXPECTED_NUM_INSERT
#undef  UTEST_NEW_ITEM_RAWDATA
#undef  UTEST_NUM_ITEMS_NEW
} // end of app_acl_test__build_update_list_2



#define  UTEST_ACL_SAVE_SETUP(rawsql_max_nkbytes, new_data_serialized) \
    UTEST_ACL_COMMON_SETUP(rawsql_max_nkbytes) \
    aacl_data_t mock_existing_data[2] = { \
        {.usr_id=395,  .capability={.transcode=1, .renew=0, .edit_acl=1}}, \
        {.usr_id=304,  .capability={.transcode=0, .renew=1, .edit_acl=0}}, \
    }; \
    aacl_result_t  mock_saved_result = {.data={.size=2, .capacity=2, .entries=&mock_existing_data[0]}}; \
    json_t *mock_new_data = json_loadb(new_data_serialized, sizeof(new_data_serialized) - 1, 0, NULL); \

#define  UTEST_ACL_SAVE_TEARDOWN \
    json_decref(mock_new_data);


Ensure(app_acl_test__save_ok)
{
#define  UTEST_NEW_ITEM_RAWDATA   \
    "[{\"usr_id\":1884,\"access_control\":{\"transcode\":false,\"renew\":true, \"edit_acl\":true}}," \
     "{\"usr_id\":395,\"access_control\":{\"transcode\":true, \"renew\":false,\"edit_acl\":false}}]"
    UTEST_ACL_SAVE_SETUP(5, UTEST_NEW_ITEM_RAWDATA)
    db_query_result_t  mock_q_result = {._final=1}, *mock_q_result_p = &mock_q_result;
    uint8_t  expect_write_flg = 1;
    expect(utest_dbpool__is_conn_closing, will_return(0), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbpool__acquire_free_conn, will_return(&mock_conn), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbconn__add_new_query, when(_conn, is_equal_to(&mock_conn)), when(q, is_not_equal_to(NULL)));
    expect(utest_dbpool__release_used_conn, will_return(DBA_RESULT_OK), when(_conn, is_equal_to(&mock_conn)));
    expect(utest_dbconn__try_process_queries, will_return(DBA_RESULT_OK),
            will_set_contents_of_parameter(expect_write_flg_p, &expect_write_flg, sizeof(uint8_t)),
            will_set_contents_of_parameter(q_result_p, &mock_q_result_p, sizeof(db_query_result_t *)),
          );
    expect(utest_acl__operation_done_cb, when(flg_err, is_equal_to(0)),  when(flg_wr_ok, is_equal_to(1)),
               when(actual_num_rows, is_equal_to(0)) );
    int err = app_resource_acl_save(&mock_acl_cfg, &mock_saved_result, mock_new_data);
    assert_that(err, is_equal_to(0));
    UTEST_ACL_SAVE_TEARDOWN
#undef  UTEST_NEW_ITEM_RAWDATA
} // end of app_acl_test__save_ok


Ensure(app_acl_test__save_error) 
{
#define  UTEST_NEW_ITEM_RAWDATA   \
    "[{\"usr_id\":1884,\"access_control\":{\"transcode\":false,\"renew\":true, \"edit_acl\":true}}," \
     "{\"usr_id\":395,\"access_control\":{\"transcode\":true, \"renew\":false,\"edit_acl\":false}}]"
    UTEST_ACL_SAVE_SETUP(5, UTEST_NEW_ITEM_RAWDATA)
    uint8_t  expect_write_flg = 1, expect_err_flg = 1;
    expect(utest_dbpool__is_conn_closing, will_return(0), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbpool__acquire_free_conn, will_return(&mock_conn), when(_pool, is_equal_to(&mock_db_pool)));
    expect(utest_dbconn__add_new_query, when(_conn, is_equal_to(&mock_conn)), when(q, is_not_equal_to(NULL)));
    expect(utest_dbpool__release_used_conn, will_return(DBA_RESULT_OK), when(_conn, is_equal_to(&mock_conn)));
    expect(utest_dbconn__try_process_queries, will_return(DBA_RESULT_OK),
            will_set_contents_of_parameter(expect_write_flg_p, &expect_write_flg, sizeof(uint8_t)),
            will_set_contents_of_parameter(expect_err_p, &expect_err_flg, sizeof(uint8_t)),
          );
    expect(utest_acl__operation_done_cb, when(flg_err, is_equal_to(1)),  when(flg_wr_ok, is_equal_to(0)),
               when(actual_num_rows, is_equal_to(0)) );
    int err = app_resource_acl_save(&mock_acl_cfg, &mock_saved_result, mock_new_data);
    assert_that(err, is_equal_to(0));
    UTEST_ACL_SAVE_TEARDOWN
#undef  UTEST_NEW_ITEM_RAWDATA
} // end of app_acl_test__save_error


TestSuite *app_resource_acl_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_acl_test__load_ok);
    add_test(suite, app_acl_test__load_error);
    add_test(suite, app_acl_test__build_update_list_1);
    add_test(suite, app_acl_test__build_update_list_2);
    add_test(suite, app_acl_test__save_ok);
    add_test(suite, app_acl_test__save_error);
    return suite;
}
