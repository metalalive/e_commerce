#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

extern const atfp_ops_entry_t  atfp_ops_image_ffmpg_out;

#define  UTEST_SRC_FILE_LOCALBUF_PATH  "/path/to/src/buff"
#define  UTEST_REMOTE_FILE_PATH  "/path/to/remote"
#define  UTEST_FP_VERSION   "GY"
#define  UTEST_SPEC_TOPLVL_SERIAL  "{\"outputs\":{\""UTEST_FP_VERSION"\":{}}}"
#define  NUM_CB_ARGS_ASAOBJ  (ASAMAP_INDEX__IN_ASA_USRARG + 1)


static void  utest_img_ffo__avctx_init (atfp_av_ctx_t *src, atfp_av_ctx_t *dst,
        const char *filepath, json_t *filt_spec, json_t *err_info)
{
    int err = mock(src, dst, filepath, filt_spec, err_info);
    if(err)
       json_object_set_new(err_info, "utest", json_string("mock error detail"));
}

static void   utest_img_ffo__avfilt_init (atfp_av_ctx_t *src, atfp_av_ctx_t *dst,
        json_t *filt_spec, json_t *err_info)
{
    int err = mock(src, dst, filt_spec, err_info);
    if(err)
       json_object_set_new(err_info, "utest", json_string("mock error detail"));
}

static void  utest_img_ffo__avctx_deinit (atfp_av_ctx_t *dst)
{ mock(dst); }

static void  utest_img_ffo__asaobj_deinit (asa_op_base_cfg_t *asaobj)
{ mock(asaobj); }

static void  utest_atfp_img_ffo__usr_cb (atfp_t *processor)
{ mock(processor); }


#define  UTEST_FFM_COMMON_SETUP \
    void  *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0},  *asadst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_op_base_cfg_t  mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asasrc_cb_args}}; \
    asa_op_base_cfg_t  mock_asa_dst = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asadst_cb_args}, \
            .deinit=utest_img_ffo__asaobj_deinit, .op={.mkdir={.path={.origin=UTEST_REMOTE_FILE_PATH}}} }; \
    asa_op_localfs_cfg_t  mock_asa_local = {.super={.op={.open={.dst_path=UTEST_SRC_FILE_LOCALBUF_PATH}}}}; \
    atfp_asa_map_t  *mock_map = atfp_asa_map_init(1); \
    atfp_asa_map_set_source(mock_map, &mock_asa_src); \
    atfp_asa_map_set_localtmp(mock_map, &mock_asa_local); \
    atfp_asa_map_add_destination(mock_map,  &mock_asa_dst); \
    json_t *mock_errinfo = json_object(); \
    json_t *mock_spec = json_loadb(UTEST_SPEC_TOPLVL_SERIAL, sizeof(UTEST_SPEC_TOPLVL_SERIAL) - 1, 0, NULL); \
    atfp_av_ctx_t  mock_avctx_src = {0}; \
    atfp_img_t  mock_fp_src = {.super={.backend_id=ATFP_BACKEND_LIB__UNKNOWN}, .av=&mock_avctx_src, \
        .super={.data={.storage={.handle=&mock_asa_src}}}}; \
    asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = &mock_fp_src; \
    asasrc_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \

#define  UTEST_FFM_COMMON_TEARDOWN \
    json_decref(mock_errinfo); \
    json_decref(mock_spec); \
    atfp_asa_map_set_source(mock_map, NULL); \
    atfp_asa_map_set_localtmp(mock_map, NULL); \
    atfp_asa_map_deinit(mock_map);


#define  UTEST_FFM_INIT_SETUP \
    UTEST_FFM_COMMON_SETUP \
    atfp_img_t *mock_fp_dst = (atfp_img_t *) atfp_ops_image_ffmpg_out.ops.instantiate(); \
    mock_fp_dst->super.backend_id = ATFP_BACKEND_LIB__UNKNOWN; \
    mock_fp_dst->super.data = (atfp_data_t){.storage={.handle=&mock_asa_dst}, .version=UTEST_FP_VERSION, \
         .error=mock_errinfo, .spec=mock_spec, .callback=utest_atfp_img_ffo__usr_cb}; \
    mock_fp_dst->ops.dst.avctx_init   = utest_img_ffo__avctx_init; \
    mock_fp_dst->ops.dst.avctx_deinit = utest_img_ffo__avctx_deinit; \
    mock_fp_dst->ops.dst.avfilter_init = utest_img_ffo__avfilt_init; \
    asadst_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = mock_fp_dst; \
    asadst_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \

