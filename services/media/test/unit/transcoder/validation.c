#include <libavformat/avformat.h>
#include <libavcodec/codec.h>
#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"
#include "transcoder/datatypes.h"
#include "transcoder/file_processor.h"

#define STRINGIFY(x) #x

#define ELEMENT_STREAM_VIDEO_GEN(_label, _codec, _height, _width, _fps) \
    "\"" _label "\": {\"type\":\"video\",  \"codec\":\"" _codec "\"," \
    "\"attribute\":{\"height_pixel\": " STRINGIFY(_height) ", \"width_pixel\": " STRINGIFY(_width \
    ) ", \"framerate\":" STRINGIFY(_fps) "}}"

#define ELEMENT_STREAM_AUDIO_GEN(_label, _codec, _bitrate) \
    "\""_label \
    "\":{\"type\":\"audio\",\"codec\":\""_codec \
    "\",\"attribute\":{\"bitrate_kbps\":" STRINGIFY(_bitrate) "}}"

#define VDO_OUTPUT_ITEM_GEN(_label, _container, _elm_st_map_list) \
    "\""_label \
    "\":{\"container\":\""_container \
    "\",\"elementary_streams\": ["_elm_st_map_list \
    "]}"

#define IMG_OUTPUT_ITEM_GEN( \
    _label, _scale_w, _scale_h, _crop_w, _crop_h, _crop_pos_x, _crop_pos_y, _msk_patt \
) \
    "\"" _label "\":{\"mask\":{\"pattern\":\""_msk_patt \
    "\"}," \
    "\"scale\":{\"width\":" STRINGIFY(_scale_w) ",\"height\":" STRINGIFY(_scale_h \
    ) "}," \
      "\"crop\":{\"x\":" STRINGIFY(_crop_pos_x) ", \"y\":" STRINGIFY(_crop_pos_y \
      ) ", \"width\":" STRINGIFY(_crop_w) ", \"height\":" STRINGIFY(_crop_h) "}}"

#define VIDEO_REQ_BODY_GEN(_resource_id, _elm_st_section, _output_section) \
    "{\"elementary_streams\":{"_elm_st_section \
    "},\"resource_id\":\""_resource_id \
    "\"," \
    "\"outputs\":{"_output_section \
    "}}"

#define IMAGE_REQ_BODY_GEN(_resource_id, _output_section) \
    "{\"resource_id\":\""_resource_id \
    "\",\"outputs\":{"_output_section \
    "}}"

#define ELM_ST_V1 ELEMENT_STREAM_VIDEO_GEN("vdo_one", "libx264rgb", 400, 630, 20)
#define ELM_ST_V2 ELEMENT_STREAM_VIDEO_GEN("vdo_two", "v410", 390, 620, 18)
#define ELM_ST_V3 ELEMENT_STREAM_VIDEO_GEN("vdo_thri", "flv", 370, 600, 17)
#define ELM_ST_V4 ELEMENT_STREAM_VIDEO_GEN("vdo_for", "wmv2", 390, 620, 16)
#define ELM_ST_A1 ELEMENT_STREAM_AUDIO_GEN("ado_one", "aac", 61)
#define ELM_ST_A2 ELEMENT_STREAM_AUDIO_GEN("ado_two", "dca", 61)
#define ELM_ST_A3 ELEMENT_STREAM_AUDIO_GEN("ado_thri", "alac", 57)

