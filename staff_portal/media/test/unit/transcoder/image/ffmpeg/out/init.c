#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "transcoder/image/common.h"
#include "transcoder/image/ffmpeg.h"

extern const atfp_ops_entry_t  atfp_ops_image_ffmpg_out;

#define  UTEST_SRC_FILE_LOCALBUF_PATH  "/path/to/src/buff"
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


#define  UTEST_FFM_INIT_SETUP \
    void  *asasrc_cb_args[NUM_CB_ARGS_ASAOBJ] = {0},  *asadst_cb_args[NUM_CB_ARGS_ASAOBJ] = {0}; \
    asa_op_base_cfg_t  mock_asa_src = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asasrc_cb_args}}; \
    asa_op_base_cfg_t  mock_asa_dst = {.cb_args={.size=NUM_CB_ARGS_ASAOBJ, .entries=asadst_cb_args}, \
            .deinit=utest_img_ffo__asaobj_deinit }; \
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
    atfp_img_t *mock_fp_dst = (atfp_img_t *) atfp_ops_image_ffmpg_out.ops.instantiate(); \
    mock_fp_dst->super.backend_id = ATFP_BACKEND_LIB__UNKNOWN; \
    mock_fp_dst->super.data = (atfp_data_t){.storage={.handle=&mock_asa_dst}, .version=UTEST_FP_VERSION, \
         .error=mock_errinfo, .spec=mock_spec, .callback=utest_atfp_img_ffo__usr_cb}; \
    mock_fp_dst->ops.dst.avctx_init   = utest_img_ffo__avctx_init; \
    mock_fp_dst->ops.dst.avctx_deinit = utest_img_ffo__avctx_deinit; \
    mock_fp_dst->ops.dst.avfilter_init = utest_img_ffo__avfilt_init; \
    asasrc_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = &mock_fp_src; \
    asasrc_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \
    asadst_cb_args[ATFP_INDEX__IN_ASA_USRARG]   = mock_fp_dst; \
    asadst_cb_args[ASAMAP_INDEX__IN_ASA_USRARG] = mock_map; \

#define  UTEST_FFM_INIT_TEARDOWN \
    json_decref(mock_errinfo); \
    json_decref(mock_spec); \
    atfp_asa_map_set_source(mock_map, NULL); \
    atfp_asa_map_set_localtmp(mock_map, NULL); \
    atfp_asa_map_deinit(mock_map);

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

TestSuite *app_transcoder_img_ffm_out_init_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_img_ffo_test__init_ok);
    add_test(suite, atfp_img_ffo_test__avctx_error);
    add_test(suite, atfp_img_ffo_test__avfilt_error);
    return suite;
}
