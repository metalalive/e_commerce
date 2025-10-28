#include <search.h>
#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "utils.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"
#include "../test/unit/transcoder/test.h"

#define UTEST_FILE_BASEPATH   "tmp/utest"
#define UTEST_ASASRC_BASEPATH UTEST_FILE_BASEPATH "/asasrc"

#define DONE_FLAG_INDEX__IN_ASA_USRARG (ATFP_INDEX__IN_ASA_USRARG + 1)
#define NUM_CB_ARGS_ASAOBJ             (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define RD_BUF_MAX_SZ                  512
#define MOCK_STORAGE_ALIAS             "persist_usr_asset"

#define MOCK_USER_ID                   426
#define MOCK_UPLD_REQ_1_ID             0xd150de7a
#define MOCK_ENCRYPTED_DOC_ID          "eb0yaWsirYt=="
#define MOCK_HOST_DOMAIN               "your.domain.com:443"
#define MOCK_REST_PATH                 "/utest/video/playback"
#define MOCK__QUERYPARAM_LABEL__RES_ID "ut_doc_id"
#define MOCK__QUERYPARAM_LABEL__DETAIL "ut_detail_keyword"

#define RUNNER_CREATE_FOLDER(fullpath) mkdir(fullpath, S_IRWXU)

#define HLS__BUILD_MST_PLIST_START__SETUP \
    uv_loop_t  *loop = uv_default_loop(); \
    json_t     *mock_spec = json_object(), *mock_err_info = json_object(); \
    app_cfg_t  *mock_appcfg = app_get_global_cfg(); \
    const char *sys_basepath = mock_appcfg->env_vars.sys_base_path; \
    asa_cfg_t   mock_src_storage_cfg = { \
          .alias = MOCK_STORAGE_ALIAS, \
          .base_path = PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, strdup), \
          .ops = \
            {.fn_scandir = app_storage_localfs_scandir, \
               .fn_scandir_next = app_storage_localfs_scandir_next, \
               .fn_close = app_storage_localfs_close, \
               .fn_typesize = app_storage_localfs_typesize} \
    }; \
    mock_appcfg->storages.size = 1; \
    mock_appcfg->storages.capacity = 1; \
    mock_appcfg->storages.entries = &mock_src_storage_cfg; \
    atfp_hls_t mock_fp = { \
        .super = \
            {.data = \
                 {.callback = _utest_hls_build_mst_plist_start__done_cb, \
                  .spec = mock_spec, \
                  .error = mock_err_info, \
                  .usr_id = MOCK_USER_ID, \
                  .upld_req_id = MOCK_UPLD_REQ_1_ID, \
                  .storage = {.handle = NULL}}}, \
        .internal = {.op = {.build_master_playlist = atfp_hls_stream__build_mst_plist}} \
    }; \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, RUNNER_CREATE_FOLDER); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, \
        UTEST_OPS_MKDIR \
    ); \
    json_object_set_new(mock_spec, "loop", json_integer((uint64_t)loop)); \
    json_object_set_new(mock_spec, "buf_max_sz", json_integer(RD_BUF_MAX_SZ)); \
    json_object_set_new(mock_spec, "storage_alias", json_string(MOCK_STORAGE_ALIAS));

#define HLS__BUILD_MST_PLIST_START__TEARDOWN \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, \
        UTEST_OPS_RMDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_RMDIR); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, rmdir); \
    { \
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle; \
        if (asa_src) \
            asa_src->deinit(asa_src); \
    }; \
    mock_appcfg->storages.size = 0; \
    mock_appcfg->storages.capacity = 0; \
    mock_appcfg->storages.entries = NULL; \
    free(mock_src_storage_cfg.base_path); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

static void _utest_hls_build_mst_plist_start__done_cb(atfp_t *processor) {
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    json_t            *err_info = processor->data.error;
    json_t            *spec = processor->data.spec;
    size_t             err_cnt = json_object_size(err_info);
    json_t            *expected_folder_names = json_object_get(spec, "_expected_folder_names");
    uint32_t           num_versions_found = 0, idx = 0;
    if (asa_src) {
        num_versions_found = asa_src->op.scandir.fileinfo.size;
        for (idx = 0; idx < num_versions_found; idx++) {
            asa_dirent_t *entry = &asa_src->op.scandir.fileinfo.data[idx];
            json_t       *item = json_object_get(expected_folder_names, entry->name);
            if (!item)
                continue;
            assert_that(entry->type, is_equal_to(ASA_DIRENT_DIR));
            json_object_del(expected_folder_names, entry->name);
        }
    }
    mock(asa_src, err_cnt, num_versions_found);
} // end of  _utest_hls_build_mst_plist_start__done_cb