#define UTEST_VALIDATE_VDO_REQ__SETUP( \
    num_a_codecs, num_v_codecs, num_muxers, num_rso_a_bitrate, a_bitrate_init_expr, num_rso_v_pxl, \
    v_pixel_init_expr, num_rso_v_fps, v_fps_init_expr, _req_body_serial \
) \
    int     idx = 0; \
    json_t *mock_spec = json_loadb(_req_body_serial, sizeof(_req_body_serial) - 1, 0, NULL); \
    assert_that(mock_spec, is_not_null); \
    if (!mock_spec) \
        return; \
    json_t *mock_err_info = json_object(); \
    AVCodec mock_a_codecs[num_a_codecs] = {0}, mock_v_codecs[num_v_codecs] = {0}, \
            *mock_a_codec_ps[num_a_codecs] = {0}, *mock_v_codec_ps[num_v_codecs] = {0}; \
    AVOutputFormat           mock_muxers[num_muxers] = {0}, *mock_muxer_ps[num_muxers] = {0}; \
    aav_cfg_resolution_pix_t mock_video_rso_pixels[num_rso_v_pxl] = v_pixel_init_expr; \
    uint16_t                 mock_audio_bitrate_kbps[num_rso_a_bitrate] = a_bitrate_init_expr; \
    uint8_t                  mock_video_framerates[num_rso_v_fps] = v_fps_init_expr; \
    for (idx = 0; idx < num_a_codecs; idx++) \
        mock_a_codec_ps[idx] = &mock_a_codecs[idx]; \
    for (idx = 0; idx < num_v_codecs; idx++) \
        mock_v_codec_ps[idx] = &mock_v_codecs[idx]; \
    for (idx = 0; idx < num_muxers; idx++) \
        mock_muxer_ps[idx] = &mock_muxers[idx]; \
    app_cfg_t       *acfg = app_get_global_cfg(); \
    aav_cfg_codec_t *mock_encoders = &acfg->transcoder.output.encoder; \
    mock_encoders->audio.size = mock_encoders->audio.capacity = num_a_codecs; \
    mock_encoders->video.size = mock_encoders->video.capacity = num_v_codecs; \
    mock_encoders->audio.entries = (void **)&mock_a_codec_ps[0]; \
    mock_encoders->video.entries = (void **)&mock_v_codec_ps[0]; \
    acfg->transcoder.output.muxers.size = acfg->transcoder.output.muxers.capacity = num_muxers; \
    acfg->transcoder.output.muxers.entries = (void **)&mock_muxer_ps[0]; \
    aav_cfg_resolution_a_t *_rso_a = &acfg->transcoder.output.resolution.audio; \
    aav_cfg_resolution_v_t *_rso_v = &acfg->transcoder.output.resolution.video; \
    _rso_a->bitrate_kbps.size = _rso_a->bitrate_kbps.capacity = num_rso_a_bitrate; \
    _rso_v->pixels.size = _rso_v->pixels.capacity = num_rso_v_pxl; \
    _rso_v->fps.size = _rso_v->fps.capacity = num_rso_v_fps; \
    _rso_a->bitrate_kbps.entries = &mock_audio_bitrate_kbps[0]; \
    _rso_v->pixels.entries = &mock_video_rso_pixels[0]; \
    _rso_v->fps.entries = &mock_video_framerates[0];

#define UTEST_VALIDATE_VDO_REQ__TEARDOWN \
    acfg->transcoder.output = (aav_cfg_output_t){0}; \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

Ensure(atfp_test__validate_video_req__ok) {
#define OUT_ITEM1 VDO_OUTPUT_ITEM_GEN("dA", "avi", "\"vdo_one\",\"ado_one\"")
#define OUT_ITEM2 VDO_OUTPUT_ITEM_GEN("Er", "m4v", "\"ado_two\",\"vdo_two\"")
#define OUT_ITEM3 VDO_OUTPUT_ITEM_GEN("sN", "ogv", "\"vdo_thri\",\"ado_thri\"")
#define OUT_ITEM4 VDO_OUTPUT_ITEM_GEN("Se", "avi", "\"ado_one\",\"vdo_for\"")
#define UTEST_REQBODY \
    VIDEO_REQ_BODY_GEN( \
        "faceRec0", \
        ELM_ST_A1 "," ELM_ST_A2 "," ELM_ST_A3 "," ELM_ST_V1 "," ELM_ST_V2 "," ELM_ST_V3 "," ELM_ST_V4, \
        OUT_ITEM1 "," OUT_ITEM2 "," OUT_ITEM3 "," OUT_ITEM4 \
    )
#define INIT_EXPR_A_BITRATE \
    { 57, 61 }
#define INIT_EXPR_V_RSO_PXL \
    { \
        {600, 370}, {620, 390}, { 630, 400 } \
    }
#define INIT_EXPR_V_RSO_FPS {18, 17, 16, 20}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        3, 4, 3, 2, INIT_EXPR_A_BITRATE, 3, INIT_EXPR_V_RSO_PXL, 4, INIT_EXPR_V_RSO_FPS, UTEST_REQBODY
    )
    mock_v_codecs[0].name = "libx264rgb";
    mock_v_codecs[1].name = "v410";
    mock_v_codecs[2].name = "flv";
    mock_v_codecs[3].name = "wmv2";
    mock_a_codecs[0].name = "aac";
    mock_a_codecs[1].name = "dca";
    mock_a_codecs[2].name = "alac";
    mock_muxers[0].name = "avi";
    mock_muxers[1].name = "ogv";
    mock_muxers[2].name = "m4v";
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(0));
    json_t     *actual_outputs = json_object_get(mock_spec, "outputs"), *item = NULL;
    const char *actual_version = NULL;
    json_object_foreach(actual_outputs, actual_version, item) {
        assert_that(item, is_not_null);
        json_t *internal = json_object_get(item, "__internal__");
        assert_that(internal, is_not_null);
        json_t *_a_key_item = json_object_get(internal, "audio_key");
        json_t *_v_key_item = json_object_get(internal, "video_key");
        assert_that(_a_key_item, is_not_null);
        assert_that(_v_key_item, is_not_null);
    } // end of loop
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_A_BITRATE
#undef INIT_EXPR_V_RSO_PXL
#undef INIT_EXPR_V_RSO_FPS
#undef UTEST_REQBODY
#undef OUT_ITEM1
#undef OUT_ITEM2
#undef OUT_ITEM3
#undef OUT_ITEM4
} // end of atfp_test__validate_video_req__ok

