#include <search.h>
#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>
#include <uv.h>

#include "app_cfg.h"
#include "views.h"
#include "storage/localfs.h"
#include "transcoder/video/hls.h"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASALOCAL_BASEPATH   UTEST_FILE_BASEPATH "/asalocal"

#define  DONE_FLAG_INDEX__IN_ASA_USRARG    (ASAMAP_INDEX__IN_ASA_USRARG + 1)
#define  NUM_CB_ARGS_ASAOBJ                (DONE_FLAG_INDEX__IN_ASA_USRARG + 1)

#define  MOCK_USER_ID          426
#define  MOCK_UPLD_REQ_1_ID    0xd150de7a
#define  MOCK_RESOURCE_1_ID    "eb0y#aWs"
#define  MOCK_HOST_DOMAIN      "your.domain.com:443"
#define  MOCK__QUERYPARAM_LABEL__RES_ID    "ut_docID" // fit 8-byte size, to make valgrind happy
#define  MOCK__QUERYPARAM_LABEL__VERSION   "ut_doc_quality"
#define  MOCK__QUERYPARAM_LABEL__DETAIL    "u_detail"


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

static const char * utest__get_crypto_key (json_t *_keyinfo, const char *_key_id, json_t **_item_out)
{
    const char *key_id_out = (const char *) mock(_keyinfo, _key_id);
    *_item_out = json_object_get(_keyinfo, key_id_out);
    return  key_id_out;
}

static int  utest__encrypt_document_id (atfp_data_t *fp_data, json_t *kitem, unsigned char **out, size_t *out_sz)
{ return  (int) mock(fp_data, out, out_sz); }


static void _utest_hls_init_stream__done_cb(atfp_t *processor)
{
    json_t  *return_data_obj = NULL;
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    json_t  *err_info = processor->data.error;
    json_t  *spec = processor->data.spec;
    uint8_t  num_err_item = json_object_size(err_info);
    const char *doc_id = NULL, *host = NULL, *detail = NULL;
    if(num_err_item == 0) {
        return_data_obj = json_object_get(spec, "return_data");
        assert_that(return_data_obj, is_not_equal_to(NULL));
        if(return_data_obj) {
            const char *actual_typ = json_string_value(json_object_get(return_data_obj, "type"));
            assert_that(actual_typ, is_equal_to_string("hls"));
            host = json_string_value(json_object_get(return_data_obj, "host"));
            doc_id = json_string_value(json_object_get(return_data_obj, MOCK__QUERYPARAM_LABEL__RES_ID));
            detail = json_string_value(json_object_get(return_data_obj, MOCK__QUERYPARAM_LABEL__DETAIL));
        }
    }
    int actual_resp_status = (int) json_integer_value(json_object_get(spec, "http_resp_code"));
    mock(processor, actual_resp_status, doc_id, host, detail);
    uint8_t  *done_flg_p = hlsproc->asa_local.super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
    if(done_flg_p)
        *done_flg_p = 1;
} // end of  _utest_hls_init_stream__done_cb

#define  ATFP_HLS_TEST__INIT_STREAM__SETUP \
    uint8_t mock_done_flag = 0 ; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(); \
    json_t *mock_err_info = json_object(); \
    void  *mock_asa_cb_args [NUM_CB_ARGS_ASAOBJ]; \
    app_cfg_t *mock_appcfg = app_get_global_cfg(); \
    mock_appcfg->tmp_buf.path = UTEST_ASALOCAL_BASEPATH; \
    asa_cfg_t  mock_local_storage_cfg = {.base_path=NULL, .ops={.fn_mkdir=app_storage_localfs_mkdir, \
        .fn_open=app_storage_localfs_open, .fn_close=app_storage_localfs_close, \
        .fn_read=app_storage_localfs_read, .fn_write=app_storage_localfs_write, }}; \
    atfp_hls_t *mock_fp = (atfp_hls_t *)atfp__video_hls__instantiate(); \
    mock_fp->internal.op.get_crypto_key = utest__get_crypto_key; \
    mock_fp->internal.op.encrypt_document_id = utest__encrypt_document_id; \
    mock_fp->super.data = (atfp_data_t) {.callback=_utest_hls_init_stream__done_cb, .spec=mock_spec, \
        .error=mock_err_info, .usr_id=MOCK_USER_ID, .upld_req_id=MOCK_UPLD_REQ_1_ID}; \
    mock_fp->asa_local = (asa_op_localfs_cfg_t) {.super={.storage=&mock_local_storage_cfg, \
        .cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=mock_asa_cb_args} }} ; \
    mock_asa_cb_args[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp->super; \
    mock_asa_cb_args[DONE_FLAG_INDEX__IN_ASA_USRARG] = &mock_done_flag; \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU);