#define UTEST_RESOURCE_VERSION_1 "pR4c" // currnet version size is 2 octets, this is to make valgrind happy
#define UTEST_RESOURCE_VERSION_2 "RsYs"
#define UTEST_RESOURCE_VERSION_3 "5aqM"
#define UTEST_UNRELATED_FOLDER_NAME \
    ATFP__COMMITTED_FOLDER_NAME "/" \
                                "will_be_excluded"
#define UTEST_UNRELATED_FILE_NAME \
    ATFP__COMMITTED_FOLDER_NAME "/" \
                                "will_also.be_cut"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
#define UTEST_RESOURCE_PATH_VERSION_2 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_2
#define UTEST_RESOURCE_PATH_VERSION_3 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_3
Ensure(atfp_hls_test__build_mst_plist__start_ok_1) {
    // master playlist in each version folder should provide
    // `bandwidth` attribute in `ext-x-stream-inf` tag
    HLS__BUILD_MST_PLIST_START__SETUP;
    {
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
            UTEST_OPS_MKDIR
        );
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
            UTEST_OPS_MKDIR
        );
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_UNRELATED_FOLDER_NAME,
            UTEST_OPS_MKDIR
        );
    }
    json_t *expected_folder_names = json_object();
    json_object_set_new(expected_folder_names, UTEST_RESOURCE_VERSION_1, json_true());
    json_object_set_new(expected_folder_names, UTEST_RESOURCE_VERSION_2, json_true());
    json_object_set_new(mock_spec, "_expected_folder_names", expected_folder_names);
#define _COMMON_CODE() \
    mock_fp.internal.op.build_master_playlist(&mock_fp); \
    size_t err_cnt = json_object_size(mock_err_info); \
    assert_that(err_cnt, is_equal_to(0)); \
    if (err_cnt == 0) { \
        expect( \
            _utest_hls_build_mst_plist_start__done_cb, when(asa_src, is_not_null), \
            when(err_cnt, is_equal_to(0)), \
            when(num_versions_found, is_greater_than(json_object_size(expected_folder_names))) \
        ); \
        uv_run(loop, UV_RUN_ONCE); \
        assert_that(json_object_size(expected_folder_names), is_equal_to(0)); \
        assert_that( \
            mock_fp.internal.op.build_master_playlist, \
            is_equal_to(atfp_hls_stream__build_mst_plist__continue) \
        ); \
        assert_that( \
            mock_fp.internal.op.build_master_playlist, is_not_equal_to(atfp_hls_stream__build_mst_plist) \
        ); \
    }
    _COMMON_CODE()
    HLS__BUILD_MST_PLIST_START__TEARDOWN
} // end of atfp_hls_test__build_mst_plist__start_ok_1

Ensure(atfp_hls_test__build_mst_plist__start_ok_2) {
    HLS__BUILD_MST_PLIST_START__SETUP { // refresh and new version is found, update master playlist
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_3,
            UTEST_OPS_MKDIR
        );
        const char *_wr_buf = "abcde";
        size_t      _wr_buf_sz = 5;
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_UNRELATED_FILE_NAME,
            UTEST_OPS_WRITE2FILE
        );
    }
    json_t *expected_folder_names = json_object();
    json_object_set_new(expected_folder_names, UTEST_RESOURCE_VERSION_1, json_true());
    json_object_set_new(expected_folder_names, UTEST_RESOURCE_VERSION_2, json_true());
    json_object_set_new(expected_folder_names, UTEST_RESOURCE_VERSION_3, json_true());
    json_object_set_new(mock_spec, "_expected_folder_names", expected_folder_names);
    _COMMON_CODE()
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_3,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_UNRELATED_FOLDER_NAME, UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_UNRELATED_FILE_NAME, UTEST_OPS_UNLINK
    );
    HLS__BUILD_MST_PLIST_START__TEARDOWN
} // end of  atfp_hls_test__build_mst_plist__start_ok_2
#undef _COMMON_CODE
#undef UTEST_RESOURCE_PATH_VERSION_3
#undef UTEST_RESOURCE_PATH_VERSION_2
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_RESOURCE_VERSION_3
#undef UTEST_RESOURCE_VERSION_2
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_UNRELATED_FOLDER_NAME
#undef UTEST_UNRELATED_FILE_NAME

