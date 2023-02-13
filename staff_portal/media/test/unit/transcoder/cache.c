#include <sys/file.h>

#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"
#include "transcoder/file_processor.h"

extern atfp_ops_entry_t  atfp_ops_video_hls;

#define  STRINGIFY(x)  #x

#define  MOCK_DOC_ID  "bL2y+asirW7tr9="
#define  MOCK_USR_ID  2345
#define  MOCK_RESOURCE_ID  0xa2e3b4c5
#define  MOCK_USR_ID_STR   STRINGIFY(2345)
#define  MOCK_RESOURCE_ID_STR  STRINGIFY(a2e3b4c5)
#define  MOCK_VERSION   "Tk"

#define  UTEST_FILE_BASEPATH   "tmp/utest"
#define  UTEST_ASALOCAL_BASEPATH     UTEST_FILE_BASEPATH "/asalocal"
#define  UTEST_CACHE_BASEPATH       UTEST_ASALOCAL_BASEPATH "/" ATFP_CACHED_FILE_FOLDERNAME
#define  UTEST_CACHE_TARGETPATH     UTEST_CACHE_BASEPATH "/" MOCK_DOC_ID
#define  UTEST_CACHED_FILEPATH   "abc/def/ghij.txt"

#define  DONE_FLAG_INDEX__IN_ASA_USRARG    (ERRINFO_INDEX__IN_ASA_USRARG + 1)
#define  FILEDES2_INDEX__IN_ASA_USRARG     (ERRINFO_INDEX__IN_ASA_USRARG + 2)
#define  NUM_CB_ARGS_ASAOBJ                (FILEDES2_INDEX__IN_ASA_USRARG + 1)
#define  MOCK_STORAGE_ALIAS    "localfs"


static void  utest__stcch_init__done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    json_t *err_info = asaobj->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    size_t  err_cnt = json_object_size(err_info);
    mock(asaobj, processor, _map, err_cnt);
    if(asaobj->cb_args.size > DONE_FLAG_INDEX__IN_ASA_USRARG) {
        uint8_t  *done_flg_p = asaobj->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flg_p)
            *done_flg_p = 1;
    }
} // end of  utest__stcch_init__done_cb

static void  utest__stcch_deinit__done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    mock(asaobj, processor, _map);
    if(asaobj->cb_args.size > DONE_FLAG_INDEX__IN_ASA_USRARG) {
        uint8_t  *done_flg_p = asaobj->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flg_p)
            *done_flg_p = 1;
    }
}

static void  utest_mock_fp_processing_fn(atfp_t *processor)
{
    json_t *err_info  =  processor->data.error;
    uint8_t  _is_final = 0, *_is_final_p = &_is_final;
    const char *src_bytes = processor->transfer.streaming_dst.block.data;
    size_t  *src_bytes_sz_p = &processor->transfer.streaming_dst.block.len;
    int err = mock(processor, _is_final_p, src_bytes, src_bytes_sz_p);
    processor->transfer.streaming_dst.flags.is_final = _is_final;
    if(err)
        json_object_set_new(err_info, "transcoder", json_string("[utest] process failure"));
    processor->data.callback(processor);
} // end of  utest_mock_fp_processing_fn

static uint8_t  utest_mock_fp_deinit_fn(atfp_t *processor)
{
    mock(processor);
    free(processor);
    return  0;
}


#define  ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP \
    void    (*fp_process_origin)(atfp_t *) = atfp_ops_video_hls.ops.processing ; \
    uint8_t (*fp_deinit_origin)(atfp_t *)  = atfp_ops_video_hls.ops.deinit ; \
    atfp_ops_video_hls.ops.processing = utest_mock_fp_processing_fn; \
    atfp_ops_video_hls.ops.deinit = utest_mock_fp_deinit_fn;

#define  ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN \
    atfp_ops_video_hls.ops.processing = fp_process_origin; \
    atfp_ops_video_hls.ops.deinit = fp_deinit_origin;


