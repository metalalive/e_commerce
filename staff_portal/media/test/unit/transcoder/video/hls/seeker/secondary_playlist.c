#include <search.h>
#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"
#include "../test/unit/transcoder/video/hls/seeker/test.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASASRC_BASEPATH      UTEST_FILE_BASEPATH "/asasrc"
#define  UTEST_ASALOCAL_BASEPATH    UTEST_FILE_BASEPATH "/asalocal"

#define  DONE_FLAG_INDEX__IN_ASA_USRARG    (ATFP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ                (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define  MOCK_STORAGE_ALIAS    "localfs"

#define  MOCK_USER_ID           360
#define  MOCK_UPLD_REQ_1_ID     0x150de9a6
#define  MOCK_VERSION_STR       "OB"
#define  MOCK_ENCRYPTED_DOC_ID  "b0YL2y+asirW7t=="
#define  MOCK_HOST_DOMAIN       "your.domain.com:457"
#define  MOCK_REST_PATH         "/utest/video/playback"
#define  MOCK__QUERYPARAM_LABEL__RES_ID    "ut_doc_id"
#define  MOCK__QUERYPARAM_LABEL__DETAIL    "ut_detail_keyword"


static  __attribute__((optimize("O0")))  void _utest_hls_lvl2_plist__common_done_cb (atfp_t *processor)
{
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    json_t  *err_info = processor->data.error;
    size_t  err_cnt = json_object_size(err_info);
    char    * out_chunkbytes = processor->transfer.streaming_dst.block.data;
    size_t    out_chunkbytes_sz = processor->transfer.streaming_dst.block.len;
    uint8_t   is_final = processor->transfer.streaming_dst.flags.is_final;
    uint8_t   eof_reached  =  processor->transfer.streaming_dst.flags.eof_reached;
    mock(asa_src, err_cnt, out_chunkbytes, out_chunkbytes_sz, is_final, eof_reached);
    asa_op_base_cfg_t  *_asa_local = & ((atfp_hls_t *)processor) ->asa_local .super;
    if(_asa_local->cb_args.entries) {
        uint8_t  *done_flg_p = _asa_local->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flg_p)
            *done_flg_p = 1;
    }
} // end of _utest_hls_lvl2_plist__common_done_cb


#define  HLS__LVL2_PLIST_VALIDATE__SETUP \
    uint8_t mock_done_flag = 0 ; \
    void  *mock_asalocal_cb_args [NUM_CB_ARGS_ASAOBJ] = {NULL, &mock_done_flag}; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(); \
    json_t *mock_doc_metadata = json_object(); \
    json_t *mock_err_info = json_object(); \
    app_cfg_t *mock_appcfg = app_get_global_cfg(); \
    asa_cfg_t  mock_src_storage_cfg = {.alias=MOCK_STORAGE_ALIAS, .base_path=UTEST_ASASRC_BASEPATH, \
        .ops={ .fn_read=app_storage_localfs_read, .fn_open=app_storage_localfs_open, \
            .fn_close=app_storage_localfs_close, .fn_typesize=app_storage_localfs_typesize }}; \
    mock_appcfg->storages.size = 1; \
    mock_appcfg->storages.capacity = 1; \
    mock_appcfg->storages.entries = &mock_src_storage_cfg; \
    mock_appcfg->tmp_buf.path = UTEST_ASALOCAL_BASEPATH; \
    atfp_hls_t  mock_fp = {.super = {.data = {.callback=_utest_hls_lvl2_plist__common_done_cb, \
        .spec=mock_spec, .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID, \
        .version=MOCK_VERSION_STR, .storage={.handle=NULL}}}, \
        .asa_local={.super={.storage=&mock_src_storage_cfg, \
            .cb_args={.entries=mock_asalocal_cb_args, .size=NUM_CB_ARGS_ASAOBJ}}}, \
        .internal={.op={.build_secondary_playlist=atfp_hls_stream__build_lvl2_plist, \
            .get_crypto_key=atfp_get_crypto_key}} \
    }; \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    mkdir(UTEST_ASASRC_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH,   MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            ATFP__COMMITTED_FOLDER_NAME, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR, UTEST_OPS_MKDIR); \
    json_object_set_new(mock_spec, "loop", json_integer((uint64_t)loop)); \
    json_object_set_new(mock_spec, "buf_max_sz", json_integer(RD_BUF_MAX_SZ)); \
    json_object_set_new(mock_spec, "storage_alias", json_string(MOCK_STORAGE_ALIAS)); \
    json_object_set_new(mock_spec, "metadata", mock_doc_metadata);