#define  UTEST_FFM_INIT_TEARDOWN   UTEST_FFM_COMMON_TEARDOWN

Ensure(atfp_img_ffo_test__init_ok)
{
    UTEST_FFM_INIT_SETUP
    mock_fp_src.super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    mock_fp_dst->super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    expect(utest_img_ffo__avctx_init, will_return(0),  when(src, is_equal_to(&mock_avctx_src)),
          when(filepath, is_equal_to_string(UTEST_SRC_FILE_LOCALBUF_PATH"."UTEST_FP_VERSION)),
          when(dst, is_not_null),  when(filt_spec, is_not_null), when(err_info, is_equal_to(mock_errinfo))
    );
    expect(utest_img_ffo__avfilt_init, will_return(0),  when(src, is_equal_to(&mock_avctx_src)),
          when(dst, is_not_null),  when(filt_spec, is_not_null), when(err_info, is_equal_to(mock_errinfo))
    );
    expect(utest_atfp_img_ffo__usr_cb, when(processor, is_equal_to(mock_fp_dst)));
    atfp__image_ffm_out__init_transcode(&mock_fp_dst->super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    expect(utest_img_ffo__avctx_deinit, when(dst, is_not_null));
    expect(utest_img_ffo__asaobj_deinit, when(asaobj, is_equal_to(&mock_asa_dst)));
    atfp__image_ffm_out__deinit_transcode(&mock_fp_dst->super);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__init_ok


Ensure(atfp_img_ffo_test__avctx_error)
{
    UTEST_FFM_INIT_SETUP
    // subcase #1 : backend ID mismatch
    expect(utest_atfp_img_ffo__usr_cb, when(processor, is_equal_to(mock_fp_dst)));
    atfp__image_ffm_out__init_transcode(&mock_fp_dst->super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    json_object_clear(mock_errinfo);
    // subcase #2 : avctx init error
    int expect_err = 1;
    mock_fp_src.super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    mock_fp_dst->super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    expect(utest_img_ffo__avctx_init, will_return(expect_err),  when(src, is_equal_to(&mock_avctx_src)),
          when(filepath, is_equal_to_string(UTEST_SRC_FILE_LOCALBUF_PATH"."UTEST_FP_VERSION)),
          when(dst, is_not_null),  when(filt_spec, is_not_null), when(err_info, is_equal_to(mock_errinfo))
    );
    expect(utest_atfp_img_ffo__usr_cb, when(processor, is_equal_to(mock_fp_dst)));
    atfp__image_ffm_out__init_transcode(&mock_fp_dst->super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    expect(utest_img_ffo__avctx_deinit, when(dst, is_not_null));
    expect(utest_img_ffo__asaobj_deinit, when(asaobj, is_equal_to(&mock_asa_dst)));
    atfp__image_ffm_out__deinit_transcode(&mock_fp_dst->super);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avctx_error


Ensure(atfp_img_ffo_test__avfilt_error)
{
    UTEST_FFM_INIT_SETUP
    int expect_err = 1;
    mock_fp_src.super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    mock_fp_dst->super.backend_id = ATFP_BACKEND_LIB__FFMPEG ;
    expect(utest_img_ffo__avctx_init, will_return(0),  when(src, is_equal_to(&mock_avctx_src)),
          when(filepath, is_equal_to_string(UTEST_SRC_FILE_LOCALBUF_PATH"."UTEST_FP_VERSION)),
          when(dst, is_not_null),  when(filt_spec, is_not_null), when(err_info, is_equal_to(mock_errinfo))
    );
    expect(utest_img_ffo__avfilt_init, will_return(expect_err),  when(src, is_equal_to(&mock_avctx_src)),
          when(dst, is_not_null),  when(filt_spec, is_not_null), when(err_info, is_equal_to(mock_errinfo))
    );
    expect(utest_atfp_img_ffo__usr_cb, when(processor, is_equal_to(mock_fp_dst)));
    atfp__image_ffm_out__init_transcode(&mock_fp_dst->super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    expect(utest_img_ffo__avctx_deinit, when(dst, is_not_null));
    expect(utest_img_ffo__asaobj_deinit, when(asaobj, is_equal_to(&mock_asa_dst)));
    atfp__image_ffm_out__deinit_transcode(&mock_fp_dst->super);
    UTEST_FFM_INIT_TEARDOWN
} // end of  atfp_img_ffo_test__avfilt_error


static  uint8_t utest_img_ffo__fp_src__has_done_processing (atfp_t *processor)
{ return (uint8_t)mock(processor); }

static int  utest_img_ffo__avctx_wirte_pkt_local(atfp_av_ctx_t *avobj)
{ return (int)mock(avobj); }

static int  utest_img_ffo__avctx_encode(atfp_av_ctx_t *avobj)
{ return (int)mock(avobj); }

static int  utest_img_ffo__avctx_filter(atfp_av_ctx_t *src, atfp_av_ctx_t *dst)
{ return (int)mock(src, dst); }

static int  utest_img_ffo__avctx_final_filter(atfp_av_ctx_t *src, atfp_av_ctx_t *dst)
{ return (int)mock(src, dst); }

static int  utest_img_ffo__avctx_final_encode(atfp_av_ctx_t *avobj)
{ return (int)mock(avobj); }

static int  utest_img_ffo__avctx_final_write_local(atfp_av_ctx_t *avobj)
{ return (int)mock(avobj); }

static int  utest_img_ffo__avctx_has_done_flush_filter (atfp_av_ctx_t *avobj)
{ return (int)mock(avobj); }

static ASA_RES_CODE  utest_img_ffo__avctx_save2storage(atfp_img_t *fp)
{
    ASA_RES_CODE result = (ASA_RES_CODE)mock(fp);
    if(result != ASTORAGE_RESULT_ACCEPT)
        json_object_set_new(fp->super.data.error, "reason", json_string("unit test error"));
    return result;
}

#define  UTEST_FFM_PROCESS_SETUP \
    UTEST_FFM_COMMON_SETUP \
    atfp_ops_t mock_fp_src_ops = {.has_done_processing=utest_img_ffo__fp_src__has_done_processing}; \
    mock_fp_src.super.ops = (atfp_ops_t *) &mock_fp_src_ops; \
    atfp_av_ctx_t  mock_avctx_dst = {0}; \
    atfp_img_t   mock_fp_dst = {.av=&mock_avctx_dst}; \
    mock_fp_dst.super.data = (atfp_data_t){.storage={.handle=&mock_asa_dst}, \
         .error=mock_errinfo, .spec=mock_spec, .callback=utest_atfp_img_ffo__usr_cb}; \
    mock_fp_dst.ops.dst.write_pkt = utest_img_ffo__avctx_wirte_pkt_local; \
    mock_fp_dst.ops.dst.encode  = utest_img_ffo__avctx_encode; \
    mock_fp_dst.ops.dst.filter  = utest_img_ffo__avctx_filter; \
    mock_fp_dst.ops.dst.finalize.filter  = utest_img_ffo__avctx_final_filter; \
    mock_fp_dst.ops.dst.finalize.encode  = utest_img_ffo__avctx_final_encode; \
    mock_fp_dst.ops.dst.finalize.write   = utest_img_ffo__avctx_final_write_local; \
    mock_fp_dst.ops.dst.has_done_flush_filter  = utest_img_ffo__avctx_has_done_flush_filter; \
    mock_fp_dst.ops.dst.save_to_storage  = utest_img_ffo__avctx_save2storage; \
    asadst_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = &mock_fp_dst; \
    asadst_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] =  mock_map;

#define  UTEST_FFM_PROCESS_TEARDOWN  UTEST_FFM_COMMON_TEARDOWN


Ensure(atfp_img_ffo_test__process_write_pkt_ok)
{
    UTEST_FFM_PROCESS_SETUP // assume 2 packets will be encoded from the filtered frame
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_encode, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst)) );
    expect(utest_img_ffo__avctx_encode, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_encode, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA));
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA));
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(0),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    assert_that(mock_fp_dst.ops.dst.write_pkt, is_equal_to(utest_img_ffo__avctx_wirte_pkt_local));
    assert_that(mock_fp_dst.ops.dst.encode, is_equal_to(utest_img_ffo__avctx_encode));
    assert_that(mock_fp_dst.ops.dst.filter, is_equal_to(utest_img_ffo__avctx_filter));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of  atfp_img_ffo_test__process_write_pkt_ok