Ensure(atfp_test__validate_video_req__err_elm_stream) {
#define UTEST_REQBODY \
    VIDEO_REQ_BODY_GEN("someID", "\"some_label\":{\"type\":\"sideband\",\"some_attri\":5678}", "")
#define INIT_EXPR_AV_EMPTY {0}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        1, 1, 1, 1, INIT_EXPR_AV_EMPTY, 1, INIT_EXPR_AV_EMPTY, 1, INIT_EXPR_AV_EMPTY, UTEST_REQBODY
    )
    // subcase #1 : missing attribute
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    json_t *actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "attribute");
        assert_that(actual_err_detail, is_not_null);
    }
    // subcase #2 : unsupported stream type
    json_object_clear(mock_err_info);
    json_object_set_new(
        json_object_get(json_object_get(mock_spec, "elementary_streams"), "some_label"), "attribute",
        json_array()
    );
    err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "type");
        assert_that(actual_err_detail, is_not_null);
    }
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_AV_EMPTY
#undef UTEST_REQBODY
} // end of atfp_test__validate_video_req__err_elm_stream

Ensure(atfp_test__validate_video_req__err_elm_stream_a) {
#define UTEST_REQBODY VIDEO_REQ_BODY_GEN("myResID", ELM_ST_A2 "," ELM_ST_A3, "")
#define INIT_EXPR_A_BITRATE \
    { 54, 61, 67, 71 }
#define INIT_EXPR_AV_EMPTY {0}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        2, 1, 1, 4, INIT_EXPR_A_BITRATE, 1, INIT_EXPR_AV_EMPTY, 1, INIT_EXPR_AV_EMPTY, UTEST_REQBODY
    )
    // subcasse #1 : codec label not matched
    mock_a_codecs[0].name = "xxx_alac";
    mock_a_codecs[1].name = "xxx_dca";
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    json_t *actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "codec");
        assert_that(actual_err_detail, is_not_null);
    }
    // subcasse #2 : unsupported bitrate
    mock_a_codecs[0].name = "alac";
    mock_a_codecs[1].name = "dca";
    json_object_clear(mock_err_info);
    err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "bitrate_kbps");
        assert_that(actual_err_detail, is_not_null);
    }
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_AV_EMPTY
#undef INIT_EXPR_A_BITRATE
#undef UTEST_REQBODY
} // end of atfp_test__validate_video_req__err_elm_stream_a