#define  HLS__LVL2_PLIST_VALIDATE__TEARDOWN \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, \
            ATFP__COMMITTED_FOLDER_NAME, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH,   MOCK_USER_ID,  0, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,  0, NULL, UTEST_OPS_RMDIR); \
    rmdir(UTEST_ASASRC_BASEPATH); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH); \
    { \
        asa_op_base_cfg_t  *asa_src = mock_fp.super.data.storage.handle; \
        if(asa_src) { \
            asa_src->deinit(asa_src); \
            uv_run(loop, UV_RUN_ONCE); \
        } \
    }; \
    mock_appcfg->storages.size = 0; \
    mock_appcfg->storages.capacity = 0; \
    mock_appcfg->storages.entries = NULL; \
    mock_appcfg->tmp_buf.path =  NULL; \
    json_decref(mock_spec); \
    json_decref(mock_err_info);


#define   UTEST__MAX_TARGET_DURATION    "468"
#define   UTEST__CRYPTOKEY_CHOSEN_ID    "8134EADF"
#define   UTEST__PLIST_ORIGIN_CONTENT  "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:" \
    UTEST__MAX_TARGET_DURATION "\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n" \
    "#EXT-X-MAP:URI=\"/path/to/init_map\"\n#EXTINF:437.270567,\n/path/to/segment_a01.file"
#define   UTEST__CRYPTOKEY_MIN_CONTENT  \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \""UTEST__CRYPTOKEY_CHOSEN_ID"\":{\"iv\":{\"nbytes\":8,\"data\":\"5D4A38331751A3\"},\"alg\":\"aes\"}}"
#define   RD_BUF_MAX_SZ         (sizeof(UTEST__PLIST_ORIGIN_CONTENT) + 1)
Ensure(atfp_hls_test__l2_pl__validate_ok) {
    HLS__LVL2_PLIST_VALIDATE__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID)); \
    { // create (remote) media playlist, and  (local) crypto keyfile
        const char *_wr_buf = UTEST__PLIST_ORIGIN_CONTENT;
        size_t _wr_buf_sz = sizeof(UTEST__PLIST_ORIGIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE);
        _wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_WRITE2FILE);
    }
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        uint8_t  expect_eof_reached = (sizeof(UTEST__PLIST_ORIGIN_CONTENT) - 1) < RD_BUF_MAX_SZ;
        expect(_utest_hls_lvl2_plist__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_equal_to(0)),
                when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(expect_eof_reached)),  
                when(out_chunkbytes_sz, is_equal_to(0)), when(out_chunkbytes, is_null));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        asa_op_base_cfg_t *asa_src = mock_fp.super.data.storage.handle;
        assert_that(asa_src , is_not_null);
        if(asa_src)
            assert_that(asa_src->op.read.dst,  begins_with_string(UTEST__PLIST_ORIGIN_CONTENT));
        assert_that(mock_fp.internal.op.build_secondary_playlist , is_equal_to(atfp_hls_stream__lvl2_plist__parse_header));
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    HLS__LVL2_PLIST_VALIDATE__TEARDOWN
} // end of  atfp_hls_test__l2_pl__validate_ok
#undef  RD_BUF_MAX_SZ
#undef  UTEST__CRYPTOKEY_MIN_CONTENT
#undef  UTEST__PLIST_ORIGIN_CONTENT
#undef  UTEST__MAX_TARGET_DURATION
#undef  UTEST__CRYPTOKEY_CHOSEN_ID


#define   RD_BUF_MAX_SZ       64
Ensure(atfp_hls_test__l2_pl__validate_missing_plist) {
    HLS__LVL2_PLIST_VALIDATE__SETUP
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_lvl2_plist__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_greater_than(0)),
                when(is_final, is_equal_to(0)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_get(mock_err_info, "storage") , is_not_null);
    }
    HLS__LVL2_PLIST_VALIDATE__TEARDOWN
} // end of  atfp_hls_test__l2_pl__validate_missing_plist
#undef  RD_BUF_MAX_SZ


