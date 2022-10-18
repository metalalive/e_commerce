#include <search.h>
#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASASRC_BASEPATH     UTEST_FILE_BASEPATH "/asasrc"
#define  UTEST_ASALOCAL_BASEPATH   UTEST_FILE_BASEPATH "/asalocal"

#define  DONE_FLAG_INDEX__IN_ASA_USRARG    (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ                (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)
#define  RD_BUF_MAX_SZ    512

#define  MOCK_USER_ID          426
#define  MOCK_UPLD_REQ_1_ID    0xd150de7a
#define  MOCK_RESOURCE_1_ID    "eb0y#aWs"
#define  MOCK_HOST_DOMAIN      "your.domain.com:443"
#define  MOCK_REST_PATH        "/utest/video/playback"
#define  MOCK__QUERYPARAM_LABEL__RES_ID    "ut_doc_id"
#define  MOCK__QUERYPARAM_LABEL__VERSION   "ut_doc_quallity"
#define  MOCK__QUERYPARAM_LABEL__DETAIL    "ut_detail_keyword"


#define  UTEST_RUN_OPERATION_WITH_PATH(_basepath, _usr_id, _upld_req_id, _filename, _cmd) \
{ \
    size_t  nwrite = 0, _fname_sz = 0; \
    char *__filename = _filename; \
    if(__filename != NULL) \
        _fname_sz = strlen(__filename); \
    size_t  path_sz = strlen(_basepath) + 1 + USR_ID_STR_SIZE + 1 + UPLOAD_INT2HEX_SIZE(_upld_req_id) \
            + 1 + _fname_sz + 1; \
    char path[path_sz]; \
    if(_usr_id!=0 && _upld_req_id!=0 && __filename) {  \
        nwrite = snprintf(&path[0], path_sz, "%s/%d/%x/%s", _basepath, \
                  _usr_id, _upld_req_id, __filename); \
    } else if(_usr_id!=0 && _upld_req_id!=0 && !__filename) { \
        nwrite = snprintf(&path[0], path_sz, "%s/%d/%x", _basepath, _usr_id, _upld_req_id); \
    } else if(_usr_id!=0 && _upld_req_id==0 && !__filename) { \
        nwrite = snprintf(&path[0], path_sz, "%s/%d", _basepath, _usr_id); \
    } \
    if(nwrite != 0) { \
        assert(path_sz >= nwrite); \
        _cmd(&path[0], nwrite) \
    } \
}