#define  ATFP_STREAM_CACHE_INIT__SETUP \
    uint8_t  mock_done_flag = 0 ; \
    uint32_t mock_buf_sz = 200; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(), *mock_err_info = json_object(); \
    asa_cfg_t  mock_storage_cfg = {.alias=MOCK_STORAGE_ALIAS, .base_path=UTEST_ASALOCAL_BASEPATH, \
        .ops={.fn_close=app_storage_localfs_close, .fn_open=app_storage_localfs_open, \
            .fn_mkdir=app_storage_localfs_mkdir, .fn_typesize=app_storage_localfs_typesize }}; \
    app_cfg_t *mock_appcfg = app_get_global_cfg(); \
    mock_appcfg->storages.size = 1; \
    mock_appcfg->storages.capacity = 1; \
    mock_appcfg->storages.entries = &mock_storage_cfg; \
    json_object_set_new(mock_spec, "doc_basepath", json_string(UTEST_CACHE_TARGETPATH)); \
    json_object_set_new(mock_spec, API_QPARAM_LABEL__DOC_DETAIL, json_string(UTEST_CACHED_FILEPATH)); \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    mkdir(UTEST_ASALOCAL_BASEPATH, S_IRWXU); \
    mkdir(UTEST_CACHE_BASEPATH, S_IRWXU); \
    mkdir(UTEST_CACHE_TARGETPATH, S_IRWXU);

#define  ATFP_STREAMCACHE_INIT__METADATA_SETUP { \
    json_object_set_new(mock_spec, "crypto_key_id", json_string("its_key_id")); \
    atfp_data_t  mock_fp_data = {.usr_id=246, .upld_req_id=0xe2acce55, .spec=mock_spec}; \
    int err = atfp_cache_save_metadata(UTEST_CACHE_TARGETPATH, "hls", &mock_fp_data); \
    assert_that(err, is_equal_to(0)); \
}

#define  ATFP_STREAM_CACHE_INIT__TEARDOWN \
    unlink(UTEST_CACHE_TARGETPATH  "/" ATFP_ENCRYPT_METADATA_FILENAME); \
    unlink(UTEST_CACHE_TARGETPATH  "/" UTEST_CACHED_FILEPATH); \
    rmdir(UTEST_CACHE_TARGETPATH   "/abc/def"); \
    rmdir(UTEST_CACHE_TARGETPATH   "/abc"); \
    rmdir(UTEST_CACHE_TARGETPATH); \
    rmdir(UTEST_CACHE_BASEPATH); \
    rmdir(UTEST_ASALOCAL_BASEPATH); \
    rmdir(UTEST_FILE_BASEPATH); \
    mock_appcfg->storages.size = 0; \
    mock_appcfg->storages.capacity = 0; \
    mock_appcfg->storages.entries = NULL; \
    json_decref(mock_spec); \
    json_decref(mock_err_info);


Ensure(atfp_test__stcch_init__newentry_ok) 
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    ATFP_STREAMCACHE_INIT__METADATA_SETUP
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest_mock_fp_processing_fn, will_return(0), when(processor, is_not_null));
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null),  when(err_cnt, is_equal_to(0))  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest_mock_fp_deinit_fn,  when(processor, is_not_null));
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__newentry_ok


Ensure(atfp_test__stcch_init__cached_found)
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    { // assume cached file already exists
        mkdir(UTEST_CACHE_TARGETPATH"/abc", S_IRWXU);
        mkdir(UTEST_CACHE_TARGETPATH"/abc/def", S_IRWXU);
        const char *_path = UTEST_CACHE_TARGETPATH "/" UTEST_CACHED_FILEPATH;
        int fd = open(_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
        close(fd);
    }
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null),  when(err_cnt, is_equal_to(0))  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__cached_found


Ensure(atfp_test__stcch_init__missing_metadata)
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null),  when(err_cnt, is_greater_than(0))  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__missing_metadata


Ensure(atfp_test__stcch_init__fileprocessor_error)
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    ATFP_STREAMCACHE_INIT__METADATA_SETUP
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        int err = 1;
        expect(utest_mock_fp_processing_fn, will_return(err), when(processor, is_not_null));
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null),  when(err_cnt, is_greater_than(0))  );
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)));
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest_mock_fp_deinit_fn,  when(processor, is_not_null));
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__fileprocessor_error