#define   UTEST__CRYPTOKEY_CHOSEN_ID    "12345678"
#define   UTEST__PLIST_ORIGIN_CONTENT  "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:468" \
    "\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"/path/to/init_map\"" \
    "\n#EXTINF:437.270567,\n/path/to/segment_a01.file"
#define   UTEST__CRYPTOKEY_MIN_CONTENT  \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \"F2239D48\":{\"iv\":{\"nbytes\":8,\"data\":\"5D4A38331751A390\"},\"alg\":\"aes\"}}"
#define   RD_BUF_MAX_SZ         (sizeof(UTEST__PLIST_ORIGIN_CONTENT) + 1)
Ensure(atfp_hls_test__l2_pl__validate_missing_key) {
    HLS__LVL2_PLIST_VALIDATE__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID)); \
    { // create (remote) media playlist, and  (local) crypto keyfile
        const char *_wr_buf = UTEST__PLIST_ORIGIN_CONTENT;
        size_t _wr_buf_sz = sizeof(UTEST__PLIST_ORIGIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE);
        _wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_WRITE2FILE);
    }
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_lvl2_plist__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_greater_than(0)),
                when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(1)),  
                when(out_chunkbytes_sz, is_equal_to(0)), when(out_chunkbytes, is_null));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_get(mock_err_info, "storage") , is_not_null);
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    HLS__LVL2_PLIST_VALIDATE__TEARDOWN
} // end of  atfp_hls_test__l2_pl__validate_missing_key
#undef  RD_BUF_MAX_SZ
#undef  UTEST__CRYPTOKEY_MIN_CONTENT
#undef  UTEST__PLIST_ORIGIN_CONTENT
#undef  UTEST__CRYPTOKEY_CHOSEN_ID


#define   UTEST__CRYPTOKEY_CHOSEN_ID    "8134EADF"
// missing  EXT-X-PLAYLIST-TYPE
#define   UTEST__PLIST_ORIGIN_CONTENT  "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:468" \
    "\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-MAP:URI=\"/path/to/init_map\"\n#EXTINF:437.270567,\n/path/to/segment_a01.file"
#define   UTEST__CRYPTOKEY_MIN_CONTENT  \
    "{\"73724A57\":{\"iv\":{\"nbytes\":8,\"data\":\"296F986F0B7531A9\"},\"alg\":\"aes\"}," \
    " \""UTEST__CRYPTOKEY_CHOSEN_ID"\":{\"iv\":{\"nbytes\":8,\"data\":\"5D4A38331751A3\"},\"alg\":\"aes\"}}"
#define   RD_BUF_MAX_SZ         (sizeof(UTEST__PLIST_ORIGIN_CONTENT) + 1)
Ensure(atfp_hls_test__l2_pl__validate_tag_error) {
    HLS__LVL2_PLIST_VALIDATE__SETUP
    json_object_set_new(mock_doc_metadata, "key_id", json_string(UTEST__CRYPTOKEY_CHOSEN_ID)); \
    { // create (remote) media playlist, and  (local) crypto keyfile
        const char *_wr_buf = UTEST__PLIST_ORIGIN_CONTENT;
        size_t _wr_buf_sz = sizeof(UTEST__PLIST_ORIGIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE);
        _wr_buf = UTEST__CRYPTOKEY_MIN_CONTENT;
        _wr_buf_sz = sizeof(UTEST__CRYPTOKEY_MIN_CONTENT) - 1;
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_WRITE2FILE);
    }
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(0));
    if(err_cnt == 0) {
        expect(_utest_hls_lvl2_plist__common_done_cb, when(asa_src, is_not_null), when(err_cnt, is_greater_than(0)),
                when(is_final, is_equal_to(0)), when(eof_reached, is_equal_to(1)),  
                when(out_chunkbytes_sz, is_equal_to(0)), when(out_chunkbytes, is_null));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_get(mock_err_info, "transcoder") , is_not_null);
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        ATFP__COMMITTED_FOLDER_NAME"/"MOCK_VERSION_STR"/"HLS_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
        HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    HLS__LVL2_PLIST_VALIDATE__TEARDOWN
} // end of  atfp_hls_test__l2_pl__validate_tag_error
#undef  RD_BUF_MAX_SZ
#undef  UTEST__CRYPTOKEY_MIN_CONTENT
#undef  UTEST__PLIST_ORIGIN_CONTENT
#undef  UTEST__CRYPTOKEY_CHOSEN_ID