Ensure(atfp_hls_test__build_mst_plist__start_nonexist) {
    HLS__BUILD_MST_PLIST_START__SETUP
    // assume there hasn't been version folder for transcoded video
    mock_fp.internal.op.build_master_playlist(&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if (err_cnt == 0) {
        expect(
            _utest_hls_build_mst_plist_start__done_cb, when(asa_src, is_not_null),
            when(err_cnt, is_greater_than(0)), when(num_versions_found, is_equal_to(0))
        );
        uv_run(loop, UV_RUN_ONCE);
        assert_that(mock_fp.internal.op.build_master_playlist, is_equal_to(atfp_hls_stream__build_mst_plist));
    }
    HLS__BUILD_MST_PLIST_START__TEARDOWN
} // end of  atfp_hls_test__build_mst_plist__start_nonexist

static __attribute__((optimize("O0"))) void _utest_hls_build_mst_plist_continue__done_cb(atfp_t *processor) {
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    json_t            *err_info = processor->data.error;
    size_t             err_cnt = json_object_size(err_info);
    char              *out_chunkbytes = processor->transfer.streaming_dst.block.data;
    size_t             out_chunkbytes_sz = processor->transfer.streaming_dst.block.len;
    uint8_t            is_final = processor->transfer.streaming_dst.flags.is_final;
    mock(asa_src, err_cnt, out_chunkbytes, out_chunkbytes_sz, is_final);
    uint8_t *done_flg_p = asa_src->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    if (done_flg_p)
        *done_flg_p = 1;
} // end of _utest_hls_build_mst_plist_continue__done_cb

// relative path concatenated by `MOCK_USER_ID`, `MOCK_UPLD_REQ_1_ID`
// and `ATFP__COMMITTED_FOLDER_NAME`
#define MOCK_SCANDIR_PATH "426/d150de7a/committed"

#define HLS__BUILD_MST_PLIST_CONTINUE__SETUP(_versions, _num_versions) \
    const char *sys_basepath = getenv("SYS_BASE_PATH"); \
    char        mock_rd_buf[RD_BUF_MAX_SZ] = {0}; \
    uint8_t     mock_done_flag = 0; \
    uv_loop_t  *loop = uv_default_loop(); \
    json_t     *mock_spec = json_object(); \
    json_t     *mock_err_info = json_object(); \
    asa_cfg_t   mock_src_storage_cfg = { \
          .alias = MOCK_STORAGE_ALIAS, \
          .base_path = PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, strdup), \
          .ops = \
            {.fn_open = app_storage_localfs_open, \
               .fn_close = app_storage_localfs_close, \
               .fn_read = app_storage_localfs_read} \
    }; \
    void                *mock_asa_src_cb_args[NUM_CB_ARGS_ASAOBJ]; \
    asa_op_localfs_cfg_t mock_asa_src = { \
        .loop = loop, \
        .super = \
            {.storage = &mock_src_storage_cfg, \
             .op = \
                 {.scandir = \
                      {.fileinfo = {.size = _num_versions, .rd_idx = 0, .data = _versions}, \
                       .path = MOCK_SCANDIR_PATH}, \
                  .read = {.dst_max_nbytes = RD_BUF_MAX_SZ, .dst = &mock_rd_buf[0]}}, \
             .cb_args = {.entries = mock_asa_src_cb_args, .size = NUM_CB_ARGS_ASAOBJ}} \
    }; \
    atfp_hls_t mock_fp = { \
        .super = \
            {.data = \
                 {.callback = _utest_hls_build_mst_plist_continue__done_cb, \
                  .spec = mock_spec, \
                  .error = mock_err_info, \
                  .usr_id = MOCK_USER_ID, \
                  .upld_req_id = MOCK_UPLD_REQ_1_ID, \
                  .storage = {.handle = &mock_asa_src.super}}}, \
        .internal = {.op = {.build_master_playlist = atfp_hls_stream__build_mst_plist__continue}} \
    }; \
    mock_asa_src_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp.super; \
    mock_asa_src_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &mock_done_flag; \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, RUNNER_CREATE_FOLDER); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, RUNNER_CREATE_FOLDER); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, \
        UTEST_OPS_MKDIR \
    ); \
    json_t *qp_labels = json_object(); \
    json_object_set_new(mock_spec, "host_domain", json_string(MOCK_HOST_DOMAIN)); \
    json_object_set_new(mock_spec, "host_path", json_string(MOCK_REST_PATH)); \
    json_object_set_new(mock_spec, "doc_id", json_string(MOCK_ENCRYPTED_DOC_ID)); \
    json_object_set_new(qp_labels, "doc_id", json_string(MOCK__QUERYPARAM_LABEL__RES_ID)); \
    json_object_set_new(qp_labels, "detail", json_string(MOCK__QUERYPARAM_LABEL__DETAIL)); \
    json_object_set_new(mock_spec, "query_param_label", qp_labels);