Ensure(atfp_test__validate_video_req__err_elm_stream_v) {
#define UTEST_REQBODY VIDEO_REQ_BODY_GEN("resID123", ELM_ST_V1 "," ELM_ST_V2, "")
#define INIT_EXPR_V_RSO_PXL \
    { \
        { 1234, 5678 } \
    }
#define INIT_EXPR_V_RSO_FPS \
    { 218, 220 }
#define INIT_EXPR_AV_EMPTY {0}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        1, 2, 1, 1, INIT_EXPR_AV_EMPTY, 1, INIT_EXPR_V_RSO_PXL, 2, INIT_EXPR_V_RSO_FPS, UTEST_REQBODY
    )
    // subcasse #1 : codec label not matched
    mock_v_codecs[0].name = "libxx264";
    mock_v_codecs[1].name = "v410oooo";
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    json_t *actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "codec");
        assert_that(actual_err_detail, is_not_null);
    }
    // subcasse #2 : unsupported resolution
    mock_v_codecs[0].name = "libx264rgb";
    mock_v_codecs[1].name = "v410";
    json_object_clear(mock_err_info);
    err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    actual_err_info = json_object_get(mock_err_info, "elementary_streams");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "height_pixel");
        assert_that(actual_err_detail, is_not_null);
        actual_err_detail = json_object_get(actual_err_info, "framerate");
        assert_that(actual_err_detail, is_not_null);
    }
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_AV_EMPTY
#undef INIT_EXPR_V_RSO_PXL
#undef INIT_EXPR_V_RSO_FPS
#undef UTEST_REQBODY
} // end of  atfp_test__validate_video_req__err_elm_stream_v

Ensure(atfp_test__validate_video_req__err_output) {
#define OUT_ITEM1 VDO_OUTPUT_ITEM_GEN("dA", "avi", "\"vdo_one\",\"ado_one\"")
#define OUT_ITEM2 VDO_OUTPUT_ITEM_GEN("E@", "m4v", "\"ado_two\",\"vdo_two\"")
#define OUT_ITEM3 VDO_OUTPUT_ITEM_GEN("cR", "ogv", "\"vdo_one\",\"ado_two\"")
#define UTEST_REQBODY \
    VIDEO_REQ_BODY_GEN( \
        "myResID", ELM_ST_A1 "," ELM_ST_A2 "," ELM_ST_V1 "," ELM_ST_V2, \
        OUT_ITEM1 "," OUT_ITEM2 "," OUT_ITEM3 \
    )
#define INIT_EXPR_A_BITRATE \
    { 57, 61 }
#define INIT_EXPR_V_RSO_PXL \
    { \
        {600, 370}, {620, 390}, { 630, 400 } \
    }
#define INIT_EXPR_V_RSO_FPS {18, 17, 16, 20}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        2, 2, 2, 2, INIT_EXPR_A_BITRATE, 3, INIT_EXPR_V_RSO_PXL, 4, INIT_EXPR_V_RSO_FPS, UTEST_REQBODY
    )
    mock_v_codecs[0].name = "libx264rgb";
    mock_v_codecs[1].name = "v410";
    mock_a_codecs[0].name = "aac";
    mock_a_codecs[1].name = "dca";
    mock_muxers[0].name = "avi";
    mock_muxers[1].name = "ogC";
    // subcasse #1 : invalid character in version string
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    json_t *actual_err_info = json_object_get(mock_err_info, "outputs");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "version");
        assert_that(actual_err_detail, is_not_null);
    }
    // subcasse #2 : muxer label not matched
    json_object_clear(mock_err_info);
    json_object_del(json_object_get(mock_spec, "outputs"), "E@");
    err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    actual_err_info = json_object_get(mock_err_info, "outputs");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "container");
        assert_that(actual_err_detail, is_not_null);
    }
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_A_BITRATE
#undef INIT_EXPR_V_RSO_PXL
#undef INIT_EXPR_V_RSO_FPS
#undef UTEST_REQBODY
#undef OUT_ITEM1
#undef OUT_ITEM2
#undef OUT_ITEM3
} // end of atfp_test__validate_video_req__err_output

Ensure(atfp_test__validate_video_req__err_output_map_elm_st) {
#define OUT_ITEM1     VDO_OUTPUT_ITEM_GEN("dA", "avi", "\"vdo_one\",\"ado_unknown\"")
#define UTEST_REQBODY VIDEO_REQ_BODY_GEN("myResID", ELM_ST_A1 "," ELM_ST_V1, OUT_ITEM1)
#define INIT_EXPR_A_BITRATE \
    { 57, 61 }
#define INIT_EXPR_V_RSO_PXL \
    { \
        {600, 370}, {620, 390}, { 630, 400 } \
    }
#define INIT_EXPR_V_RSO_FPS {20}
    UTEST_VALIDATE_VDO_REQ__SETUP(
        1, 1, 1, 2, INIT_EXPR_A_BITRATE, 3, INIT_EXPR_V_RSO_PXL, 1, INIT_EXPR_V_RSO_FPS, UTEST_REQBODY
    )
    mock_v_codecs[0].name = "libx264rgb";
    mock_a_codecs[0].name = "aac";
    mock_muxers[0].name = "avi";
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_VIDEO, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(1));
    json_t *actual_err_info = json_object_get(mock_err_info, "outputs");
    assert_that(actual_err_info, is_not_null);
    if (actual_err_info) {
        json_t *actual_err_detail = json_object_get(actual_err_info, "elementary_streams");
        assert_that(actual_err_detail, is_not_null);
    }
    UTEST_VALIDATE_VDO_REQ__TEARDOWN