#define  ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP { \
    json_t  *mock_qp_labels = json_object(),  *mock_update_interval = json_object(); \
    json_object_set_new(mock_qp_labels, "resource_id", json_string(MOCK__QUERYPARAM_LABEL__RES_ID)); \
    json_object_set_new(mock_qp_labels, "version", json_string(MOCK__QUERYPARAM_LABEL__VERSION)); \
    json_object_set_new(mock_qp_labels, "detail",  json_string(MOCK__QUERYPARAM_LABEL__DETAIL)); \
    json_object_set_new(mock_update_interval, "keyfile",   json_real(MOCK_UPDATE_SECS_KEYFILE)); \
    json_object_set_new(mock_spec, "host", json_string(MOCK_HOST_DOMAIN)); \
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
    rmdir(UTEST_FILE_BASEPATH); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);


#define  MOCK_UPDATE_SECS_KEYFILE    2.f
#define  MOCK_DOC_ID1__HALF1    "5p7RJbdkFR"
#define  MOCK_DOC_ID1__HALF2    "ArEk5xVm="
#define  MOCK_DOC_ID2__HALF1    "p7RJdk"
#define  MOCK_DOC_ID2__HALF2    "FRxArxVm="
#define  MOCK_DOC_ID_1      MOCK_DOC_ID1__HALF1 "/" MOCK_DOC_ID1__HALF2
#define  MOCK_DOC_ID_2      MOCK_DOC_ID2__HALF1 "/" MOCK_DOC_ID2__HALF2
#define  MOCK_KEY_ITEM_1    {"908e3873", "1d2a07b4836c998e""2a07b4836c939a08",  "e1d2a07b48360c99""1d2a07b48361c909", MOCK_DOC_ID_1}
#define  MOCK_KEY_ITEM_2    {"08e3873a", "d2a07b4836c998e5""a07b4836c939a081",  "1d2a07b48360c9e9""d2a07b48361c9096", MOCK_DOC_ID_2}
Ensure(atfp_hls_test__init_stream__update_ok_1) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
#define  _COMMON_CODE(_keyitem, _enable_key_rotation) \
    const char *keyitem[4] = _keyitem; \
    const char *key_id_hex = keyitem[0], *expect_key_hex = keyitem[1], *expect_iv_hex = keyitem[2], \
             *expect_doc_id = keyitem[3]; \
    { \
        atfp__video_hls__init_stream(&mock_fp->super); \
        size_t err_cnt = json_object_size(mock_err_info); \
        assert_that(err_cnt, is_equal_to(0)); \
        if(err_cnt == 0) { \
            int expect_resp_status = 200; \
            int mock_bignum = 0; \
            const char *_expect_doc_id = strdup(expect_doc_id); \
            size_t _expect_doc_id_sz = strlen(expect_doc_id); \
            if(_enable_key_rotation) { \
                expect(BN_new,  will_return(&mock_bignum)); \
                expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY << 3))); \
                expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_IV << 3))); \
                expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY_ID << 2))); \
                expect(BN_bn2hex, will_return(strdup(expect_key_hex)), when(a, is_equal_to(&mock_bignum))); \
                expect(BN_bn2hex, will_return(strdup(expect_iv_hex )), when(a, is_equal_to(&mock_bignum))); \
                expect(BN_bn2hex, will_return(strdup(key_id_hex)), when(a, is_equal_to(&mock_bignum))); \
                expect(BN_free,  when(a, is_equal_to(&mock_bignum))); \
            } \
            expect(utest__get_crypto_key,  when(_key_id, is_equal_to_string(ATFP__CRYPTO_KEY_MOST_RECENT)), \
                     will_return(key_id_hex), when(_keyinfo, is_not_null)); \
            expect(utest__encrypt_document_id, will_return(1), \
                      will_set_contents_of_parameter(out, &_expect_doc_id, sizeof(char *)), \
                      will_set_contents_of_parameter(out_sz, &_expect_doc_id_sz, sizeof(size_t)), \
                    ); \
            expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(expect_resp_status)), \
                    when(host,   is_equal_to_string(MOCK_HOST_DOMAIN)), \
                    when(detail, is_equal_to_string(HLS_MASTER_PLAYLIST_FILENAME)), \
                    when(doc_id, is_equal_to_string(expect_doc_id)) ); \
            while(!mock_done_flag) \
                uv_run(loop, UV_RUN_ONCE); \
            assert_that(err_cnt, is_equal_to(0)); \
        } \
    }
    _COMMON_CODE(MOCK_KEY_ITEM_1, 1)
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__update_ok_1