static  ASA_RES_CODE _utest_storage_mkdir_err1_fn (asa_op_base_cfg_t *asaobj, uint8_t  allow_exists)
{ // it will cause error on creating file later
    asaobj->op.mkdir.path.origin[0] = 'X';
    return app_storage_localfs_mkdir(asaobj, allow_exists);
}

Ensure(atfp_test__stcch_init__mk_detailpath_error)
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    ATFP_STREAMCACHE_INIT__METADATA_SETUP
    mock_storage_cfg.ops.fn_mkdir = _utest_storage_mkdir_err1_fn;
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest_mock_fp_processing_fn, will_return(0), when(processor, is_not_null));
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null),  when(err_cnt, is_greater_than(0))  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest_mock_fp_deinit_fn,  when(processor, is_not_null));
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    mock_storage_cfg.ops.fn_mkdir = app_storage_localfs_mkdir;
    rmdir(UTEST_CACHE_TARGETPATH  "/Xbc/def");
    rmdir(UTEST_CACHE_TARGETPATH  "/Xbc");
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__mk_detailpath_error



static  ASA_RES_CODE _utest_storage_open_err_fn (asa_op_base_cfg_t *asaobj)
{ // assume other request is creating the same file and lock it concurrently
    int ret = strcmp(asaobj->op.open.dst_path, UTEST_CACHE_TARGETPATH"/"UTEST_CACHED_FILEPATH);
    if((ret == 0) && (asaobj->op.open.flags == (O_WRONLY | O_CREAT)))
    {
        int *samefile_fd = asaobj->cb_args.entries[FILEDES2_INDEX__IN_ASA_USRARG];
        *samefile_fd = open(asaobj->op.open.dst_path, asaobj->op.open.flags, asaobj->op.open.mode);
        assert_that(*samefile_fd, is_greater_than(-1));
        ret = flock(*samefile_fd, LOCK_EX | LOCK_NB);
        assert_that(ret, is_equal_to(0));
    }
    return app_storage_localfs_open(asaobj);
} // end of  _utest_storage_open_err_fn

Ensure(atfp_test__stcch_init__newentry_lock_fail)
{
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__SETUP
    ATFP_STREAM_CACHE_INIT__SETUP
    ATFP_STREAMCACHE_INIT__METADATA_SETUP
    int samefile_fd = -1;
    mock_storage_cfg.ops.fn_open = _utest_storage_open_err_fn;
    asa_op_localfs_cfg_t  *_cch_entry = atfp_streamcache_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ,  mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = &mock_done_flag;
        _cch_entry->super.cb_args.entries[FILEDES2_INDEX__IN_ASA_USRARG] = &samefile_fd;
        expect(utest_mock_fp_processing_fn, will_return(0), when(processor, is_not_null));
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null),  when(err_cnt, is_greater_than(0))  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        expect(utest_mock_fp_deinit_fn,  when(processor, is_not_null));
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_not_null)  );
        _cch_entry ->super.deinit(&_cch_entry->super);
        uv_run(loop, UV_RUN_ONCE);
    }
    mock_storage_cfg.ops.fn_open = app_storage_localfs_open;
    if(samefile_fd >= 0) {
        flock(samefile_fd, LOCK_UN | LOCK_NB);
        close(samefile_fd);
    }
    ATFP_STREAM_CACHE_INIT__TEARDOWN
    ATFP_STREAMCACHE_SWITCH_PROCESSING_FN__TEARDOWN
} // end of  atfp_test__stcch_init__newentry_lock_fail


#define UTEST_ASASRC_USR_PATH        UTEST_ASALOCAL_BASEPATH"/"MOCK_USR_ID_STR
#define UTEST_ASASRC_RESOURCE_PATH   UTEST_ASASRC_USR_PATH"/"MOCK_RESOURCE_ID_STR
#define UTEST_ASASRC_COMMIT_PATH     UTEST_ASASRC_RESOURCE_PATH"/"ATFP__COMMITTED_FOLDER_NAME
#define UTEST_ASASRC_FINAL_FILEPATH   UTEST_ASASRC_COMMIT_PATH"/"MOCK_VERSION