#define  UTEST_OPS_UNLINK(_path, _path_sz)    {unlink(_path);}
#define  UTEST_OPS_RMDIR(_path, _path_sz)     {rmdir(_path);}
#define  UTEST_OPS_MKDIR(_path, _path_sz)     {mkdir(_path,S_IRWXU);}
#define  UTEST_OPS_WRITE2FILE(_path, _path_sz) \
{ \
    int fd = open(_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    write(fd, _wr_buf, _wr_buf_sz); \
    close(fd); \
}

#define  UTEST_OPS_VERIFY_MST_PLIST(_path, _path_sz) \
    {_utest_ops_verify_mst_plist(_path, _expect_versions, _num_expect_versions);}

static void  _utest_ops_verify_mst_plist(const char *_path, const char **_expect_versions, int _num_expect_versions)
{
    struct hsearch_data  _htab = {0}; \
    hcreate_r((size_t)_num_expect_versions, &_htab);
    struct stat  statbuf = {0};
    int ret = stat(_path, &statbuf), dummy_num = 123;
    assert_that(ret, is_equal_to(0));
    if(!ret) {
        int idx = 0;
        for(idx = 0; idx < _num_expect_versions; idx++) {
            ENTRY  e = {.key=(char *)_expect_versions[idx], .data = (void*)&dummy_num };
            ENTRY *e_ret = NULL;
            ret = hsearch_r(e, ENTER, &e_ret, &_htab); \
        }
        int fd = open(_path, O_RDONLY, S_IRUSR);
        size_t  f_sz = statbuf.st_size + 1;
        char buf[f_sz], *buf_p = &buf[0];
        size_t  nread =  read(fd, (void *)buf_p, f_sz);
        buf[nread++] = 0x0;
        assert(f_sz == nread);
        for(idx = 0; (buf_p) && (idx < _num_expect_versions); idx++) {
            buf_p = strstr(buf_p, "\n#EXT-X-STREAM-INF:");
            assert_that(buf_p, is_not_null);   if(!buf_p) continue;
            buf_p += sizeof("\n#EXT-X-STREAM-INF:");
            buf_p  = strstr(buf_p, MOCK_HOST_DOMAIN  MOCK_REST_PATH);
            assert_that(buf_p, is_not_null);   if(!buf_p) continue;
            buf_p += sizeof(MOCK_HOST_DOMAIN  MOCK_REST_PATH);
            buf_p  = strstr(buf_p, MOCK__QUERYPARAM_LABEL__RES_ID "=" MOCK_RESOURCE_1_ID);
            assert_that(buf_p, is_not_null);   if(!buf_p) continue;
            buf_p  = strstr(buf_p, MOCK__QUERYPARAM_LABEL__VERSION "=");
            assert_that(buf_p, is_not_null);   if(!buf_p) continue;
            buf_p += sizeof(MOCK__QUERYPARAM_LABEL__VERSION "=") - 1;
            char *_actual_version = strndup(buf_p, APP_TRANSCODED_VERSION_SIZE);
            ENTRY  e = {.key=_actual_version, .data=NULL};  ENTRY *e_ret = NULL;
            hsearch_r(e, FIND, &e_ret, &_htab);
            assert_that(e_ret, is_not_null);
            if(e_ret)
                assert_that(e_ret->data, is_equal_to(&dummy_num));
            free(_actual_version);
        }
        close(fd);
    }
    hdestroy_r(&_htab);
} // end of _utest_ops_verify_mst_plist


static void _utest_hls_init_stream__done_cb(atfp_t *processor)
{
    json_t  *return_data_obj = NULL;
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    json_t  *err_info = processor->data.error;
    json_t  *spec = processor->data.spec;
    uint8_t  num_err_item = json_object_size(err_info);
    const char *actual_url = NULL;
    if(num_err_item == 0) {
        return_data_obj = json_object_get(spec, "return_data");
        assert_that(return_data_obj, is_not_equal_to(NULL));
        if(return_data_obj) {
            const char *actual_typ = json_string_value(json_object_get(return_data_obj, "type"));
            assert_that(actual_typ, is_equal_to_string("hls"));
            actual_url = json_string_value(json_object_get(return_data_obj, "entry"));
        }
    }
    int actual_resp_status = (int) json_integer_value(json_object_get(spec, "http_resp_code"));
    mock(processor, actual_resp_status, actual_url);
    uint8_t  *done_flg_p = asa_src->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    if(done_flg_p)
        *done_flg_p = 1;
} // end of  _utest_hls_init_stream__done_cb

#define  ATFP_HLS_TEST__INIT_STREAM__SETUP \
    char mock_rd_buf[RD_BUF_MAX_SZ] = {0}; \
    uint8_t mock_done_flag = 0 ; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(); \
    json_t *mock_err_info = json_object(); \
    void  *mock_asa_src_cb_args [NUM_CB_ARGS_ASAOBJ]; \
    app_cfg_t *mock_appcfg = app_get_global_cfg(); \
    mock_appcfg->tmp_buf.path = UTEST_ASALOCAL_BASEPATH; \
    asa_cfg_t  mock_src_storage_cfg = {.base_path=UTEST_ASASRC_BASEPATH, .ops={ \
        .fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close, \
        .fn_read=app_storage_localfs_read, .fn_scandir=app_storage_localfs_scandir, \
        .fn_scandir_next=app_storage_localfs_scandir_next}}; \
    asa_cfg_t  mock_local_storage_cfg = {.base_path=NULL, .ops={.fn_mkdir=app_storage_localfs_mkdir, \
        .fn_open=app_storage_localfs_open,.fn_close=app_storage_localfs_close, \
        .fn_write=app_storage_localfs_write}}; \
    asa_op_localfs_cfg_t  *mock_asa_src = calloc(1, sizeof(asa_op_localfs_cfg_t)); \
    *mock_asa_src = (asa_op_localfs_cfg_t) {.loop=loop, .super={.deinit=(void (*)(asa_op_base_cfg_t *))free, \
        .storage=&mock_src_storage_cfg, .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=mock_asa_src_cb_args}, \
        .op={.read={.dst_max_nbytes=RD_BUF_MAX_SZ, .dst=&mock_rd_buf[0]}}}}; \
    atfp_hls_t *mock_fp = (atfp_hls_t *)atfp__video_hls__instantiate(); \
    mock_fp->super.data = (atfp_data_t) {.callback=_utest_hls_init_stream__done_cb, .spec=mock_spec, \
        .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID, \
        .storage={.handle=&mock_asa_src->super}}; \
    mock_fp->asa_local = (asa_op_localfs_cfg_t) {.super={.storage=&mock_local_storage_cfg}} ; \
    mock_asa_src_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp->super; \
    mock_asa_src_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &mock_done_flag; \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    mkdir(UTEST_ASASRC_BASEPATH, S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            0, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, UTEST_OPS_MKDIR);


