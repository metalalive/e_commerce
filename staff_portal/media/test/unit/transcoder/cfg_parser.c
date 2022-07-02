#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include "transcoder/cfg_parser.h"

Ensure(transcoder_cfg_test_empty_setting) {
#define  TEST_SERIALIZED_JSON  "{\"input\": {\"demuxers\":null, \"decoders\": []}, \"output\": {}}"
    app_cfg_t  acfg = {0};
    json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
    int err = parse_cfg_transcoder(obj, &acfg);
    assert_that(err, is_not_equal_to(0));
    json_decref(obj);
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_empty_setting

Ensure(transcoder_cfg_test_invalid_demuxer) {
#define  TEST_SERIALIZED_JSON   "{\"input\": {\"demuxers\":[\"wmv\",\"mkv\",\"ogg\"]," \
    " \"decoders\": []}, \"output\": {}}"
    app_cfg_t  acfg = {0};
    AVInputFormat ifmts[2] = {0};
    json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
    expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("wmv")));
    expect(av_find_input_format, will_return(&ifmts[1]),  when(short_name, is_equal_to_string("mkv")));
    expect(av_find_input_format, will_return(NULL),  when(short_name, is_equal_to_string("ogg")));
    int err = parse_cfg_transcoder(obj, &acfg);
    assert_that(err, is_not_equal_to(0));
    json_decref(obj);
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_invalid_demuxer

Ensure(transcoder_cfg_test_invalid_decoder) {
    app_cfg_t  acfg = {0};
    AVInputFormat ifmts[1] = {0};
    AVCodec mock_decoders[3] = {0};
#define  TEST_SERIALIZED_JSON   "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": []}, \"output\": {}}"
    {
        json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
        expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4a")));
        int err = parse_cfg_transcoder(obj, &acfg);
        assert_that(err, is_not_equal_to(0));
        json_decref(obj);
    }
#undef  TEST_SERIALIZED_JSON
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], " \
    " \"decoders\": {\"video\":[\"h263p\",\"av1\"], \"audio\":[\"aac\",\"aptx\"]}}, \"output\": {}}"
    {
        json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
        expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4a")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("h263p")));
        expect(avcodec_find_decoder_by_name, will_return(NULL),  when(name, is_equal_to_string("av1")));
        int err = parse_cfg_transcoder(obj, &acfg);
        assert_that(err, is_not_equal_to(0));
        expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4a")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("h263p")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[1]),  when(name, is_equal_to_string("av1")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[2]),  when(name, is_equal_to_string("aac")));
        expect(avcodec_find_decoder_by_name, will_return(NULL),  when(name, is_equal_to_string("aptx")));
        err = parse_cfg_transcoder(obj, &acfg);
        assert_that(err, is_not_equal_to(0));
        json_decref(obj);
    }
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_invalid_decoder

Ensure(transcoder_cfg_test_invalid_muxer) {
    app_cfg_t  acfg = {0};
    AVInputFormat  ifmts[1] = {0};
    AVOutputFormat ofmts[2] = {0};
    AVCodec mock_decoders[2] = {0};
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4axxx\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\",\"mpegts\",\"webm\"]}}"
    {
        json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
        expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4axxx")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("av1")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[1]),  when(name, is_equal_to_string("ac3")));
        expect(av_guess_format, will_return(&ofmts[0]),  when(short_name, is_equal_to_string("hls")));
        expect(av_guess_format, will_return(&ofmts[1]),  when(short_name, is_equal_to_string("mpegts")));
        expect(av_guess_format, will_return(NULL),       when(short_name, is_equal_to_string("webm")));
        int err = parse_cfg_transcoder(obj, &acfg);
        assert_that(err, is_not_equal_to(0));
        json_decref(obj);
    }
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_invalid_muxer

Ensure(transcoder_cfg_test_invalid_encoder) {
    app_cfg_t  acfg = {0};
    AVInputFormat  ifmts[1] = {0};
    AVOutputFormat ofmts[1] = {0};
    AVCodec mock_decoders[4] = {0};
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"dca\",\"wmav1\"]}}}"
    {
        json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
        expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4a")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("av1")));
        expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[1]),  when(name, is_equal_to_string("ac3")));
        expect(av_guess_format, will_return(&ofmts[0]),  when(short_name, is_equal_to_string("hls")));
        expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[2]),  when(name, is_equal_to_string("hevc")));
        expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[3]),  when(name, is_equal_to_string("dca")));
        expect(avcodec_find_encoder_by_name, will_return(NULL),  when(name, is_equal_to_string("wmav1")));
        int err = parse_cfg_transcoder(obj, &acfg);
        assert_that(err, is_not_equal_to(0));
        json_decref(obj);
    }
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_invalid_encoder