#define ATFP_NONSTREAM_CACHE_INIT__SETUP \
    ATFP_STREAM_CACHE_INIT__SETUP \
    json_object_set_new(mock_spec, "storage_alias", json_string(MOCK_STORAGE_ALIAS)); \
    json_object_set_new(mock_spec, "asa_src_basepath", json_string(UTEST_ASASRC_COMMIT_PATH)); \
    json_object_set_new(mock_spec, API_QPARAM_LABEL__DOC_DETAIL, json_string(MOCK_VERSION)); \
    mkdir(UTEST_ASASRC_USR_PATH, S_IRWXU); \
    mkdir(UTEST_ASASRC_RESOURCE_PATH, S_IRWXU); \
    mkdir(UTEST_ASASRC_COMMIT_PATH, S_IRWXU);

#define ATFP_NONSTREAM_CACHE_INIT__TEARDOWN \
    unlink(UTEST_ASASRC_FINAL_FILEPATH); \
    unlink(UTEST_CACHE_TARGETPATH"/"MOCK_VERSION); \
    rmdir(UTEST_ASASRC_COMMIT_PATH); \
    rmdir(UTEST_ASASRC_RESOURCE_PATH); \
    rmdir(UTEST_ASASRC_USR_PATH); \
    ATFP_STREAM_CACHE_INIT__TEARDOWN

Ensure(atfp_test__nstcch_init__newentry_ok)
{
    ATFP_NONSTREAM_CACHE_INIT__SETUP
    int src_fd = open(UTEST_ASASRC_FINAL_FILEPATH, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    close(src_fd);
    asa_op_localfs_cfg_t *_cch_entry = atfp_cache_nonstream_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ, mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        int ret = access(UTEST_CACHE_TARGETPATH"/"MOCK_VERSION, F_OK);
        assert_that(ret, is_equal_to(-1));
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null), when(_map, is_not_null),  when(err_cnt, is_equal_to(0)) );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        assert_that(access(UTEST_CACHE_TARGETPATH"/"MOCK_VERSION, F_OK), is_equal_to(0));
        mock_done_flag = 0;
        _cch_entry ->super.deinit(&_cch_entry->super);
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null), when(_map, is_null)  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_NONSTREAM_CACHE_INIT__TEARDOWN
} // end of atfp_test__nstcch_init__newentry_ok


Ensure(atfp_test__nstcch_init__cached_found)
{
    ATFP_NONSTREAM_CACHE_INIT__SETUP
    int src_fd = open(UTEST_CACHE_TARGETPATH"/"MOCK_VERSION, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    close(src_fd);
    asa_op_localfs_cfg_t *_cch_entry = atfp_cache_nonstream_init (loop, mock_spec, mock_err_info,
           NUM_CB_ARGS_ASAOBJ, mock_buf_sz, utest__stcch_init__done_cb, utest__stcch_deinit__done_cb);
    assert_that(_cch_entry, is_not_null);
    if(_cch_entry) {
        _cch_entry->super.cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG] = (uint8_t *) &mock_done_flag;
        expect(utest__stcch_init__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null), when(_map, is_null),  when(err_cnt, is_equal_to(0)) );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
        mock_done_flag = 0;
        _cch_entry ->super.deinit(&_cch_entry->super);
        expect(utest__stcch_deinit__done_cb,   when(asaobj, is_equal_to(_cch_entry)),
                when(processor, is_null), when(_map, is_null)  );
        while(!mock_done_flag)
            uv_run(loop, UV_RUN_ONCE);
    }
    ATFP_NONSTREAM_CACHE_INIT__TEARDOWN
} // end of atfp_test__nstcch_init__cached_found
#undef  UTEST_ASASRC_USR_PATH      
#undef  UTEST_ASASRC_RESOURCE_PATH 
#undef  UTEST_ASASRC_COMMIT_PATH   
#undef  UTEST_ASASRC_FINAL_FILEPATH