#define  ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP { \
    json_t *mock_hostinfo = json_object(),  *mock_qp_labels = json_object(), \
           *mock_update_interval = json_object(); \
    json_object_set_new(mock_hostinfo, "domain", json_string(MOCK_HOST_DOMAIN)); \
    json_object_set_new(mock_hostinfo, "path",   json_string(MOCK_REST_PATH)); \
    json_object_set_new(mock_qp_labels, "resource_id", json_string(MOCK__QUERYPARAM_LABEL__RES_ID)); \
    json_object_set_new(mock_qp_labels, "version", json_string(MOCK__QUERYPARAM_LABEL__VERSION)); \
    json_object_set_new(mock_qp_labels, "detail",  json_string(MOCK__QUERYPARAM_LABEL__DETAIL)); \
    json_object_set_new(mock_update_interval, "playlist",  json_real(MOCK_UPDATE_SECS_PLAYLIST)); \
    json_object_set_new(mock_update_interval, "keyfile",   json_real(MOCK_UPDATE_SECS_KEYFILE)); \
    json_object_set_new(mock_spec, "host", mock_hostinfo); \
    json_object_set_new(mock_spec, "query_param_label", mock_qp_labels); \
    json_object_set_new(mock_spec, "update_interval", mock_update_interval); \
    json_object_set_new(mock_spec, "loop", json_integer((uint64_t)loop)); \
    json_object_set_new(mock_spec, "id", json_string(MOCK_RESOURCE_1_ID)); \
}


#define  ATFP_HLS_TEST__INIT_STREAM__TEARDOWN \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, \
            0, NULL, UTEST_OPS_RMDIR); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, ATFP__COMMITTED_FOLDER_NAME, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_RMDIR); \
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, \
            0, NULL, UTEST_OPS_RMDIR); \
    rmdir(UTEST_ASASRC_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);