Ensure(atfp_hls_test__init_stream__skip_key_rotation) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    _COMMON_CODE(MOCK_KEY_ITEM_1, 0)
    {
        const char *path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID_1"/"ATFP_ENCRYPT_METADATA_FILENAME;
        json_t *_metadata = json_load_file(path, 0, NULL);
        assert_that(json_string_value(json_object_get(_metadata, "key_id")), is_equal_to_string(key_id_hex));
        assert_that(json_string_value(json_object_get(_metadata, "mimetype")), is_equal_to_string("hls"));
        assert_that(json_integer_value(json_object_get(_metadata, "usr_id")), is_equal_to(MOCK_USER_ID));
        assert_that(json_integer_value(json_object_get(_metadata, "upld_req")), is_equal_to(MOCK_UPLD_REQ_1_ID));
        json_decref(_metadata);
    }
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
}  // end of  atfp_hls_test__init_stream__skip_key_rotation


Ensure(atfp_hls_test__init_stream__update_ok_2) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    sleep(MOCK_UPDATE_SECS_KEYFILE + 1.f);
    _COMMON_CODE(MOCK_KEY_ITEM_2, 1)
    {
        const char *path = NULL;
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID_1"/"ATFP_ENCRYPT_METADATA_FILENAME;
        unlink(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID_2"/"ATFP_ENCRYPT_METADATA_FILENAME;
        unlink(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID_1;
        rmdir(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID_2;
        rmdir(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID1__HALF1;
        rmdir(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME"/"MOCK_DOC_ID2__HALF1;
        rmdir(path);
        path = UTEST_ASALOCAL_BASEPATH"/"ATFP_ENCRYPTED_FILE_FOLDERNAME;
        rmdir(path);
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of  atfp_hls_test__init_stream__update_ok_2
#undef  _COMMON_CODE
#undef  MOCK_KEY_ITEM_1
#undef  MOCK_KEY_ITEM_2
#undef  MOCK_DOC_ID_1
#undef  MOCK_DOC_ID_2
#undef  MOCK_DOC_ID1__HALF1    
#undef  MOCK_DOC_ID1__HALF2    
#undef  MOCK_DOC_ID2__HALF1    
#undef  MOCK_DOC_ID2__HALF2    
#undef  MOCK_UPDATE_SECS_KEYFILE


Ensure(atfp_hls_test__init_stream__args_error) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    (void *) loop;
    atfp__video_hls__init_stream(&mock_fp->super);
    size_t err_cnt = json_object_size(mock_err_info);
    assert_that(err_cnt, is_equal_to(1));
    int actual_resp_status = (int) json_integer_value(json_object_get(mock_spec, "http_resp_code"));
    assert_that(actual_resp_status, is_equal_to(400));
    const char *actual_err_msg = json_string_value(json_object_get(mock_err_info, "transcoder"));
    assert_that(actual_err_msg, contains_string("missing arguments in spec"));
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__args_error



#define  MOCK_UPDATE_SECS_KEYFILE    1.f
Ensure(atfp_hls_test__init_stream__key_rotate_fail) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    {
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        if(err_cnt == 0) {
            int  mock_bignum = 0;
            expect(BN_new,  will_return(&mock_bignum));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY << 3)));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_IV << 3)));
            expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY_ID << 2)));
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
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of atfp_hls_test__init_stream__key_rotate_fail
#undef  MOCK_UPDATE_SECS_KEYFILE