Ensure(atfp_img_ffo_test__process_filter_error)
{
    UTEST_FFM_PROCESS_SETUP
    int expect_err_code = -3;
    expect(utest_img_ffo__avctx_filter, will_return(expect_err_code),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(0),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_filter_error

Ensure(atfp_img_ffo_test__process_encode_error)
{
    UTEST_FFM_PROCESS_SETUP
    int expect_err_code = -3;
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_encode, will_return(expect_err_code),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(0),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_encode_error

Ensure(atfp_img_ffo_test__process_need_more_frame)
{
    UTEST_FFM_PROCESS_SETUP
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(0),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_need_more_frame


Ensure(atfp_img_ffo_test__process_flush_filter)
{
    UTEST_FFM_PROCESS_SETUP
    int src_fp_done = 1;
    assert_that(mock_fp_dst.ops.dst.filter, is_equal_to(utest_img_ffo__avctx_filter));
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    assert_that(mock_fp_dst.ops.dst.filter, is_equal_to(utest_img_ffo__avctx_final_filter));
    // ---------
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_encode, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_encode, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA));
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_flush_filter


Ensure(atfp_img_ffo_test__process_flush_encoder)
{
    UTEST_FFM_PROCESS_SETUP
    int src_fp_done = 1, dst_flush_filt_done = 1;
    assert_that(mock_fp_dst.ops.dst.filter, is_equal_to(utest_img_ffo__avctx_filter));
    expect(utest_img_ffo__avctx_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(dst_flush_filt_done));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    assert_that(mock_fp_dst.ops.dst.filter, is_equal_to(utest_img_ffo__avctx_final_filter));
    assert_that(mock_fp_dst.ops.dst.encode, is_equal_to(utest_img_ffo__avctx_final_encode));
    // ---------
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA));
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__NEED_MORE_DATA),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(0));
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_flush_encoder