#define  MOCK_UPDATE_SECS_PLAYLIST   3.f
#define  MOCK_UPDATE_SECS_KEYFILE    3.f
#define  UTEST_RESOURCE_VERSION_1   "pR"
#define  UTEST_RESOURCE_VERSION_2   "Rs"
#define  UTEST_RESOURCE_VERSION_3   "5a"
#define  UTEST_RESOURCE_PATH_VERSION_1   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
#define  UTEST_RESOURCE_PATH_VERSION_2   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_2
#define  UTEST_RESOURCE_PATH_VERSION_3   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_3
Ensure(atfp_hls_test__init_stream__update_ok_1) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    { // master playlist in each version folder should provide `bandwidth` attribute in `ext-x-stream-inf` tag
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_MKDIR );
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_2, UTEST_OPS_MKDIR );
        const char *_wr_buf = NULL; size_t _wr_buf_sz = 0;
#define  TEST_WRITE_DATA   "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-STREAM-INF:BANDWIDTH=123456,RESOLUTION=160x120\nlvl2_plist.m3u8\n\n"
        _wr_buf = TEST_WRITE_DATA,  _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE );
#undef   TEST_WRITE_DATA
#define  TEST_WRITE_DATA   "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-STREAM-INF:BANDWIDTH=234567,RESOLUTION=179x131\nlvl2_plist.m3u8\n\n"
        _wr_buf = TEST_WRITE_DATA,  _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_2 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE );
#undef   TEST_WRITE_DATA
    } { // collect `ext-x-stream-inf` tag from each version folder, create master playlist
#define  _COMMON_CODE(key_id_hex) \
        atfp__video_hls__init_stream(&mock_fp->super); \
        size_t err_cnt = json_object_size(mock_err_info); \
        assert_that(err_cnt, is_equal_to(0)); \
        if(err_cnt == 0) { \
            int expect_resp_status = 200; \
            int mock_bignum = 0; \
            expect(BN_new,  will_return(&mock_bignum)); \
            expect(BN_rand, will_return(1), when(bits, is_equal_to(128))); \
            expect(BN_rand, will_return(1), when(bits, is_equal_to(128))); \
            expect(BN_rand, will_return(1), when(bits, is_equal_to(32))); \
            expect(BN_bn2hex, will_return(strdup("1d2a07b4836c998e"  "2a07b4836c939a08")), when(a, is_equal_to(&mock_bignum))); \
            expect(BN_bn2hex, will_return(strdup("e1d2a07b48360c99"  "1d2a07b48361c909")), when(a, is_equal_to(&mock_bignum))); \
            expect(BN_bn2hex, will_return(strdup(key_id_hex)), when(a, is_equal_to(&mock_bignum))); \
            expect(BN_free,  when(a, is_equal_to(&mock_bignum))); \
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(expect_resp_status)), \
                    when(actual_url, contains_string(MOCK_HOST_DOMAIN  MOCK_REST_PATH)), \
                    when(actual_url, contains_string("id="  MOCK_RESOURCE_1_ID)) ); \
            while(!mock_done_flag) \
                uv_run(loop, UV_RUN_ONCE); \
            assert_that(err_cnt, is_equal_to(0)); \
            UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, \
                    MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_VERIFY_MST_PLIST); \
        }
        int _num_expect_versions = 2;
        const char *_expect_versions[2] = {UTEST_RESOURCE_VERSION_1, UTEST_RESOURCE_VERSION_2};
        _COMMON_CODE("908e3873")
    }
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__update_ok_1


Ensure(atfp_hls_test__init_stream__update_interval_too_short) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    atfp__video_hls__init_stream(&mock_fp->super);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(1));
    int actual_resp_status = (int) json_integer_value(json_object_get(mock_spec, "http_resp_code"));
    assert_that(actual_resp_status, is_equal_to(429));
    const char *actual_err_msg = json_string_value(json_object_get(mock_err_info, "transcoder"));
    assert_that(actual_err_msg, contains_string("interval too short"));
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__update_interval_too_short


Ensure(atfp_hls_test__init_stream__update_ok_2) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    { // refresh and new version is found, update master playlist
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_3, UTEST_OPS_MKDIR );
        const char *_wr_buf = NULL; size_t _wr_buf_sz = 0;