#undef INIT_EXPR_A_BITRATE
#undef INIT_EXPR_V_RSO_PXL
#undef INIT_EXPR_V_RSO_FPS
#undef UTEST_REQBODY
#undef OUT_ITEM1
} // end of atfp_test__validate_video_req__err_output_map_elm_st

#define UTEST_VALIDATE_IMG_REQ__SETUP(_limit_width, _limit_height, msk_patt_basepath, _req_body_serial) \
    json_t *mock_spec = json_loadb(_req_body_serial, sizeof(_req_body_serial) - 1, 0, NULL); \
    assert_that(mock_spec, is_not_null); \
    if (!mock_spec) \
        return; \
    json_t        *mock_err_info = json_object(); \
    app_cfg_t     *acfg = app_get_global_cfg(); \
    aav_cfg_img_t *mock_imgcfg = &acfg->transcoder.output.image; \
    mock_imgcfg->mask.basepath = msk_patt_basepath; \
    mock_imgcfg->limit = (aav_cfg_resolution_pix_t){.width = _limit_width, .height = _limit_height};

#define UTEST_VALIDATE_IMG_REQ__TEARDOWN \
    acfg->transcoder.output.image = (aav_cfg_img_t){0}; \
    json_decref(mock_spec); \
    json_decref(mock_err_info);