#define  HLS__L2_PLIST_PARSE_HDR__SETUP  \
    json_t *mock_err_info = json_object(); \
    json_t *mock_spec = json_object(); \
    json_t *qp_labels = json_object(); \
    size_t  serial_keyitem_sz = sizeof(UTEST__CRYPTOKEY_ITEM) - 1; \
    json_t *crypto_keyitem = json_loadb(UTEST__CRYPTOKEY_ITEM, serial_keyitem_sz, 0, NULL); \
    json_object_set_new(mock_spec, "_crypto_key", crypto_keyitem); \
    json_object_set_new(mock_spec, API_QUERYPARAM_LABEL__RESOURCE_ID, json_string(MOCK_ENCRYPTED_DOC_ID)); \
    json_object_set_new(mock_spec, "host_domain", json_string(MOCK_HOST_DOMAIN)); \
    json_object_set_new(mock_spec, "host_path", json_string(MOCK_REST_PATH)); \
    json_object_set_new(qp_labels, "doc_id", json_string(API_QUERYPARAM_LABEL__RESOURCE_ID)); \
    json_object_set_new(qp_labels, "detail", json_string(API_QUERYPARAM_LABEL__DETAIL_ELEMENT)); \
    json_object_set_new(mock_spec, "query_param_label", qp_labels); \
    json_object_set_new(mock_spec, "wrbuf_max_sz", json_integer(RD_BUF_MAX_SZ)); \
    asa_op_base_cfg_t  mock_asa_src = {.op={.read={.dst_max_nbytes=RD_BUF_MAX_SZ, .dst=UTEST__PLIST_ORIGIN}}}; \
    atfp_hls_t  mock_fp = {.super = {.data = {.callback=_utest_hls_lvl2_plist__common_done_cb, \
        .spec=mock_spec, .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID, \
        .version=MOCK_VERSION_STR, .storage={.handle=&mock_asa_src}}}, \
        .internal={.op={.build_secondary_playlist=atfp_hls_stream__lvl2_plist__parse_header}} \
    };

#define  HLS__L2_PLIST_PARSE_HDR__TEARDOWN  \
    json_decref(mock_spec); \
    json_decref(mock_err_info); \
    if(mock_fp.super.transfer.streaming_dst.block.data) \
       free(mock_fp.super.transfer.streaming_dst.block.data);


#define   UTEST__IV_HEX  "296F986F0B7531A9"
#define   UTEST__CRYPTOKEY_ITEM   "{\"iv\":{\"nbytes\":8,\"data\":\""UTEST__IV_HEX"\"},\"alg\":\"aes\"}"
#define   UTEST__PLIST_TAG_KEY  "\n#EXT-X-KEY:METHOD=AES-64,URI=\"https://" MOCK_HOST_DOMAIN MOCK_REST_PATH \
    "?" API_QUERYPARAM_LABEL__RESOURCE_ID"="MOCK_ENCRYPTED_DOC_ID "&" API_QUERYPARAM_LABEL__DETAIL_ELEMENT \
    "=" HLS_REQ_KEYFILE_LABEL "\",IV=0x" UTEST__IV_HEX
#define   UTEST__INIT_MAP_URL  "https://" MOCK_HOST_DOMAIN MOCK_REST_PATH "?" API_QUERYPARAM_LABEL__RESOURCE_ID \
    "=" MOCK_ENCRYPTED_DOC_ID "&" API_QUERYPARAM_LABEL__DETAIL_ELEMENT "=" MOCK_VERSION_STR "/" HLS_FMP4_FILENAME
#define   UTEST__PLIST_TAG_MAP_PARSED    "\n#EXT-X-MAP:USR_ATTR=987,URI=\""UTEST__INIT_MAP_URL"\""
#define   UTEST__MAX_TARGET_DURATION  "8299"
#define   UTEST__PLIST_COMMON_HEADER  "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:" UTEST__MAX_TARGET_DURATION \
    "\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD"  
#define   UTEST__PLIST_ORIGIN   UTEST__PLIST_COMMON_HEADER"\n#EXT-X-MAP:USR_ATTR=987,URI=\""HLS_FMP4_FILENAME"\"\n#EXTINF:0.123"
#define   UTEST__PLIST_PARSED   UTEST__PLIST_COMMON_HEADER  UTEST__PLIST_TAG_KEY  UTEST__PLIST_TAG_MAP_PARSED