#define  MOCK_UPDATE_SECS_KEYFILE    1.f
Ensure(atfp_hls_test__init_stream__encrypt_id_error) {
    ATFP_HLS_TEST__INIT_STREAM__SETUP
    ATFP_HLS_TEST__INIT_STREAM__SPEC_SETUP
    {
        const char *expect_key_id_hex = "108e3872";
        atfp__video_hls__init_stream(&mock_fp->super);
        size_t err_cnt = json_object_size(mock_err_info);
        assert_that(err_cnt, is_equal_to(0));
        int  mock_bignum = 0;
        expect(BN_new,  will_return(&mock_bignum));
        expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY << 3)));
        expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_IV << 3)));
        expect(BN_rand, will_return(1), when(bits, is_equal_to(HLS__NBYTES_KEY_ID << 2)));
        expect(BN_bn2hex, will_return(strdup("1d2a07b4836c998e" "2a07b4836c939a08")), when(a, is_equal_to(&mock_bignum)));
        expect(BN_bn2hex, will_return(strdup("e1d2a07b48360c99" "a07b4836c939a085")), when(a, is_equal_to(&mock_bignum)));
        expect(BN_bn2hex, will_return(strdup(expect_key_id_hex)), when(a, is_equal_to(&mock_bignum)));
        expect(BN_free,  when(a, is_equal_to(&mock_bignum)));
        expect(utest__get_crypto_key,  when(_key_id, is_equal_to_string(ATFP__CRYPTO_KEY_MOST_RECENT)), 
                will_return(expect_key_id_hex), when(_keyinfo, is_not_null));
        expect(utest__encrypt_document_id, will_return(0)); // error happened
        expect(_utest_hls_init_stream__done_cb, when(actual_resp_status, is_equal_to(503)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(json_object_size(mock_err_info), is_equal_to(1));
    }
    UTEST_RUN_OPERATION_WITH_PATH(UTEST_ASALOCAL_BASEPATH, MOCK_USER_ID,
            MOCK_UPLD_REQ_1_ID,  HLS_CRYPTO_KEY_FILENAME, UTEST_OPS_UNLINK);
    ATFP_HLS_TEST__INIT_STREAM__TEARDOWN
} // end of  atfp_hls_test__init_stream__encrypt_id_error
#undef  MOCK_UPDATE_SECS_KEYFILE




static void  utest_hls_stream_elm__build_mst_plist (atfp_hls_t *_hlsproc)
{ mock(_hlsproc); }

static void  utest_hls_stream_elm__build_lvl2_plist (atfp_hls_t *_hlsproc)
{ mock(_hlsproc); }

static void  utest_hls_stream_elm__encrypt_segment (atfp_hls_t *_hlsproc)
{ mock(_hlsproc); }


#define    HLS_TEST__SEEK_STREAM_ELEMENT__SETUP \
    json_t *mock_spec = json_object(); \
    json_t *mock_err_info = json_object(); \
    atfp_hls_t  mock_fp = {.super={.data={.spec=mock_spec, .error=mock_err_info}}, \
        .internal={.op={.build_master_playlist=utest_hls_stream_elm__build_mst_plist, \
            .build_secondary_playlist=utest_hls_stream_elm__build_lvl2_plist, \
            .encrypt_segment=utest_hls_stream_elm__encrypt_segment \
        }} \
    };

#define    HLS_TEST__SEEK_STREAM_ELEMENT__TEARDOWN \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