#define  TEST_WRITE_DATA   "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-STREAM-INF:BANDWIDTH=345678,RESOLUTION=201x156\nlvl2_plist.m3u8\n\n"
        _wr_buf = TEST_WRITE_DATA,  _wr_buf_sz = sizeof(TEST_WRITE_DATA);
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_3 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE );
#undef   TEST_WRITE_DATA
        sleep(MOCK_UPDATE_SECS_PLAYLIST);
        int _num_expect_versions = 3;
        const char *_expect_versions[3] = {UTEST_RESOURCE_VERSION_1, UTEST_RESOURCE_VERSION_2, UTEST_RESOURCE_VERSION_3};
        _COMMON_CODE("29b8e387")
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_2 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_3 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_RMDIR );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             UTEST_RESOURCE_PATH_VERSION_2, UTEST_OPS_RMDIR );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             UTEST_RESOURCE_PATH_VERSION_3, UTEST_OPS_RMDIR );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of  atfp_hls_test__init_stream__update_ok_2
#undef  _COMMON_CODE
#undef  UTEST_RESOURCE_PATH_VERSION_3
#undef  UTEST_RESOURCE_PATH_VERSION_2
#undef  UTEST_RESOURCE_PATH_VERSION_1
#undef  UTEST_RESOURCE_VERSION_3
#undef  UTEST_RESOURCE_VERSION_2
#undef  UTEST_RESOURCE_VERSION_1
#undef  MOCK_UPDATE_SECS_PLAYLIST
#undef  MOCK_UPDATE_SECS_KEYFILE


Ensure(atfp_hls_test__init_stream__args_error) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    atfp__video_hls__init_stream(&mock_fp->super);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(1));
    int actual_resp_status = (int) json_integer_value(json_object_get(mock_spec, "http_resp_code"));
    assert_that(actual_resp_status, is_equal_to(400));
    const char *actual_err_msg = json_string_value(json_object_get(mock_err_info, "transcoder"));
    assert_that(actual_err_msg, contains_string("missing arguments in spec"));
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__args_error




#define  MOCK_CRYPTO_KEYFILE_CONTENT   "{\"EC3C81D4\":{\"key\":{\"nbytes\":16,\"data\":\"E614DC5A6252E79017D755FC982FB6DF\"},\"iv\":{\"nbytes\":16,\"data\":\"7245BC4205C6F4119583D25855D2BB01\"},\"alg\":\"aes\",\"timestamp\":0}}"
#define  UTEST_OPS_CREATE_KEYFILE(_path, _path_sz) { \
    int fd2 = open(_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    json_t *keyinfo = json_loadb(MOCK_CRYPTO_KEYFILE_CONTENT, sizeof(MOCK_CRYPTO_KEYFILE_CONTENT) - 1, 0, NULL); \
    json_t *keyitem = json_object_get(keyinfo, "EC3C81D4"); \
    json_object_deln(keyitem, "timestamp", 9); \
    json_object_set_new(keyitem, "timestamp", json_integer(time(NULL))); \
    json_dumpfd(keyinfo, fd2, 0); \
    json_decref(keyinfo); \
    close(fd2); \
}

#define  MOCK_UPDATE_SECS_PLAYLIST    1.0f
#define  MOCK_UPDATE_SECS_KEYFILE    10.0f
Ensure(atfp_hls_test__init_stream__dst_plist_lock_fail) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR);
    int fd = -1;