static void _test_transcoder_cfg_test_invalid_resolution (const char *serialized_json)
{
    app_cfg_t  acfg = {0};
    AVInputFormat  ifmts[1] = {0};
    AVOutputFormat ofmts[1] = {0};
    AVCodec mock_decoders[4] = {0};
    json_t *obj = json_loads(serialized_json, 0, NULL);
    expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("m4a")));
    expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("av1")));
    expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[1]),  when(name, is_equal_to_string("ac3")));
    expect(av_guess_format, will_return(&ofmts[0]),  when(short_name, is_equal_to_string("hls")));
    expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[2]),  when(name, is_equal_to_string("hevc")));
    expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[3]),  when(name, is_equal_to_string("dca")));
    int err = parse_cfg_transcoder(obj, &acfg);
    assert_that(err, is_not_equal_to(0));
    json_decref(obj);
} // end of _test_transcoder_cfg_test_invalid_resolution

Ensure(transcoder_cfg_test_invalid_resolution) {
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"dca\"]}, " \
    " \"video\":{\"pixels\":[[123,456],[789]], \"fps\":[12,0]} }}"
    _test_transcoder_cfg_test_invalid_resolution(TEST_SERIALIZED_JSON);
#undef  TEST_SERIALIZED_JSON
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"dca\"]}, " \
    " \"video\":{\"pixels\":[[123,456],[78,-9]], \"fps\":[12,0]} }}"
    _test_transcoder_cfg_test_invalid_resolution(TEST_SERIALIZED_JSON);
#undef  TEST_SERIALIZED_JSON
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"dca\"]}, " \
    " \"video\":{\"pixels\":[[123,456],[78,9]], \"fps\":[12,0]} }}"
    _test_transcoder_cfg_test_invalid_resolution(TEST_SERIALIZED_JSON);
#undef  TEST_SERIALIZED_JSON
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"m4a\"], \"decoders\": {\"video\":[\"av1\"], \"audio\":[\"ac3\"]}}, " \
    " \"output\": {\"muxers\":[\"hls\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"dca\"]}, " \
    " \"video\":{\"pixels\":[[123,456],[78,9]], \"fps\":[12,15]}, \"audio\":{\"bitrate_kbps\":[24, -96]}  }}"
    _test_transcoder_cfg_test_invalid_resolution(TEST_SERIALIZED_JSON);
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_invalid_resolution


Ensure(transcoder_cfg_test_ok) {
    app_cfg_t  acfg = {0};
    AVInputFormat  ifmts[1] = {0};
    AVOutputFormat ofmts[1] = {0};
    AVCodec mock_decoders[4] = {0};
#define  TEST_SERIALIZED_JSON \
    "{\"input\": {\"demuxers\":[\"mp4\"], \"decoders\": {\"video\":[\"h264\"], \"audio\":[\"aac\"]}}, " \
    " \"output\": {\"muxers\":[\"webm\"], \"encoders\": {\"video\":[\"hevc\"], \"audio\":[\"ac3\"]}, " \
    " \"video\":{\"pixels\":[[240,360],[78,49], [540,450]], \"fps\":[15,19]}, \"audio\":{\"bitrate_kbps\":[48, 80]}  }}"
    json_t *obj = json_loads(TEST_SERIALIZED_JSON, 0, NULL);
    expect(av_find_input_format, will_return(&ifmts[0]),  when(short_name, is_equal_to_string("mp4")));
    expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[0]),  when(name, is_equal_to_string("h264")));
    expect(avcodec_find_decoder_by_name, will_return(&mock_decoders[1]),  when(name, is_equal_to_string("aac")));
    expect(av_guess_format, will_return(&ofmts[0]),  when(short_name, is_equal_to_string("webm")));
    expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[2]),  when(name, is_equal_to_string("hevc")));
    expect(avcodec_find_encoder_by_name, will_return(&mock_decoders[3]),  when(name, is_equal_to_string("ac3")));
    int err = parse_cfg_transcoder(obj, &acfg);
    assert_that(err, is_equal_to(0));
    {
        aav_cfg_input_t  *cfg_in  = &acfg.transcoder.input;
        aav_cfg_output_t *cfg_out = &acfg.transcoder.output;
        assert_that(cfg_in->demuxers.size, is_equal_to(1));
        assert_that(cfg_in->demuxers.entries[0], is_equal_to(&ifmts[0]));
        assert_that(cfg_out->muxers.size, is_equal_to(1));
        assert_that(cfg_out->muxers.entries[0], is_equal_to(&ofmts[0]));
        assert_that(cfg_in->decoder.video.size,  is_equal_to(1));
        assert_that(cfg_in->decoder.audio.size,  is_equal_to(1));
        assert_that(cfg_out->encoder.video.size, is_equal_to(1));
        assert_that(cfg_out->encoder.audio.size, is_equal_to(1));
        assert_that(cfg_in->decoder.video.entries[0], is_equal_to(&mock_decoders[0]));
        assert_that(cfg_in->decoder.audio.entries[0], is_equal_to(&mock_decoders[1]));
        assert_that(cfg_out->encoder.video.entries[0], is_equal_to(&mock_decoders[2]));
        assert_that(cfg_out->encoder.audio.entries[0], is_equal_to(&mock_decoders[3]));
        assert_that(cfg_out->resolution.video.pixels.size, is_equal_to(3));
        assert_that(cfg_out->resolution.video.pixels.entries[2].width , is_equal_to(540));
        assert_that(cfg_out->resolution.video.pixels.entries[2].height, is_equal_to(450));
        assert_that(cfg_out->resolution.video.fps.size, is_equal_to(2));
        assert_that(cfg_out->resolution.video.fps.entries[0], is_equal_to(15));
        assert_that(cfg_out->resolution.video.fps.entries[1], is_equal_to(19));
        assert_that(cfg_out->resolution.audio.bitrate_kbps.size, is_equal_to(2));
        assert_that(cfg_out->resolution.audio.bitrate_kbps.entries[0], is_equal_to(48));
        assert_that(cfg_out->resolution.audio.bitrate_kbps.entries[1], is_equal_to(80));
    }
    app_transcoder_cfg_deinit(&acfg.transcoder);
    json_decref(obj);
#undef  TEST_SERIALIZED_JSON
} // end of transcoder_cfg_test_ok


TestSuite *app_transcoder_cfg_parser_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, transcoder_cfg_test_empty_setting);
    add_test(suite, transcoder_cfg_test_invalid_demuxer);
    add_test(suite, transcoder_cfg_test_invalid_decoder);
    add_test(suite, transcoder_cfg_test_invalid_muxer);
    add_test(suite, transcoder_cfg_test_invalid_encoder);
    add_test(suite, transcoder_cfg_test_invalid_resolution);
    add_test(suite, transcoder_cfg_test_ok);
    return suite;
} // end of app_transcoder_cfg_parser_tests