#define HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, \
        UTEST_OPS_RMDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH( \
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR \
    ); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_RMDIR); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_ASASRC_BASEPATH, rmdir); \
    PATH_CONCAT_THEN_RUN(sys_basepath, UTEST_FILE_BASEPATH, rmdir); \
    free(mock_src_storage_cfg.base_path); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

#define HLS_MST_PLIST_CONTENT_HEADER "#EXTM3U\n#EXT-X-VERSION:7\n"

#define UTEST_NUM_VERSIONS            2
#define UTEST_RESOURCE_VERSION_1      "Id"
#define UTEST_RESOURCE_VERSION_2      "De"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
#define UTEST_RESOURCE_PATH_VERSION_2 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_2
Ensure(atfp_hls_test__build_mst_plist__continue_ok_1) { // parse valid data from first playlist read
    asa_dirent_t mock_versions[UTEST_NUM_VERSIONS] = {
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_1},
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_2}
    };
    HLS__BUILD_MST_PLIST_CONTINUE__SETUP(&mock_versions[0], UTEST_NUM_VERSIONS);
    {
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
            UTEST_OPS_MKDIR
        );
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
            UTEST_OPS_MKDIR
        );
        const char *_wr_buf = NULL;
        size_t      _wr_buf_sz = 0;
#define TEST_WRITE_DATA \
    HLS_MST_PLIST_CONTENT_HEADER \
    "#EXT-X-STREAM-INF:BANDWIDTH=123456,RESOLUTION=160x120\n" HLS_PLAYLIST_FILENAME "\n\n"
        _wr_buf = TEST_WRITE_DATA, _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE
        );
#undef TEST_WRITE_DATA
#define TEST_WRITE_DATA \
    HLS_MST_PLIST_CONTENT_HEADER \
    "#EXT-X-STREAM-INF:BANDWIDTH=765432,RESOLUTION=189x320\n" HLS_PLAYLIST_FILENAME "\n\n"
        _wr_buf = TEST_WRITE_DATA, _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_2 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE
        );
#undef TEST_WRITE_DATA
    }
#define _COMMON_CODE(_expect_doc_id_str, _expect_detail_str, _expect_final) \
    mock_done_flag = 0; \
    mock_fp.internal.op.build_master_playlist(&mock_fp); \
    size_t err_cnt = json_object_size(mock_err_info); \
    assert_that(err_cnt, is_equal_to(0)); \
    if (err_cnt == 0) { \
        uint8_t     _num_plist_merged = mock_fp.internal.num_plist_merged; \
        const char *expect_data_beginwith = \
            (_num_plist_merged == 0) ? HLS_MST_PLIST_CONTENT_HEADER : "\n#EXT-X-STREAM-INF:"; \
        expect( \
            _utest_hls_build_mst_plist_continue__done_cb, when(out_chunkbytes_sz, is_greater_than(0)), \
            when(err_cnt, is_equal_to(0)), when(out_chunkbytes, contains_string(_expect_detail_str)), \
            when(out_chunkbytes, begins_with_string(expect_data_beginwith)), \
            when(out_chunkbytes, contains_string(MOCK_HOST_DOMAIN MOCK_REST_PATH)), \
            when(out_chunkbytes, contains_string(_expect_doc_id_str)), \
            when(is_final, is_equal_to(_expect_final)) \
        ); \
        while (!mock_done_flag) \
            uv_run(loop, UV_RUN_ONCE); \
        assert_that(json_object_size(mock_err_info), is_equal_to(0)); \
    }
    {
        const char *expect_doc_id = MOCK__QUERYPARAM_LABEL__RES_ID "=" MOCK_ENCRYPTED_DOC_ID;
        const char *expect_detail =
            MOCK__QUERYPARAM_LABEL__DETAIL "=" UTEST_RESOURCE_VERSION_1 "/" HLS_PLAYLIST_FILENAME;
        _COMMON_CODE(expect_doc_id, expect_detail, 0)
    }
    {
        const char *expect_doc_id = MOCK__QUERYPARAM_LABEL__RES_ID "=" MOCK_ENCRYPTED_DOC_ID;
        const char *expect_detail =
            MOCK__QUERYPARAM_LABEL__DETAIL "=" UTEST_RESOURCE_VERSION_2 "/" HLS_PLAYLIST_FILENAME;
        _COMMON_CODE(expect_doc_id, expect_detail, 1)
    }
    assert_that(mock_fp.internal.num_plist_merged, is_equal_to(2));
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        UTEST_RESOURCE_PATH_VERSION_2 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
        UTEST_OPS_RMDIR
    );
    HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN
} // end of   atfp_hls_test__build_mst_plist__continue_ok_1
#undef UTEST_NUM_VERSIONS
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_RESOURCE_VERSION_2
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_RESOURCE_PATH_VERSION_2