Ensure(atfp_img_ffo_test__process_save2storage_ok)
{
    UTEST_FFM_PROCESS_SETUP
    int src_fp_done = 1, dst_flush_filt_done = 1;
    mock_fp_dst.ops.dst.filter = utest_img_ffo__avctx_final_filter;
    mock_fp_dst.ops.dst.encode = utest_img_ffo__avctx_final_encode;
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst)) );
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_wirte_pkt_local, will_return(ATFP_AVCTX_RET__OK));
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER));
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(dst_flush_filt_done));
    expect(utest_img_ffo__avctx_final_write_local, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_save2storage, will_return(ASTORAGE_RESULT_ACCEPT),
           when(fp, is_equal_to(&mock_fp_dst.super))  );
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_equal_to(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(1));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_save2storage_ok


Ensure(atfp_img_ffo_test__process_save2storage_err)
{
    UTEST_FFM_PROCESS_SETUP
    int src_fp_done = 1, dst_flush_filt_done = 1;
    mock_fp_dst.ops.dst.filter = utest_img_ffo__avctx_final_filter;
    mock_fp_dst.ops.dst.encode = utest_img_ffo__avctx_final_encode;
    expect(utest_img_ffo__avctx_final_filter, will_return(ATFP_AVCTX_RET__OK),
           when(src, is_equal_to(&mock_avctx_src)), when(dst, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_final_encode, will_return(ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__fp_src__has_done_processing, will_return(src_fp_done),
          when(processor, is_equal_to(&mock_fp_src.super))  );
    expect(utest_img_ffo__avctx_has_done_flush_filter, will_return(dst_flush_filt_done));
    expect(utest_img_ffo__avctx_final_write_local, will_return(ATFP_AVCTX_RET__OK),
           when(avobj, is_equal_to(&mock_avctx_dst))  );
    expect(utest_img_ffo__avctx_save2storage, will_return(ASTORAGE_RESULT_DATA_ERROR),
           when(fp, is_equal_to(&mock_fp_dst.super))  );
    atfp__image_ffm_out__proceeding_transcode(&mock_fp_dst.super);
    assert_that(json_object_size(mock_errinfo), is_greater_than(0));
    assert_that(mock_fp_dst.super.op_async_done.processing, is_equal_to(0));
    UTEST_FFM_PROCESS_TEARDOWN
} // end of atfp_img_ffo_test__process_save2storage_err


TestSuite *app_transcoder_img_ffm_out_init_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffo_test__init_ok);
    add_test(suite, atfp_img_ffo_test__avctx_error);
    add_test(suite, atfp_img_ffo_test__avfilt_error);
    add_test(suite, atfp_img_ffo_test__process_write_pkt_ok);
    add_test(suite, atfp_img_ffo_test__process_filter_error);
    add_test(suite, atfp_img_ffo_test__process_encode_error);
    add_test(suite, atfp_img_ffo_test__process_need_more_frame);
    add_test(suite, atfp_img_ffo_test__process_flush_filter);
    add_test(suite, atfp_img_ffo_test__process_flush_encoder);
    add_test(suite, atfp_img_ffo_test__process_save2storage_ok);
    add_test(suite, atfp_img_ffo_test__process_save2storage_err);
    return suite;
}