#define OUT_ITEM1     IMG_OUTPUT_ITEM_GEN("Gx", 280, 210, 306, 225, 14, 64, "custom189")
#define OUT_ITEM2     IMG_OUTPUT_ITEM_GEN("jk", 240, 180, 306, 225, 97, 64, "custom190")
#define OUT_ITEM3     IMG_OUTPUT_ITEM_GEN("Dh", 250, 202, 320, 240, 123, 5, "custom191")
#define UTEST_REQBODY IMAGE_REQ_BODY_GEN("myResID", OUT_ITEM1 "," OUT_ITEM2 "," OUT_ITEM3)
Ensure(atfp_test__validate_image_req__ok) {
    UTEST_VALIDATE_IMG_REQ__SETUP(330, 250, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of atfp_test__validate_image_req__ok

Ensure(atfp_test__validate_image_req__ok_spare) {
    UTEST_VALIDATE_IMG_REQ__SETUP(330, 250, "media/data/test/image/mask", UTEST_REQBODY) {
        json_t *outputs_item = json_object_get(mock_spec, "outputs");
        json_t *output_item = json_object_get(outputs_item, "Gx");
        json_object_del(output_item, "scale");
        output_item = json_object_get(outputs_item, "jk");
        json_object_del(output_item, "crop");
        output_item = json_object_get(outputs_item, "Dh");
        json_object_del(output_item, "mask");
    }
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_equal_to(0));
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of atfp_test__validate_image_req__ok_spare
#undef UTEST_REQBODY
#undef OUT_ITEM3
#undef OUT_ITEM2
#undef OUT_ITEM1

#define UTEST_REQBODY IMAGE_REQ_BODY_GEN("myResID", "")
Ensure(atfp_test__validate_image_req__empty) {
    UTEST_VALIDATE_IMG_REQ__SETUP(100, 60, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_not_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    json_t *actual_err_detail = json_object_get(mock_err_info, "outputs");
    assert_that(actual_err_detail, is_not_null);
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of  atfp_test__validate_image_req__empty
#undef UTEST_REQBODY

#define EXPECT_VERSION  "Qe"
#define INVALID_OUTITEM "\"" EXPECT_VERSION "\":{\"junk\":true, \"rate\":9.040029}"
#define UTEST_REQBODY   IMAGE_REQ_BODY_GEN("myResID", INVALID_OUTITEM)
Ensure(atfp_test__validate_image_req__invalid_attri_version) {
    UTEST_VALIDATE_IMG_REQ__SETUP(100, 60, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_not_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    json_t *actual_err_detail = json_object_get(mock_err_info, "output");
    assert_that(actual_err_detail, is_not_null);
    const char *actual_err_version = json_string_value(json_object_get(mock_err_info, "version"));
    assert_that(actual_err_version, is_equal_to_string(EXPECT_VERSION));
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of  atfp_test__validate_image_req__invalid_attri_version
#undef UTEST_REQBODY
#undef INVALID_OUTITEM
#undef EXPECT_VERSION

#define EXPECT_VERSION "jk"
#define OUT_ITEM1      IMG_OUTPUT_ITEM_GEN("Gx", 280, 210, 306, 225, 14, 64, "custom189")
#define OUT_ITEM2      IMG_OUTPUT_ITEM_GEN(EXPECT_VERSION, 240, 180, -1, 250, 9007, 64, "custom190")
#define UTEST_REQBODY  IMAGE_REQ_BODY_GEN("myResID", OUT_ITEM1 "," OUT_ITEM2)
Ensure(atfp_test__validate_image_req__invalid_crop) {
    UTEST_VALIDATE_IMG_REQ__SETUP(330, 248, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_not_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    const char *actual_err_version = json_string_value(json_object_get(mock_err_info, "version"));
    assert_that(actual_err_version, is_equal_to_string(EXPECT_VERSION));
    json_t *actual_err_detail = json_object_get(mock_err_info, "crop");
    assert_that(actual_err_detail, is_not_null);
    if (actual_err_detail) {
        json_t *_err_width = json_object_get(actual_err_detail, "width");
        json_t *_err_height = json_object_get(actual_err_detail, "height");
        json_t *_err_pos_x = json_object_get(actual_err_detail, "x");
        assert_that(_err_width, is_not_null);
        assert_that(_err_height, is_not_null);
        assert_that(_err_pos_x, is_not_null);
    }
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of  atfp_test__validate_image_req__invalid_crop
#undef UTEST_REQBODY
#undef OUT_ITEM2
#undef OUT_ITEM1
#undef EXPECT_VERSION

#define EXPECT_VERSION "jk"
#define OUT_ITEM1      IMG_OUTPUT_ITEM_GEN(EXPECT_VERSION, 240, 180, 230, 170, 100, 32, "custom190")
#define UTEST_REQBODY  IMAGE_REQ_BODY_GEN("myResID", OUT_ITEM1)
Ensure(atfp_test__validate_image_req__invalid_scale) {
    UTEST_VALIDATE_IMG_REQ__SETUP(330, 248, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_not_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    json_t *actual_err_detail = json_object_get(mock_err_info, "scale");
    assert_that(actual_err_detail, is_not_null);
    if (actual_err_detail) {
        json_t *_err_width = json_object_get(actual_err_detail, "width");
        json_t *_err_height = json_object_get(actual_err_detail, "height");
        assert_that(_err_width, is_not_null);
        assert_that(_err_height, is_not_null);
    }
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of  atfp_test__validate_image_req__invalid_scale
#undef UTEST_REQBODY
#undef OUT_ITEM1
#undef EXPECT_VERSION

#define OUT_ITEM1     IMG_OUTPUT_ITEM_GEN("jk", 240, 180, 250, 188, 100, 32, "nonexistMask")
#define UTEST_REQBODY IMAGE_REQ_BODY_GEN("myResID", OUT_ITEM1)
Ensure(atfp_test__validate_image_req__mask_nonexist_pattern) {
    UTEST_VALIDATE_IMG_REQ__SETUP(330, 248, "media/data/test/image/mask", UTEST_REQBODY)
    int err = atfp_validate_transcode_request(APP_FILETYPE_LABEL_IMAGE, mock_spec, mock_err_info);
    assert_that(err, is_not_equal_to(0));
    assert_that(json_object_size(mock_err_info), is_greater_than(0));
    json_t *actual_err_detail = json_object_get(mock_err_info, "mask");
    assert_that(actual_err_detail, is_not_null);
    if (actual_err_detail) {
        json_t *_err_patt = json_object_get(actual_err_detail, "pattern");
        assert_that(_err_patt, is_not_null);
    }
    UTEST_VALIDATE_IMG_REQ__TEARDOWN
} // end of  atfp_test__validate_image_req__mask_nonexist_pattern
#undef UTEST_REQBODY
#undef OUT_ITEM1

#define VERSION_LABEL       "dA"
#define OUT_ITEM1           VDO_OUTPUT_ITEM_GEN(VERSION_LABEL, "avi", "\"vdo_one\",\"ado_one\"")
#define UTEST_REQBODY       VIDEO_REQ_BODY_GEN("myResID", ELM_ST_A1 "," ELM_ST_V1, OUT_ITEM1)
#define OUTPUT_INTERNAL_RAW "{\"video_key\":\"vdo_one\", \"audio_key\":\"ado_one\"}"
Ensure(atfp_test__chk_video_version__editing) {
    const char         *mock_db_row[4] = {VERSION_LABEL, STRINGIFY(240), STRINGIFY(320), STRINGIFY(11)};
    db_query_row_info_t mock_saved_version = {.num_cols = 4, .values = (char **)&mock_db_row[0]};
    json_t             *mock_spec = json_loadb(UTEST_REQBODY, sizeof(UTEST_REQBODY) - 1, 0, NULL);
    json_t *internal_item = json_loadb(OUTPUT_INTERNAL_RAW, sizeof(OUTPUT_INTERNAL_RAW) - 1, 0, NULL);
    {
        json_t *outputs_item = json_object_get(mock_spec, "outputs");
        json_t *output_item = json_object_get(outputs_item, VERSION_LABEL);
        json_object_set_new(output_item, "__internal__", internal_item);
    }
    atfp_validate_req_dup_version(APP_FILETYPE_LABEL_VIDEO, mock_spec, &mock_saved_version);
    json_t *isupdate_item = json_object_get(internal_item, "is_update");
    assert_that(isupdate_item, is_not_null);
    assert_that(json_boolean_value(isupdate_item), is_true);
    json_decref(mock_spec);
} // end of atfp_test__chk_video_version__editing

Ensure(atfp_test__chk_video_version__duplicate) {
    const char         *mock_db_row[4] = {VERSION_LABEL, STRINGIFY(400), STRINGIFY(630), STRINGIFY(20)};
    db_query_row_info_t mock_saved_version = {.num_cols = 4, .values = (char **)&mock_db_row[0]};
    json_t             *mock_spec = json_loadb(UTEST_REQBODY, sizeof(UTEST_REQBODY) - 1, 0, NULL);
    json_t             *outputs_item = json_object_get(mock_spec, "outputs");
    {
        json_t *internal_item = json_loadb(OUTPUT_INTERNAL_RAW, sizeof(OUTPUT_INTERNAL_RAW) - 1, 0, NULL);
        json_t *output_item = json_object_get(outputs_item, VERSION_LABEL);
        json_object_set_new(output_item, "__internal__", internal_item);
    }
    atfp_validate_req_dup_version(APP_FILETYPE_LABEL_VIDEO, mock_spec, &mock_saved_version);
    assert_that(json_object_get(outputs_item, VERSION_LABEL), is_null);
    json_decref(mock_spec);
} // end of atfp_test__chk_video_version__duplicate

Ensure(atfp_test__chk_resource_version__unknown_type) {
    const char *mock_db_row[1] = {
        VERSION_LABEL,
    };
    db_query_row_info_t mock_saved_version = {.num_cols = 1, .values = (char **)&mock_db_row[0]};
    json_t             *mock_spec = json_loadb(UTEST_REQBODY, sizeof(UTEST_REQBODY) - 1, 0, NULL);
    atfp_validate_req_dup_version("unknown-file-type", mock_spec, &mock_saved_version);
    {
        json_t *outputs_item = json_object_get(mock_spec, "outputs");
        json_t *output_item = json_object_get(outputs_item, VERSION_LABEL);
        assert_that(output_item, is_not_null); // still exists
    }
    json_decref(mock_spec);
} // end of atfp_test__chk_resource_version__unknown_type
#undef OUTPUT_INTERNAL_RAW
#undef UTEST_REQBODY
#undef OUT_ITEM1
#undef VERSION_LABEL

#define VERSION_LABEL      "dA"
#define VALID_MASK_PATTERN "custom191"
#define OUT_ITEM1          IMG_OUTPUT_ITEM_GEN(VERSION_LABEL, 250, 202, 320, 240, 123, 5, VALID_MASK_PATTERN)
#define UTEST_REQBODY      IMAGE_REQ_BODY_GEN("myResID", OUT_ITEM1)
Ensure(atfp_test__chk_image_version__editing) {
    const char *mock_db_row[8] = {
        VERSION_LABEL, STRINGIFY(202), STRINGIFY(250), // scale height, width
        STRINGIFY(240), NULL,          // crop height, width, assume the width is the same as the original
                                       // one
        STRINGIFY(123), STRINGIFY(15), // crop position (x,y) , only y-value is changed
        VALID_MASK_PATTERN
    };
    db_query_row_info_t mock_saved_version = {.num_cols = 8, .values = (char **)&mock_db_row[0]};
    json_t             *mock_spec = json_loadb(UTEST_REQBODY, sizeof(UTEST_REQBODY) - 1, 0, NULL);
    json_t             *internal_item = json_object();
    {
        json_t *outputs_item = json_object_get(mock_spec, "outputs");
        json_t *output_item = json_object_get(outputs_item, VERSION_LABEL);
        json_object_set_new(output_item, "__internal__", internal_item);
    }
    atfp_validate_req_dup_version(APP_FILETYPE_LABEL_IMAGE, mock_spec, &mock_saved_version);
    json_t *isupdate_item = json_object_get(internal_item, "is_update");
    assert_that(isupdate_item, is_not_null);
    assert_that(json_boolean_value(isupdate_item), is_true);
    json_decref(mock_spec);
} // end of  atfp_test__chk_image_version__editing

Ensure(atfp_test__chk_image_version__duplicate) {
    const char *mock_db_row[8] = {
        VERSION_LABEL,
        NULL,
        STRINGIFY(250), // scale height, width, assume the height is the same as the original one
        STRINGIFY(240),
        NULL, // crop height, width, assume the width is the same as the original one
        STRINGIFY(123),
        STRINGIFY(5), // crop position (x,y) , only y-value is changed
        VALID_MASK_PATTERN
    };
    db_query_row_info_t mock_saved_version = {.num_cols = 8, .values = (char **)&mock_db_row[0]};
    json_t             *mock_spec = json_loadb(UTEST_REQBODY, sizeof(UTEST_REQBODY) - 1, 0, NULL);
    json_t             *outputs_item = json_object_get(mock_spec, "outputs");
    {
        json_t *output_item = json_object_get(outputs_item, VERSION_LABEL);
        json_object_del(json_object_get(output_item, "scale"), "height");
        json_object_del(json_object_get(output_item, "crop"), "width");
        json_object_set_new(output_item, "__internal__", json_object());
    }
    atfp_validate_req_dup_version(APP_FILETYPE_LABEL_IMAGE, mock_spec, &mock_saved_version);
    assert_that(json_object_get(outputs_item, VERSION_LABEL), is_null);
    json_decref(mock_spec);
} // end of  atfp_test__chk_image_version__duplicate
#undef UTEST_REQBODY
#undef OUT_ITEM1
#undef VERSION_LABEL

TestSuite *app_transcoder_validation_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_test__validate_video_req__ok);
    add_test(suite, atfp_test__validate_video_req__err_elm_stream);
    add_test(suite, atfp_test__validate_video_req__err_elm_stream_a);
    add_test(suite, atfp_test__validate_video_req__err_elm_stream_v);
    add_test(suite, atfp_test__validate_video_req__err_output);
    add_test(suite, atfp_test__validate_video_req__err_output_map_elm_st);
    add_test(suite, atfp_test__validate_image_req__ok);
    add_test(suite, atfp_test__validate_image_req__ok_spare);
    add_test(suite, atfp_test__validate_image_req__empty);
    add_test(suite, atfp_test__validate_image_req__invalid_attri_version);
    add_test(suite, atfp_test__validate_image_req__invalid_crop);
    add_test(suite, atfp_test__validate_image_req__invalid_scale);
    add_test(suite, atfp_test__validate_image_req__mask_nonexist_pattern);
    add_test(suite, atfp_test__chk_video_version__editing);
    add_test(suite, atfp_test__chk_video_version__duplicate);
    add_test(suite, atfp_test__chk_image_version__editing);
    add_test(suite, atfp_test__chk_image_version__duplicate);
    add_test(suite, atfp_test__chk_resource_version__unknown_type);
    return suite;
}