Ensure(atfp_hls_test__l2_pl__parse_header_ok)
{
#define   RD_BUF_MAX_SZ     (sizeof(UTEST__PLIST_PARSED) + 1)
    HLS__L2_PLIST_PARSE_HDR__SETUP
    expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
            when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_greater_than(0)),
            when(out_chunkbytes_sz, is_less_than(RD_BUF_MAX_SZ)),
            when(out_chunkbytes, begins_with_string(UTEST__PLIST_PARSED))
          );
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    assert_that(mock_fp.internal.op.build_secondary_playlist , is_equal_to(atfp_hls_stream__lvl2_plist__parse_extinf));
    assert_that(atfp_hls_lvl2pl__load_curr_rd_ptr(&mock_fp) , begins_with_string("\n#EXTINF:0.123"));
    HLS__L2_PLIST_PARSE_HDR__TEARDOWN
#undef  RD_BUF_MAX_SZ
} // end of  atfp_hls_test__l2_pl__parse_header_ok

Ensure(atfp_hls_test__l2_pl__parse_header_insufficient_buffer_1)
{
#define   RD_BUF_MAX_SZ     (sizeof(UTEST__PLIST_COMMON_HEADER) + 5)
    HLS__L2_PLIST_PARSE_HDR__SETUP
    expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_greater_than(0)),
            when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_equal_to(0)),
            when(out_chunkbytes, begins_with_string(UTEST__PLIST_COMMON_HEADER))
          );
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    HLS__L2_PLIST_PARSE_HDR__TEARDOWN
#undef  RD_BUF_MAX_SZ
} // end of  atfp_hls_test__l2_pl__parse_header_insufficient_buffer_1

Ensure(atfp_hls_test__l2_pl__parse_header_insufficient_buffer_2)
{
#define   RD_BUF_MAX_SZ     (sizeof(UTEST__PLIST_COMMON_HEADER  UTEST__PLIST_TAG_KEY  UTEST__INIT_MAP_URL) - 5)
    HLS__L2_PLIST_PARSE_HDR__SETUP
    expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_greater_than(0)),
            when(is_final, is_equal_to(0)), when(out_chunkbytes_sz, is_equal_to(0)),
            when(out_chunkbytes, begins_with_string(UTEST__PLIST_COMMON_HEADER))
          );
    mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    HLS__L2_PLIST_PARSE_HDR__TEARDOWN
#undef  RD_BUF_MAX_SZ
} // end of  atfp_hls_test__l2_pl__parse_header_insufficient_buffer_2

#undef  UTEST__PLIST_ORIGIN
#undef  UTEST__PLIST_PARSED
#undef  UTEST__PLIST_COMMON_HEADER
#undef  UTEST__MAX_TARGET_DURATION
#undef  UTEST__INIT_MAP_URL
#undef  UTEST__PLIST_TAG_MAP_PARSED
#undef  UTEST__PLIST_TAG_KEY
#undef  UTEST__CRYPTOKEY_ITEM
#undef  UTEST__IV_HEX