#define  UTEST_OPS_CREATE_PLIST(_path, _path_sz) { \
    fd = open(_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR); \
    write(fd, (void *)"abc", 3); \
    close(fd); \
    fd = open(_path, O_WRONLY, S_IRUSR | S_IWUSR); \
    flock(fd, LOCK_EX | LOCK_NB); \
}
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_CREATE_PLIST );
    sleep(MOCK_UPDATE_SECS_PLAYLIST + 2.0f);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_CREATE_KEYFILE );
    {
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) {
            int expect_resp_status = 200;
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(expect_resp_status)), 
                    when(actual_url, contains_string(MOCK_HOST_DOMAIN  MOCK_REST_PATH)), 
                    when(actual_url, contains_string("id="  MOCK_RESOURCE_1_ID)) ); 
            while(!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            assert_that(json_object_size(mock_err_info), is_equal_to(0));
        }
    }
    flock(fd, LOCK_UN | LOCK_NB);
    close(fd);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
#undef  UTEST_OPS_CREATE_N_LOCK
} // end of atfp_hls_test__init_stream__dst_plist_lock_fail
#undef  MOCK_UPDATE_SECS_PLAYLIST
#undef  MOCK_UPDATE_SECS_KEYFILE


#define  MOCK_UPLD_REQ_NONEXIST_ID    0x192038f4
#define  MOCK_UPDATE_SECS_PLAYLIST    1.0f
#define  MOCK_UPDATE_SECS_KEYFILE    10.0f
Ensure(atfp_hls_test__init_stream__src_scandir_missing) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    mock_fp->super.data.upld_req_id = MOCK_UPLD_REQ_NONEXIST_ID;
    {
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) {
            int expect_resp_status = 400;
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(expect_resp_status))); 
            while(!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            const char *actual_errmsg = json_string_value(json_object_get(mock_err_info, "storage"));
            assert_that(json_object_size(mock_err_info), is_equal_to(1));
            assert_that(actual_errmsg, contains_string("unknown source path"));
        }
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_NONEXIST_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 
            MOCK_UPLD_REQ_NONEXIST_ID, NULL, UTEST_OPS_RMDIR);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__src_scandir_missing
#undef  MOCK_UPDATE_SECS_PLAYLIST
#undef  MOCK_UPDATE_SECS_KEYFILE
#undef  MOCK_UPLD_REQ_NONEXIST_ID


#define  MOCK_UPDATE_SECS_PLAYLIST    1.0f
#define  MOCK_UPDATE_SECS_KEYFILE    10.0f
#define  UTEST_RESOURCE_VERSION_1   "pR"
#define  UTEST_RESOURCE_PATH_VERSION_1   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
Ensure(atfp_hls_test__init_stream__src_plist_missing) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_CREATE_KEYFILE );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_MKDIR );
    {
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) { // missing playlist is possible if the video was transcoded in other format
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(404))); 
            while(!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            const char *actual_errmsg = json_string_value(json_object_get(mock_err_info, "storage"));
            assert_that(json_object_size(mock_err_info), is_equal_to(1));
            assert_that(actual_errmsg, contains_string("not found"));
        }
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_RMDIR );
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__src_plist_missing
#undef  UTEST_RESOURCE_VERSION_1
#undef  UTEST_RESOURCE_PATH_VERSION_1
#undef  MOCK_UPDATE_SECS_KEYFILE
#undef  MOCK_UPLD_REQ_NONEXIST_ID


#define  MOCK_UPDATE_SECS_PLAYLIST    1.0f
#define  MOCK_UPDATE_SECS_KEYFILE    10.0f
#define  UTEST_RESOURCE_VERSION_1   "Aj"
#define  UTEST_RESOURCE_PATH_VERSION_1   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
Ensure(atfp_hls_test__init_stream__src_plist_corrupted) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, 0, NULL, UTEST_OPS_MKDIR);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID, NULL, UTEST_OPS_MKDIR);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
             HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_CREATE_KEYFILE );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_MKDIR );
    {
        const char *_wr_buf = "nothing"; size_t _wr_buf_sz =  sizeof("nothing");
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE);
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) { // a file with the same name as playlist, but different content, should be ignored
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(404))); 
            while(!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            const char *actual_errmsg = json_string_value(json_object_get(mock_err_info, "storage"));
            assert_that(json_object_size(mock_err_info), is_equal_to(1));
            assert_that(actual_errmsg, contains_string("not found"));
        }
        UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
                UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_RMDIR );
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__src_plist_corrupted
#undef  UTEST_RESOURCE_VERSION_1
#undef  UTEST_RESOURCE_PATH_VERSION_1
#undef  MOCK_UPDATE_SECS_KEYFILE
#undef  MOCK_UPDATE_SECS_PLAYLIST
#undef  MOCK_UPLD_REQ_NONEXIST_ID