#define UTEST_NUM_VERSIONS            3
#define UTEST_RESOURCE_VERSION_1      "rL"
#define UTEST_RESOURCE_VERSION_2      "dB"
#define UTEST_RESOURCE_VERSION_3      "d7"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
#define UTEST_RESOURCE_PATH_VERSION_2 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_2
#define UTEST_RESOURCE_PATH_VERSION_3 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_3
Ensure(atfp_hls_test__build_mst_plist__continue_ok_2) { // parse valid data from second playlist read
    asa_dirent_t mock_versions[UTEST_NUM_VERSIONS] = {
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_1},
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_2},
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_3}
    };
    HLS__BUILD_MST_PLIST_CONTINUE__SETUP(&mock_versions[0], UTEST_NUM_VERSIONS);
    {
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
            UTEST_OPS_MKDIR
        );
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
            UTEST_OPS_MKDIR
        );
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_3,
            UTEST_OPS_MKDIR
        );
        const char *_wr_buf = NULL;
        size_t      _wr_buf_sz = 0;
#define TEST_WRITE_DATA \
    HLS_MST_PLIST_CONTENT_HEADER \
    "#EXT-X-STREAM-INF:BANDWIDTH=893807,RESOLUTION=760x390\n" HLS_PLAYLIST_FILENAME "\n\n"
        _wr_buf = TEST_WRITE_DATA, _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_3 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE
        );
#undef TEST_WRITE_DATA
    }
    {
        const char *expect_doc_id = MOCK__QUERYPARAM_LABEL__RES_ID "=" MOCK_ENCRYPTED_DOC_ID;
        const char *expect_detail =
            MOCK__QUERYPARAM_LABEL__DETAIL "=" UTEST_RESOURCE_VERSION_3 "/" HLS_PLAYLIST_FILENAME;
        _COMMON_CODE(expect_doc_id, expect_detail, 1)
    }
    { // end of scandir reached, no playlist will be loaded
        expect(
            _utest_hls_build_mst_plist_continue__done_cb, when(out_chunkbytes_sz, is_equal_to(0)),
            when(err_cnt, is_equal_to(0)), when(out_chunkbytes, is_null), when(is_final, is_equal_to(1))
        );
        mock_fp.internal.op.build_master_playlist(&mock_fp);
        assert_that(json_object_size(mock_err_info), is_equal_to(0));
    }
    assert_that(mock_fp.internal.num_plist_merged, is_equal_to(1));
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        UTEST_RESOURCE_PATH_VERSION_3 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_2,
        UTEST_OPS_RMDIR
    );
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_3,
        UTEST_OPS_RMDIR
    );
    HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN
} // end of   atfp_hls_test__build_mst_plist__continue_ok_2
#undef _COMMON_CODE
#undef UTEST_NUM_VERSIONS
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_RESOURCE_VERSION_2
#undef UTEST_RESOURCE_VERSION_3
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_RESOURCE_PATH_VERSION_2
#undef UTEST_RESOURCE_PATH_VERSION_3

#define UTEST_NUM_VERSIONS            2
#define UTEST_RESOURCE_VERSION_1      "Pt"
#define UTEST_RESOURCE_VERSION_2      "gr"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
#define UTEST_RESOURCE_PATH_VERSION_2 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_2
Ensure(atfp_hls_test__build_mst_plist__continue_end_of_scandir) {
    asa_dirent_t mock_versions[UTEST_NUM_VERSIONS] = {
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_1},
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_2}
    };
    HLS__BUILD_MST_PLIST_CONTINUE__SETUP(&mock_versions[0], UTEST_NUM_VERSIONS);
    mock_asa_src.super.op.scandir.fileinfo.rd_idx = UTEST_NUM_VERSIONS;
    expect(
        _utest_hls_build_mst_plist_continue__done_cb, when(out_chunkbytes_sz, is_equal_to(0)),
        when(err_cnt, is_equal_to(2)), when(out_chunkbytes, is_null), when(is_final, is_equal_to(1))
    );
    mock_fp.internal.op.build_master_playlist(&mock_fp);
    int err_resp_code = json_integer_value(json_object_get(mock_err_info, "_http_resp_code"));
    assert_that(err_resp_code, is_equal_to(404));
    HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN
} // end of  atfp_hls_test__build_mst_plist__continue_end_of_scandir
#undef UTEST_NUM_VERSIONS
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_RESOURCE_VERSION_2
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_RESOURCE_PATH_VERSION_2