static  void utest_cachecommon_proceed_done_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result,
        h2o_iovec_t *buf, uint8_t is_final)
{
    size_t  out_sz = 0;
    char   *out_bytes = NULL;
    if(buf) {
        out_sz = buf->len;
        out_bytes = strndup(buf->base, out_sz); // ensure NULL-terminating string
    }
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_asa_map_t *_map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    json_t *err_info = asaobj->cb_args.entries[ERRINFO_INDEX__IN_ASA_USRARG];
    size_t  err_cnt = json_object_size(err_info);
    mock(asaobj, result, processor, _map, err_cnt, out_sz, out_bytes, is_final);
    if(asaobj->cb_args.size > DONE_FLAG_INDEX__IN_ASA_USRARG) {
        uint8_t  *done_flg_p = asaobj->cb_args.entries[DONE_FLAG_INDEX__IN_ASA_USRARG];
        if(done_flg_p)
            *done_flg_p = 1;
    }
    if(out_bytes)
        free(out_bytes);
} // end of utest_cachecommon_proceed_done_cb

#define  ATFP_CACHECOMMON_PROCEED_DBLK__SETUP \
    char  mock_cch_wr_buf[UTEST_STREAM_BUF_SZ] = {0}; \
    uint8_t  mock_done_flag = 0 ; \
    uv_loop_t *loop  = uv_default_loop(); \
    json_t *mock_spec = json_object(), *mock_err_info = json_object(); \
    void  *mock_asa_numargs[NUM_CB_ARGS_ASAOBJ] = {NULL, NULL, mock_spec, mock_err_info, &mock_done_flag}; \
    asa_cfg_t  mock_storage_cfg = {.alias=MOCK_STORAGE_ALIAS, .base_path=UTEST_ASALOCAL_BASEPATH, \
        .ops={.fn_write=app_storage_localfs_write, .fn_read=app_storage_localfs_read }}; \
    mkdir(UTEST_FILE_BASEPATH,   S_IRWXU); \
    int mock_dst_fd = open(UTEST_FILE_BASEPATH"/ghi.txt", O_RDWR | O_CREAT, S_IRUSR | S_IWUSR); \
    asa_cch_usrdata_t  mock_cch_usrdata = {.callback={.proceed=NULL}}; \
    asa_op_localfs_cfg_t  mock_asa_cch = {.loop=loop, .file={.data=&mock_cch_usrdata, .file=mock_dst_fd}, \
        .super={.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=mock_asa_numargs}, .storage=&mock_storage_cfg, \
            .op={ .write={.src_max_nbytes=UTEST_STREAM_BUF_SZ, .src=&mock_cch_wr_buf[0]}, \
                .read={.dst_max_nbytes=UTEST_STREAM_BUF_SZ, .dst=&mock_cch_wr_buf[0]} \
    }}}; \
    json_object_set_new(mock_spec, "_asa_cache_local", json_integer((uint64_t)&mock_asa_cch));

#define  ATFP_STREAMCACHE_PROCEED_DBLK__FP__SETUP \
    char  mock_stream_datachunk[UTEST_STREAM_BUF_SZ] = {0}; \
    atfp_ops_t  mock_fp_ops = {.processing=utest_mock_fp_processing_fn}; \
    atfp_t  mock_fp = {.ops=&mock_fp_ops, .data={.spec=mock_spec, .error=mock_err_info}, \
        .transfer={.streaming_dst={.block={.len=0, .data=&mock_stream_datachunk[0] }}}}; \
    mock_asa_numargs[ATFP_INDEX__IN_ASA_USRARG] = &mock_fp;


#define  ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN \
    close(mock_dst_fd); \
    unlink(UTEST_FILE_BASEPATH"/ghi.txt"); \
    rmdir(UTEST_FILE_BASEPATH); \
    json_decref(mock_spec); \
    json_decref(mock_err_info);