Ensure(atfp_hls_test__seek_stream_element__ok) {
    HLS_TEST__SEEK_STREAM_ELEMENT__SETUP
#define RUN(_version, _path, fn_name) { \
        const char *_expect_version = _version; \
        json_object_set_new(mock_spec, API_QUERYPARAM_LABEL__DETAIL_ELEMENT, \
                json_string(_version  _path)); \
        expect(fn_name, when(_hlsproc, is_equal_to(&mock_fp))); \
        atfp__video_hls__seek_stream_element (&mock_fp.super); \
        if(_expect_version && strlen(_expect_version)) \
            assert_that(mock_fp.super.data.version, is_equal_to_string(_expect_version)); \
        if(mock_fp.super.data.version) { \
            free((void *)mock_fp.super.data.version); \
            mock_fp.super.data.version = NULL; \
        } \
    }
    RUN("", HLS_MASTER_PLAYLIST_FILENAME, utest_hls_stream_elm__build_mst_plist)
    RUN("xU", "/"HLS_PLAYLIST_FILENAME, utest_hls_stream_elm__build_lvl2_plist)
    RUN("Lh", "/"HLS_FMP4_FILENAME, utest_hls_stream_elm__encrypt_segment)
    RUN("9B", "/"HLS_PLAYLIST_FILENAME, utest_hls_stream_elm__build_lvl2_plist)
    RUN("k5", "/"HLS_SEGMENT_FILENAME_PREFIX, utest_hls_stream_elm__encrypt_segment)
    HLS_TEST__SEEK_STREAM_ELEMENT__TEARDOWN
#undef RUN
} // end of  atfp_hls_test__seek_stream_element__ok

Ensure(atfp_hls_test__seek_stream_element__invalid_detail) {
    HLS_TEST__SEEK_STREAM_ELEMENT__SETUP
#define RUN(_path) { \
        const char *__path = _path; \
        if(!__path || strlen(__path) == 0) { \
            json_object_del(mock_spec, API_QUERYPARAM_LABEL__DETAIL_ELEMENT); \
        } else  { \
            json_object_set_new(mock_spec, API_QUERYPARAM_LABEL__DETAIL_ELEMENT, json_string(__path)); \
        } \
        atfp__video_hls__seek_stream_element (&mock_fp.super); \
        assert_that(json_object_size(mock_err_info), is_greater_than(0)); \
        int http_resp_status = json_integer_value(json_object_get(mock_err_info, "_http_resp_code")); \
        assert_that(http_resp_status, is_equal_to(400)); \
        assert_that(json_object_get(mock_err_info, "transcoder"), is_not_null); \
        json_object_clear(mock_err_info); \
    }
    RUN("")
    RUN("non-existent")
    RUN("x"HLS_MASTER_PLAYLIST_FILENAME)
    RUN("xXxx"HLS_MASTER_PLAYLIST_FILENAME)
    RUN("ab$e"HLS_PLAYLIST_FILENAME)
    RUN("/x/"HLS_MASTER_PLAYLIST_FILENAME)
    RUN("ab$"HLS_PLAYLIST_FILENAME)
    RUN("fa0"HLS_FMP4_FILENAME)
    RUN(HLS_PLAYLIST_FILENAME)
    RUN("xin"HLS_SEGMENT_FILENAME_PREFIX)
    RUN(HLS_FMP4_FILENAME)
    RUN("///rust"HLS_SEGMENT_FILENAME_PREFIX)
    RUN("/e/a/"HLS_SEGMENT_FILENAME_PREFIX)
    RUN("///"HLS_FMP4_FILENAME)
    RUN("/i/"HLS_SEGMENT_FILENAME_PREFIX)
    HLS_TEST__SEEK_STREAM_ELEMENT__TEARDOWN
#undef RUN
} // end of atfp_hls_test__seek_stream_element__invalid_detail


TestSuite *app_transcoder_hls_init_stream_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__init_stream__update_ok_1); // the 3 cases will run sequentially
    add_test(suite, atfp_hls_test__init_stream__skip_key_rotation);
    add_test(suite, atfp_hls_test__init_stream__update_ok_2);
    add_test(suite, atfp_hls_test__init_stream__args_error);
    add_test(suite, atfp_hls_test__init_stream__key_rotate_fail);
    add_test(suite, atfp_hls_test__init_stream__encrypt_id_error);
    add_test(suite, atfp_hls_test__seek_stream_element__ok);
    add_test(suite, atfp_hls_test__seek_stream_element__invalid_detail);
    return suite;
}