static ASA_RES_CODE utest_storage_read_fn (asa_op_base_cfg_t *asaobj)
{
    ASA_RES_CODE   evt_result =  ASTORAGE_RESULT_UNKNOWN_ERROR;
    ASA_RES_CODE  *evt_result_p = &evt_result;
    char    *read_dst_p = asaobj->op.read.dst;
    size_t   evt_nread = 0;
    size_t  *evt_nread_p = &evt_nread;
    ASA_RES_CODE  caller_result = mock(evt_result_p, read_dst_p, evt_nread_p, asaobj);
    asaobj->op.read.cb(asaobj, evt_result, evt_nread);
    return  caller_result;
}
#define  HLS__L2_PLIST_PARSE_EXTINF__SETUP  \
    json_t *mock_err_info = json_object(); \
    json_t *mock_spec = json_object(); \
    json_t *qp_labels = json_object(); \
    json_object_set_new(mock_spec, API_QUERYPARAM_LABEL__RESOURCE_ID, json_string(MOCK_ENCRYPTED_DOC_ID)); \
    json_object_set_new(mock_spec, "host_domain", json_string(MOCK_HOST_DOMAIN)); \
    json_object_set_new(mock_spec, "host_path", json_string(MOCK_REST_PATH)); \
    json_object_set_new(qp_labels, "doc_id", json_string(API_QUERYPARAM_LABEL__RESOURCE_ID)); \
    json_object_set_new(qp_labels, "detail", json_string(API_QUERYPARAM_LABEL__DETAIL_ELEMENT)); \
    json_object_set_new(mock_spec, "query_param_label", qp_labels); \
    json_object_set_new(mock_spec, "wrbuf_max_sz", json_integer(WR_BUF_MAX_SZ)); \
    void  *mock_asasrc_cb_args [NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_cfg_t  mock_src_storage_cfg = {.ops={ .fn_read=utest_storage_read_fn }}; \
    asa_op_base_cfg_t  mock_asa_src = {.storage=&mock_src_storage_cfg, .cb_args={ \
        .entries=mock_asasrc_cb_args, .size=NUM_CB_ARGS_ASAOBJ }}; \
    atfp_hls_t  mock_fp = {.super = {.data = {.callback=_utest_hls_lvl2_plist__common_done_cb, \
        .spec=mock_spec, .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID, \
        .version=MOCK_VERSION_STR, .storage={.handle=&mock_asa_src}}}, \
        .internal={.op={.build_secondary_playlist=atfp_hls_stream__lvl2_plist__parse_extinf}} \
    }; \
    mock_asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp; 

#define  HLS__L2_PLIST_PARSE_EXTINF__TEARDOWN  \
    json_decref(mock_spec); \
    json_decref(mock_err_info); 

#define   STRINGIFY_SEG_NUM(n) "000" #n
#define   UTEST_DATASEG_PREFIX_URL  "https://" MOCK_HOST_DOMAIN MOCK_REST_PATH "?" API_QUERYPARAM_LABEL__RESOURCE_ID \
    "=" MOCK_ENCRYPTED_DOC_ID "&" API_QUERYPARAM_LABEL__DETAIL_ELEMENT "=" MOCK_VERSION_STR "/" HLS_SEGMENT_FILENAME_PREFIX
#define   SINGLE_EXTINF_PARSED_SZ   (sizeof("\n#EXTINF:,\n"UTEST_DATASEG_PREFIX_URL STRINGIFY_SEG_NUM(9)) \
        + HLS_PLIST_TARGET_DURATION_MAX_BYTES)