Ensure(atfp_test__stcch_proceed_dblk__from_fileprocessor)
{
#define   UTEST_STREAM_PROCESSED_CHUNK1   "under-estimating tech debt will eventually be"
#define   UTEST_STREAM_PROCESSED_CHUNK2   "come integral pa"
#define   UTEST_STREAM_PROCESSED_CHUNK3   "rt of organization debt and hard to fix"
#define   UTEST_STREAM_BUF_SZ   sizeof(UTEST_STREAM_PROCESSED_CHUNK1)
    ATFP_CACHECOMMON_PROCEED_DBLK__SETUP
    ATFP_STREAMCACHE_PROCEED_DBLK__FP__SETUP
#define   START_PROCESSING(expect_rd_chunk, expect_final_flag) { \
        size_t  wr_sz = sizeof(char) * (sizeof(expect_rd_chunk) - 1); \
        uint8_t _expect_final_flag = expect_final_flag; \
        expect(utest_mock_fp_processing_fn, will_return(0), when(processor, is_not_null), \
            will_set_contents_of_parameter(src_bytes, expect_rd_chunk, wr_sz), \
            will_set_contents_of_parameter(src_bytes_sz_p, &wr_sz, sizeof(size_t)), \
            will_set_contents_of_parameter(_is_final_p, &_expect_final_flag, sizeof(uint8_t)) \
        ); \
        expect(utest_cachecommon_proceed_done_cb,   when(asaobj, is_equal_to(&mock_asa_cch)), \
            when(processor, is_not_null),  when(err_cnt, is_equal_to(0)), \
            when(out_sz, is_equal_to(wr_sz)),   when(is_final, is_equal_to(_expect_final_flag)), \
            when(out_bytes, is_equal_to_string(expect_rd_chunk)), \
        ); \
        atfp_streamcache_proceed_datablock (&mock_asa_cch.super, utest_cachecommon_proceed_done_cb); \
        uv_run(loop, UV_RUN_ONCE); \
    }
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK1, 0);
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK2, 0);
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK3, 1);
    {
#define  EXPECT_CACHED_CONTENT   UTEST_STREAM_PROCESSED_CHUNK1  UTEST_STREAM_PROCESSED_CHUNK2  UTEST_STREAM_PROCESSED_CHUNK3
#define  MAX_RD_SZ   sizeof(EXPECT_CACHED_CONTENT)
        char actual_cached_content[MAX_RD_SZ]  = {0};
        lseek(mock_dst_fd, 0, SEEK_SET);
        read(mock_dst_fd, &actual_cached_content[0], MAX_RD_SZ);
        assert_that(&actual_cached_content[0], is_equal_to_string(EXPECT_CACHED_CONTENT));
#undef  EXPECT_CACHED_CONTENT
#undef  MAX_RD_SZ
    }
    ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN
#undef   START_PROCESSING
#undef   UTEST_STREAM_BUF_SZ
#undef   UTEST_STREAM_PROCESSED_CHUNK1
#undef   UTEST_STREAM_PROCESSED_CHUNK2
#undef   UTEST_STREAM_PROCESSED_CHUNK3
} // end of  atfp_test__stcch_proceed_dblk__from_fileprocessor


Ensure(atfp_test__stcch_proceed_dblk__from_cachedentry)
{
#define   UTEST_STREAM_PROCESSED_CHUNK1   "under-estimating tech debt will eventu"
#define   UTEST_STREAM_PROCESSED_CHUNK2   "ally become integral part of organizat"
#define   UTEST_STREAM_PROCESSED_CHUNK3   "ion debt and hard to fix"
#define   UTEST_STREAM_BUF_SZ   (sizeof(UTEST_STREAM_PROCESSED_CHUNK1) - 1)
    ATFP_CACHECOMMON_PROCEED_DBLK__SETUP
    {
        write(mock_dst_fd, UTEST_STREAM_PROCESSED_CHUNK1, strlen(UTEST_STREAM_PROCESSED_CHUNK1));
        write(mock_dst_fd, UTEST_STREAM_PROCESSED_CHUNK2, strlen(UTEST_STREAM_PROCESSED_CHUNK2));
        write(mock_dst_fd, UTEST_STREAM_PROCESSED_CHUNK3, strlen(UTEST_STREAM_PROCESSED_CHUNK3));
        lseek(mock_dst_fd, 0, SEEK_SET);
    }
#define   START_PROCESSING(expect_rd_chunk, expect_final_flag) { \
        size_t  wr_sz = sizeof(char) * (sizeof(expect_rd_chunk) - 1); \
        expect(utest_cachecommon_proceed_done_cb,   when(asaobj, is_equal_to(&mock_asa_cch)), \
            when(processor, is_null),  when(err_cnt, is_equal_to(0)), \
            when(out_sz, is_equal_to(wr_sz)),   when(is_final, is_equal_to(expect_final_flag)), \
        ); \
        atfp_streamcache_proceed_datablock (&mock_asa_cch.super, utest_cachecommon_proceed_done_cb); \
        uv_run(loop, UV_RUN_ONCE); \
        int ret = memcmp(mock_asa_cch.super.op.read.dst, expect_rd_chunk, wr_sz); \
        assert_that(ret, is_equal_to(0)); \
    }
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK1, 0);
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK2, 0);
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK3, 1);
    ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN
#undef   START_PROCESSING
#undef   UTEST_STREAM_BUF_SZ
#undef   UTEST_STREAM_PROCESSED_CHUNK1
#undef   UTEST_STREAM_PROCESSED_CHUNK2
#undef   UTEST_STREAM_PROCESSED_CHUNK3
} // end of  atfp_test__stcch_proceed_dblk__from_cachedentry


Ensure(atfp_test__stcch_proceed_dblk__fileprocessor_error) 
{
#define   UTEST_STREAM_PROCESSED_CHUNK1   "under-estimating tech debt will eventually"
#define   UTEST_STREAM_BUF_SZ   sizeof(UTEST_STREAM_PROCESSED_CHUNK1)
    ATFP_CACHECOMMON_PROCEED_DBLK__SETUP
    ATFP_STREAMCACHE_PROCEED_DBLK__FP__SETUP
    int err = 1;
    expect(utest_mock_fp_processing_fn, will_return(err), when(processor, is_not_null));
    expect(utest_cachecommon_proceed_done_cb,   when(asaobj, is_equal_to(&mock_asa_cch)),
        when(processor, is_not_null),  when(err_cnt, is_greater_than(0)),  );
    atfp_streamcache_proceed_datablock (&mock_asa_cch.super, utest_cachecommon_proceed_done_cb);
    ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN
#undef   START_PROCESSING
#undef   UTEST_STREAM_BUF_SZ
#undef   UTEST_STREAM_PROCESSED_CHUNK1
} // end of  atfp_test__stcch_proceed_dblk__fileprocessor_error


#define  ATFP_CACHE_MAP_PROCEED_DBLK__SETUP \
    asa_op_localfs_cfg_t mock_asa_src_remote = {.loop=loop, .file={.file=-1}, \
        .super={.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=mock_asa_numargs}, .storage=&mock_storage_cfg, \
            .op={.read={.dst_max_nbytes=UTEST_STREAM_BUF_SZ, .dst=&mock_cch_wr_buf[0]} \
    }}}; \
    atfp_asa_map_t *mock_map = atfp_asa_map_init(0); \
    mock_asa_numargs[ASAMAP_INDEX__IN_ASA_USRARG] = &mock_map; \
    atfp_asa_map_set_source(mock_map, &mock_asa_src_remote.super); \
    atfp_asa_map_set_localtmp(mock_map, &mock_asa_cch);

#define  ATFP_CACHE_MAP_PROCEED_DBLK__TEARDOWN \
    atfp_asa_map_deinit(mock_map);

#define   UTEST_STREAM_PROCESSED_CHUNK1   "under-estimating tech debt will eventu"
#define   UTEST_STREAM_PROCESSED_CHUNK2   "ally become integral part of organizat"
#define   UTEST_STREAM_PROCESSED_CHUNK3   "ion debt and hard to fix"
#define   UTEST_STREAM_BUF_SZ   (sizeof(UTEST_STREAM_PROCESSED_CHUNK1) - 1)
Ensure(atfp_test__nstcch_proceed_dblk__from_remote_src)
{
    ATFP_CACHECOMMON_PROCEED_DBLK__SETUP
    ATFP_CACHE_MAP_PROCEED_DBLK__SETUP
    int mock_src_fd = open(UTEST_FILE_BASEPATH"/mnp.txt", O_RDWR | O_CREAT, S_IRUSR | S_IWUSR);
    {
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK1, strlen(UTEST_STREAM_PROCESSED_CHUNK1));
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK2, strlen(UTEST_STREAM_PROCESSED_CHUNK2));
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK3, strlen(UTEST_STREAM_PROCESSED_CHUNK3));
        lseek(mock_src_fd, 0, SEEK_SET);
        mock_asa_src_remote.file.file = mock_src_fd;
    }