#define UTEST_NUM_VERSIONS            1
#define UTEST_RESOURCE_VERSION_1      "pR"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
Ensure(atfp_hls_test__build_mst_plist__missing_file) {
    asa_dirent_t mock_versions[UTEST_NUM_VERSIONS] = {
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_1}
    };
    HLS__BUILD_MST_PLIST_CONTINUE__SETUP(&mock_versions[0], UTEST_NUM_VERSIONS);
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_MKDIR
    );
    { // missing playlist is possible if the video was transcoded in other format
        mock_fp.internal.op.build_master_playlist(&mock_fp);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if (err_cnt == 0) {
            expect(
                _utest_hls_build_mst_plist_continue__done_cb, when(out_chunkbytes, is_null),
                when(err_cnt, is_equal_to(2)), when(is_final, is_equal_to(1))
            );
            uv_run(loop, UV_RUN_ONCE);
            int err_resp_code = json_integer_value(json_object_get(mock_err_info, "_http_resp_code"));
            assert_that(err_resp_code, is_equal_to(404));
        }
    }
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_RMDIR
    );
    HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN
} // end of atfp_hls_test__build_mst_plist__missing_file
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_NUM_VERSIONS

#define UTEST_NUM_VERSIONS            1
#define UTEST_RESOURCE_VERSION_1      "Aj"
#define UTEST_RESOURCE_PATH_VERSION_1 ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
Ensure(atfp_hls_test__build_mst_plist__invalid_content) {
    asa_dirent_t mock_versions[UTEST_NUM_VERSIONS] = {
        {.type = ASA_DIRENT_DIR, .name = UTEST_RESOURCE_VERSION_1}
    };
    HLS__BUILD_MST_PLIST_CONTINUE__SETUP(&mock_versions[0], UTEST_NUM_VERSIONS);
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_MKDIR
    );
    {
        const char *_wr_buf = "unrelated content";
        size_t      _wr_buf_sz = sizeof("unrelated content");
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE
        );
        mock_fp.internal.op.build_master_playlist(&mock_fp);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if (err_cnt == 0) {
            expect(
                _utest_hls_build_mst_plist_continue__done_cb, when(out_chunkbytes, is_null),
                when(err_cnt, is_equal_to(2)), when(is_final, is_equal_to(1))
            );
            while (!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            int err_resp_code = json_integer_value(json_object_get(mock_err_info, "_http_resp_code"));
            assert_that(err_resp_code, is_equal_to(404));
        }
        UTEST_RUN_OPERATION_WITH_PATH(
            UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK
        );
    }
    UTEST_RUN_OPERATION_WITH_PATH(
        UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, UTEST_RESOURCE_PATH_VERSION_1,
        UTEST_OPS_RMDIR
    );
    HLS__BUILD_MST_PLIST_CONTINUE__TEARDOWN
} // end of atfp_hls_test__build_mst_plist__invalid_content
#undef UTEST_RESOURCE_VERSION_1
#undef UTEST_RESOURCE_PATH_VERSION_1
#undef UTEST_NUM_VERSIONS

TestSuite *app_transcoder_hls_stream_build_mst_plist_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__build_mst_plist__start_ok_1); // the 2 cases will run sequentially
    add_test(suite, atfp_hls_test__build_mst_plist__start_ok_2);
    add_test(suite, atfp_hls_test__build_mst_plist__start_nonexist);
    add_test(suite, atfp_hls_test__build_mst_plist__continue_ok_1);
    add_test(suite, atfp_hls_test__build_mst_plist__continue_ok_2);
    add_test(suite, atfp_hls_test__build_mst_plist__continue_end_of_scandir);
    add_test(suite, atfp_hls_test__build_mst_plist__missing_file);
    add_test(suite, atfp_hls_test__build_mst_plist__invalid_content);
    return suite;
}