Ensure(atfp_hls_test__l2_pl__parse_extinf_ok_1)
{
#define   UTEST__PLIST_ORIGIN_PART1  \
    "\n#EXTINF:12.27057,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(0) \
    "\n#EXTINF:27.10967,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(1) \
    "\n#EXTINF:10.956780,\n"HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(2) \
    "\n#EXTINF:24.040561,\n"HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(3) \
    "\n#EXTINF:9"
#define   UTEST__PLIST_ORIGIN_PART2 \
    ".70567,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(4) "xxxx" \
    "\n#EXTINF:2.510367,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(5) \
    "\n#EXTINF:0.910405,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(6) \
    "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx12345" // assume  segment file name is too long
#define   UTEST__PLIST_PARSED_PART1  \
    "\n#EXTINF:12.27057,\n" UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(0) \
    "\n#EXTINF:27.10967,\n" UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(1) \
    "\n#EXTINF:10.956780,\n"UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(2)
#define   UTEST__PLIST_PARSED_PART2  \
    "\n#EXTINF:24.040561,\n"UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(3)
#define   UTEST__PLIST_PARSED_PART3  \
    "\n#EXTINF:9.70567,\n" UTEST_DATASEG_PREFIX_URL   STRINGIFY_SEG_NUM(4) \
    "\n#EXTINF:2.510367,\n"UTEST_DATASEG_PREFIX_URL   STRINGIFY_SEG_NUM(5) \
    "\n#EXTINF:0.910405,\n"UTEST_DATASEG_PREFIX_URL   STRINGIFY_SEG_NUM(6) 
#define   WR_BUF_MAX_SZ    SINGLE_EXTINF_PARSED_SZ * 3 + 10
#define   RD_BUF_MAX_SZ    sizeof(UTEST__PLIST_ORIGIN_PART1)
    HLS__L2_PLIST_PARSE_EXTINF__SETUP
    char mock_wr_buf[WR_BUF_MAX_SZ] = {0},  mock_rd_buf[RD_BUF_MAX_SZ] = {0};
    mock_fp.super.transfer.streaming_dst.block.data = &mock_wr_buf[0];
    mock_asa_src.op.read.dst_max_nbytes = RD_BUF_MAX_SZ;
    mock_asa_src.op.read.dst = &mock_rd_buf[0];
    const char *init_rd_data = UTEST__PLIST_ORIGIN_PART1;
    memcpy(&mock_rd_buf[0], init_rd_data, RD_BUF_MAX_SZ - 1); // strncpy causes strange warning
    char *expect_unread_prev = NULL;
    { // subcase #1
        atfp_hls_lvl2pl__save_curr_rd_ptr(&mock_fp, mock_asa_src.op.read.dst);
        expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
                when(eof_reached, is_equal_to(0)), when(is_final, is_equal_to(0)),
                when(out_chunkbytes_sz, is_greater_than(0)),
                when(out_chunkbytes_sz, is_less_than(WR_BUF_MAX_SZ)),
                when(out_chunkbytes, begins_with_string(UTEST__PLIST_PARSED_PART1))
              );
        mock_fp.internal.op.build_secondary_playlist (&mock_fp);
        assert_that(atfp_hls_lvl2pl__load_curr_rd_ptr(&mock_fp) , begins_with_string(
               "\n"HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(2) "\n#EXTINF:24.040561,"));
    } { // subcase #2
        expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
                when(eof_reached, is_equal_to(0)), when(is_final, is_equal_to(0)),
                when(out_chunkbytes_sz, is_greater_than(0)),
                when(out_chunkbytes_sz, is_less_than(WR_BUF_MAX_SZ)),
                when(out_chunkbytes, begins_with_string(UTEST__PLIST_PARSED_PART2))
              );
        mock_fp.internal.op.build_secondary_playlist (&mock_fp);
        expect_unread_prev = "\n"HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(3) "\n#EXTINF:9";
        assert_that(atfp_hls_lvl2pl__load_curr_rd_ptr(&mock_fp), begins_with_string(expect_unread_prev));
    } { // subcase #3
        ASA_RES_CODE  expect_evt_result = ASTORAGE_RESULT_COMPLETE;
        size_t  expect_nread = mock_asa_src.op.read.dst_max_nbytes - 1 - strlen(expect_unread_prev);
        expect(utest_storage_read_fn,  will_return(ASTORAGE_RESULT_ACCEPT),
                will_set_contents_of_parameter(read_dst_p, UTEST__PLIST_ORIGIN_PART2, sizeof(char) * expect_nread),
                will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
                will_set_contents_of_parameter(evt_nread_p, &expect_nread, sizeof(size_t))
              );
        expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
                when(eof_reached, is_equal_to(0)), when(is_final, is_equal_to(0)),
                when(out_chunkbytes_sz, is_greater_than(0)),
                when(out_chunkbytes_sz, is_less_than(WR_BUF_MAX_SZ)),
                when(out_chunkbytes, begins_with_string(UTEST__PLIST_PARSED_PART3))
              );
        mock_fp.internal.op.build_secondary_playlist (&mock_fp);
        // assert_that(atfp_hls_lvl2pl__load_curr_rd_ptr(&mock_fp), begins_with_string("abcdfg"));
    } { // subcase #4
        ASA_RES_CODE  expect_evt_result = ASTORAGE_RESULT_COMPLETE;
        size_t  expect_nread = 10;
        expect(utest_storage_read_fn,  will_return(ASTORAGE_RESULT_ACCEPT),
                will_set_contents_of_parameter(read_dst_p, "xxxxx12345", sizeof(char) * expect_nread),
                will_set_contents_of_parameter(evt_result_p, &expect_evt_result, sizeof(ASA_RES_CODE)),
                will_set_contents_of_parameter(evt_nread_p, &expect_nread, sizeof(size_t))
              );
        expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
                when(eof_reached, is_equal_to(1)), when(is_final, is_equal_to(1)),
                when(out_chunkbytes_sz, is_equal_to(0))
              );
        mock_fp.internal.op.build_secondary_playlist (&mock_fp);
    }
    HLS__L2_PLIST_PARSE_EXTINF__TEARDOWN