#define  START_PROCESSING(expect_rd_bytes, expect_final_flag, expect_err_cnt) { \
    atfp_nonstreamcache_proceed_datablock (&mock_asa_cch.super, utest_cachecommon_proceed_done_cb); \
    if(json_object_size(mock_err_info) == 0) { \
        mock_done_flag = 0; \
        const char *_expect_rd_bytes = expect_rd_bytes; \
        size_t _expect_num_rbytes = _expect_rd_bytes ? strlen(_expect_rd_bytes): 0; \
        expect(utest_cachecommon_proceed_done_cb,   when(asaobj, is_equal_to(&mock_asa_cch)), \
            when(_map, is_not_null),  when(err_cnt, is_equal_to(expect_err_cnt)), \
            when(out_sz, is_equal_to(_expect_num_rbytes)),   when(is_final, is_equal_to(expect_final_flag)), \
            when(out_bytes, is_equal_to_string(_expect_rd_bytes)), \
        ); \
        while(!mock_done_flag) \
            uv_run(loop, UV_RUN_ONCE); \
    } \
}
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK1, 0, 0)
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK2, 0, 0)
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK3, 1, 0)
    close(mock_src_fd);
    unlink(UTEST_FILE_BASEPATH"/mnp.txt");
    ATFP_CACHE_MAP_PROCEED_DBLK__TEARDOWN
    ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN
} // end of atfp_test__nstcch_proceed_dblk__from_remote_src

Ensure(atfp_test__nstcch_proceed_dblk__src_read_error) {
    ATFP_CACHECOMMON_PROCEED_DBLK__SETUP
    ATFP_CACHE_MAP_PROCEED_DBLK__SETUP
    int mock_src_fd = open(UTEST_FILE_BASEPATH"/mnp.txt", O_RDWR | O_CREAT, S_IRUSR | S_IWUSR);
    {
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK1, strlen(UTEST_STREAM_PROCESSED_CHUNK1));
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK2, strlen(UTEST_STREAM_PROCESSED_CHUNK2));
        write(mock_src_fd, UTEST_STREAM_PROCESSED_CHUNK3, strlen(UTEST_STREAM_PROCESSED_CHUNK3));
        lseek(mock_src_fd, 0, SEEK_SET);
        mock_asa_src_remote.file.file = mock_src_fd;
    }
    START_PROCESSING(UTEST_STREAM_PROCESSED_CHUNK1, 0, 0)
    close(mock_src_fd); // assume it is being deleted in the middle
    unlink(UTEST_FILE_BASEPATH"/mnp.txt");
    START_PROCESSING(NULL, 1, 1)
    ATFP_CACHE_MAP_PROCEED_DBLK__TEARDOWN
    ATFP_CACHECOMMON_PROCEED_DBLK__TEARDOWN
} // end of  atfp_test__nstcch_proceed_dblk__src_read_error
#undef   START_PROCESSING
#undef   UTEST_STREAM_BUF_SZ
#undef   UTEST_STREAM_PROCESSED_CHUNK1
#undef   UTEST_STREAM_PROCESSED_CHUNK2
#undef   UTEST_STREAM_PROCESSED_CHUNK3


TestSuite *app_stream_cache_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_test__stcch_init__newentry_ok);
    add_test(suite, atfp_test__stcch_init__cached_found);
    add_test(suite, atfp_test__stcch_init__missing_metadata);
    add_test(suite, atfp_test__stcch_init__fileprocessor_error);
    add_test(suite, atfp_test__stcch_init__mk_detailpath_error);
    add_test(suite, atfp_test__stcch_init__newentry_lock_fail);
    add_test(suite, atfp_test__stcch_proceed_dblk__from_fileprocessor);
    add_test(suite, atfp_test__stcch_proceed_dblk__from_cachedentry);
    add_test(suite, atfp_test__stcch_proceed_dblk__fileprocessor_error);
    add_test(suite, atfp_test__nstcch_init__newentry_ok);
    add_test(suite, atfp_test__nstcch_init__cached_found);
    add_test(suite, atfp_test__nstcch_proceed_dblk__from_remote_src);
    add_test(suite, atfp_test__nstcch_proceed_dblk__src_read_error);
    return suite;
}