#define  MOCK_UPDATE_SECS_PLAYLIST   1.f
#define  MOCK_UPDATE_SECS_KEYFILE    1.f
#define  UTEST_RESOURCE_VERSION_1   "k0"
#define  UTEST_RESOURCE_PATH_VERSION_1   ATFP__COMMITTED_FOLDER_NAME "/" UTEST_RESOURCE_VERSION_1
Ensure(atfp_hls_test__init_stream__key_rotate_fail) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
           UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_MKDIR );
#define  TEST_WRITE_DATA   "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-STREAM-INF:BANDWIDTH=123456,RESOLUTION=160x120\nlvl2_plist.m3u8\n\n"
    const char *_wr_buf = TEST_WRITE_DATA; size_t _wr_buf_sz = sizeof(TEST_WRITE_DATA);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
           UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_WRITE2FILE );
#undef   TEST_WRITE_DATA
    {
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) {
            int  mock_bignum = 0;
            expect(BN_new,  will_return(&mock_bignum));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(128)));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(128)));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(32)));
            expect(BN_bn2hex, will_return(strdup("1d2a07b4836c998e"  "2a07b4836c939a08")), when(a, is_equal_to(&mock_bignum)));
            expect(BN_bn2hex, will_return(strdup("e1d2a07b48360c"  )), when(a, is_equal_to(&mock_bignum))); // error, not 128-bit IV
            expect(BN_bn2hex, will_return(strdup("08e38732")), when(a, is_equal_to(&mock_bignum)));
            expect(BN_free,  when(a, is_equal_to(&mock_bignum)));
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(503)));
            while(!mock_done_flag)
                uv_run(loop, UV_RUN_ONCE);
            assert_that(json_object_size(mock_err_info), is_equal_to(1));
            const char *actual_err_msg = json_string_value(json_object_get(mock_err_info, "transcoder"));
            assert_that(actual_err_msg, contains_string("rotation failure"));
        }
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
           UTEST_RESOURCE_PATH_VERSION_1 "/" HLS_MASTER_PLAYLIST_FILENAME, UTEST_OPS_UNLINK );
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASASRC_BASEPATH, MOCK_USER_ID, MOCK_UPLD_REQ_1_ID,
            UTEST_RESOURCE_PATH_VERSION_1, UTEST_OPS_RMDIR );
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__key_rotate_fail
#undef  UTEST_RESOURCE_VERSION_1
#undef  UTEST_RESOURCE_PATH_VERSION_1
#undef  MOCK_UPDATE_SECS_PLAYLIST
#undef  MOCK_UPDATE_SECS_KEYFILE


TestSuite *app_transcoder_hls_init_stream_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__init_stream__update_ok_1); // the 3 cases will run sequentially
    add_test(suite, atfp_hls_test__init_stream__update_interval_too_short);
    add_test(suite, atfp_hls_test__init_stream__update_ok_2);
    add_test(suite, atfp_hls_test__init_stream__args_error);
    add_test(suite, atfp_hls_test__init_stream__dst_plist_lock_fail);
    add_test(suite, atfp_hls_test__init_stream__src_plist_missing);
    add_test(suite, atfp_hls_test__init_stream__src_scandir_missing);
    add_test(suite, atfp_hls_test__init_stream__src_plist_corrupted);
    add_test(suite, atfp_hls_test__init_stream__key_rotate_fail);
    return suite;
}