#undef  WR_BUF_MAX_SZ
#undef  RD_BUF_MAX_SZ
#undef  UTEST__PLIST_ORIGIN_PART1
#undef  UTEST__PLIST_ORIGIN_PART2
#undef  UTEST__PLIST_PARSED_PART1
#undef  UTEST__PLIST_PARSED_PART2
#undef  UTEST__PLIST_PARSED_PART3
} // end of  atfp_hls_test__l2_pl__parse_extinf_ok_1


Ensure(atfp_hls_test__l2_pl__parse_extinf_error) {
#define   UTEST__PLIST_ORIGIN_PART1  \
    "\n#EXTINF:12.27057,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(0) \
    "\n#EXTINF:27.10967,\n" HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(1) \
    "\n#EXTINF:19."
#define   UTEST__PLIST_PARSED_PART1  \
    "\n#EXTINF:12.27057,\n" UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(0) \
    "\n#EXTINF:27.10967,\n" UTEST_DATASEG_PREFIX_URL  STRINGIFY_SEG_NUM(1)
#define   WR_BUF_MAX_SZ    SINGLE_EXTINF_PARSED_SZ * 2 + 30
#define   RD_BUF_MAX_SZ    sizeof(UTEST__PLIST_ORIGIN_PART1)
    HLS__L2_PLIST_PARSE_EXTINF__SETUP
    char mock_wr_buf[WR_BUF_MAX_SZ] = {0},  mock_rd_buf[RD_BUF_MAX_SZ] = {0};
    mock_fp.super.transfer.streaming_dst.block.data = &mock_wr_buf[0];
    mock_asa_src.op.read.dst_max_nbytes = RD_BUF_MAX_SZ;
    mock_asa_src.op.read.dst = &mock_rd_buf[0];
    memcpy(&mock_rd_buf[0], UTEST__PLIST_ORIGIN_PART1, RD_BUF_MAX_SZ - 1); // strncpy causes strange warning
    char *expect_unread_prev = "\n"HLS_SEGMENT_FILENAME_PREFIX  STRINGIFY_SEG_NUM(1) "\n#EXTINF:19.";
    { // subcase #1
        atfp_hls_lvl2pl__save_curr_rd_ptr(&mock_fp, mock_asa_src.op.read.dst);
        expect(_utest_hls_lvl2_plist__common_done_cb, when(err_cnt, is_equal_to(0)),
                when(eof_reached, is_equal_to(0)), when(is_final, is_equal_to(0)),
                when(out_chunkbytes_sz, is_greater_than(0)),
                when(out_chunkbytes_sz, is_less_than(WR_BUF_MAX_SZ)),
                when(out_chunkbytes, begins_with_string(UTEST__PLIST_PARSED_PART1))
              ); // CAUTION, string in begins_with_string() longer than actual buffer will cause undefined behaviour
        mock_fp.internal.op.build_secondary_playlist (&mock_fp);
        assert_that(atfp_hls_lvl2pl__load_curr_rd_ptr(&mock_fp) , begins_with_string(expect_unread_prev));
    } { // subcase #2
    }
    HLS__L2_PLIST_PARSE_EXTINF__TEARDOWN
#undef  WR_BUF_MAX_SZ
#undef  RD_BUF_MAX_SZ
#undef  UTEST__PLIST_ORIGIN_PART1
#undef  UTEST__PLIST_PARSED_PART1
} // end of  atfp_hls_test__l2_pl__parse_extinf_error

#undef  SINGLE_EXTINF_PARSED_SZ
#undef  UTEST_DATASEG_PREFIX_URL
#undef  STRINGIFY_SEG_NUM


TestSuite *app_transcoder_hls_stream_build_lvl2_plist_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__l2_pl__validate_ok);
    add_test(suite, atfp_hls_test__l2_pl__validate_missing_plist);
    add_test(suite, atfp_hls_test__l2_pl__validate_missing_key);
    add_test(suite, atfp_hls_test__l2_pl__validate_tag_error);
    add_test(suite, atfp_hls_test__l2_pl__parse_header_ok);
    add_test(suite, atfp_hls_test__l2_pl__parse_header_insufficient_buffer_1);
    add_test(suite, atfp_hls_test__l2_pl__parse_header_insufficient_buffer_2);
    add_test(suite, atfp_hls_test__l2_pl__parse_extinf_ok_1);
    add_test(suite, atfp_hls_test__l2_pl__parse_extinf_error);
    return suite;
}
