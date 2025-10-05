#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include "transcoder/cfg_parser.h"

static int parse_cfg_transcoder_muxer(json_t *obj, aav_cfg_input_t *cfg, void *(*finder)(const char *)) {
    json_t *item = NULL;
    int     idx = 0;
    if (!obj || !finder || !cfg || !json_is_array(obj)) {
        goto error;
    }
    size_t num_muxer_cfg = json_array_size(obj);
    h2o_vector_reserve(NULL, &cfg->demuxers, num_muxer_cfg);
    cfg->demuxers.size = num_muxer_cfg;
    json_array_foreach(obj, idx, item) {
        if (!item || !json_is_string(item)) {
            goto error;
        }
        const char *label = json_string_value(item);
        if (!label) {
            goto error;
        }
        void *fmt_mux = finder(label);
        if (!fmt_mux) {
            goto error;
        }
        cfg->demuxers.entries[idx] = fmt_mux;
    } // end of loop
    return 0;
error:
    return -1;
} // end of  parse_cfg_transcoder_muxer

static int parse_cfg_transcoder_codec(json_t *obj, aav_cfg_codec_t *cfg, AVCodec *(*finder)(const char *)) {
    if (!obj || !finder || !cfg || !json_is_object(obj)) {
        goto error;
    }
    json_t *item = NULL;
    int     idx = 0;
    json_t *video_labels = json_object_get(obj, "video");
    json_t *audio_labels = json_object_get(obj, "audio");
    size_t  num_v_labels = json_array_size(video_labels);
    size_t  num_a_labels = json_array_size(audio_labels);
    if (!video_labels || !audio_labels || num_v_labels == 0 || num_a_labels == 0) {
        goto error;
    }
    h2o_vector_reserve(NULL, &cfg->video, num_v_labels);
    h2o_vector_reserve(NULL, &cfg->audio, num_a_labels);
    cfg->video.size = num_v_labels;
    cfg->audio.size = num_a_labels;
    json_array_foreach(video_labels, idx, item) {
        const char *label = json_string_value(item);
        if (!label) {
            goto error;
        }
        void *codec = (void *)finder(label);
        if (!codec) {
            goto error;
        }
        cfg->video.entries[idx] = codec;
    } // end of loop
    json_array_foreach(audio_labels, idx, item) {
        const char *label = json_string_value(item);
        if (!label) {
            goto error;
        }
        void *codec = (void *)finder(label);
        if (!codec) {
            goto error;
        }
        cfg->audio.entries[idx] = codec;
    } // end of loop
    return 0;
error:
    return -1;
} // end of  parse_cfg_transcoder_codec

static int parse_cfg_transcoder_resolution_video(json_t *obj, aav_cfg_resolution_v_t *cfg) {
    if (!obj || !cfg || !json_is_object(obj)) {
        goto error;
    }
    json_t *item = NULL;
    int     idx = 0;
    json_t *pixels = json_object_get(obj, "pixels");
    json_t *fps = json_object_get(obj, "fps");
    size_t  num_pxl_pairs = json_array_size(pixels);
    size_t  num_fps_values = json_array_size(fps);
    if (!pixels || !fps || num_pxl_pairs == 0 || num_fps_values == 0) {
        goto error;
    }
    h2o_vector_reserve(NULL, &cfg->pixels, num_pxl_pairs);
    h2o_vector_reserve(NULL, &cfg->fps, num_fps_values);
    cfg->pixels.size = num_pxl_pairs;
    cfg->fps.size = num_fps_values;
    json_array_foreach(pixels, idx, item) {
        size_t num = json_array_size(item);
        if (num != 2) {
            fprintf(stderr, "[cfg-parser][transcoder] incorrect resolution format, item length:%lu \n", num);
            goto error;
        }
        int width = (int)json_integer_value(json_array_get(item, 0));
        int height = (int)json_integer_value(json_array_get(item, 1));
        if (width <= 0 || height <= 0) {
            fprintf(
                stderr, "[cfg-parser][transcoder] incorrect resolution format, width:%d, height:%d \n", width,
                height
            );
            goto error;
        }
        cfg->pixels.entries[idx].width = (uint32_t)width;
        cfg->pixels.entries[idx].height = (uint32_t)height;
    } // end of loop
    json_array_foreach(fps, idx, item) {
        int value = (int)json_integer_value(item);
        if (value <= 0 || value >= 0xff) {
            fprintf(stderr, "[cfg-parser][transcoder] incorrect framerate :%d \n", value);
            goto error;
        }
        cfg->fps.entries[idx] = (uint8_t)value;
    } // end of loop
    return 0;
error:
    return -1;
} // end of parse_cfg_transcoder_resolution_video

static int parse_cfg_transcoder_resolution_audio(json_t *obj, aav_cfg_resolution_a_t *cfg) {
    if (!obj || !cfg || !json_is_object(obj)) {
        goto error;
    }
    json_t *item = NULL;
    int     idx = 0;
    json_t *bitrate_kbps = json_object_get(obj, "bitrate_kbps");
    size_t  num = json_array_size(bitrate_kbps);
    if (!bitrate_kbps || num == 0) {
        goto error;
    }
    h2o_vector_reserve(NULL, &cfg->bitrate_kbps, num);
    cfg->bitrate_kbps.size = num;
    json_array_foreach(bitrate_kbps, idx, item) {
        int value = (int)json_integer_value(item);
        if (value <= 0 || value >= 0xffff) {
            fprintf(stderr, "[cfg-parser][transcoder] incorrect audio sample rate :%d \n", value);
            goto error;
        }
        cfg->bitrate_kbps.entries[idx] = (uint16_t)value;
    } // end of loop
    return 0;
error:
    return -1;
} // end of parse_cfg_transcoder_resolution_audio

static int parse_cfg_transcoder_resolution_image(json_t *obj, aav_cfg_img_t *cfg) {
    if (!obj || !cfg || !json_is_object(obj))
        goto error;
    json_t *pxl_limit = json_object_get(obj, "pixel_limit");
    json_t *msk_item = json_object_get(obj, "mask");
    if (!pxl_limit || !msk_item || !json_is_object(pxl_limit) || !json_is_object(msk_item)) {
        fprintf(
            stderr,
            "[cfg-parser][transcoder] line:%d, missing attribute `mask` "
            "or `pixel_limit`\n",
            __LINE__
        );
        goto error;
    }
    int         lmt_width = (int)json_integer_value(json_object_get(pxl_limit, "width"));
    int         lmt_height = (int)json_integer_value(json_object_get(pxl_limit, "height"));
    const char *masks_filepath = json_string_value(json_object_get(msk_item, "basepath"));
    if (lmt_width <= 0 || lmt_height <= 0) {
        fprintf(
            stderr, "[cfg-parser][transcoder] line:%d, invalid, lmt_width:%d, lmt_height:%d \n", __LINE__,
            lmt_width, lmt_height
        );
        goto error;
    } else if (!masks_filepath || strlen(masks_filepath) == 0) {
        fprintf(stderr, "[cfg-parser][transcoder] line:%d, invalid masks_filepath \n", __LINE__);
        goto error;
    }
    cfg->limit.width = lmt_width;
    cfg->limit.height = lmt_height;
    cfg->mask.basepath = strdup(masks_filepath);
    return 0;
error:
    return -1;
} // end of parse_cfg_transcoder_resolution_image

static void *app_av_find_input_format(const char *label) { return (void *)av_find_input_format(label); }

static void *app_av_find_output_format(const char *label) {
    return (void *)av_guess_format(label, NULL, NULL);
}

static int parse_cfg_transcoder_input(json_t *obj, aav_cfg_input_t *cfg) {
    int err = 0;
    err = parse_cfg_transcoder_muxer(json_object_get(obj, "demuxers"), cfg, app_av_find_input_format);
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_codec(
        json_object_get(obj, "decoders"), &cfg->decoder, avcodec_find_decoder_by_name
    );
    if (err) {
        goto error;
    }
    return 0;
error:
    return -1;
} // end of parse_cfg_transcoder_input

static int parse_cfg_transcoder_output(json_t *obj, aav_cfg_output_t *cfg) {
    int err = 0;
    err = parse_cfg_transcoder_muxer(
        json_object_get(obj, "muxers"), (aav_cfg_input_t *)cfg, app_av_find_output_format
    );
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_codec(
        json_object_get(obj, "encoders"), &cfg->encoder, avcodec_find_encoder_by_name
    );
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_resolution_video(json_object_get(obj, "video"), &cfg->resolution.video);
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_resolution_audio(json_object_get(obj, "audio"), &cfg->resolution.audio);
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_resolution_image(json_object_get(obj, "image"), &cfg->image);
    if (err) {
        goto error;
    }
    return 0;
error:
    return -1;
} // end of parse_cfg_transcoder_output

int parse_cfg_transcoder(json_t *obj, app_cfg_t *app_cfg) {
    if (!obj || !app_cfg || !json_is_object(obj)) {
        goto error;
    }
    int     err = 0;
    json_t *input = json_object_get(obj, "input");
    json_t *output = json_object_get(obj, "output");
    if (!input || !output || !json_is_object(input) || !json_is_object(output)) {
        goto error;
    }
    err = parse_cfg_transcoder_input(input, &app_cfg->transcoder.input);
    if (err) {
        goto error;
    }
    err = parse_cfg_transcoder_output(output, &app_cfg->transcoder.output);
    if (err) {
        goto error;
    }
    return 0;
error:
    app_transcoder_cfg_deinit(&app_cfg->transcoder);
    return -1;
} // end of parse_cfg_transcoder

static void transcoder_cfg_deinit_common(aav_cfg_input_t *cfg) {
    void **demuxers = cfg->demuxers.entries;
    void **codec_v = cfg->decoder.video.entries;
    void **codec_a = cfg->decoder.audio.entries;
    if (demuxers) {
        free(demuxers);
    }
    if (codec_v) {
        free(codec_v);
    }
    if (codec_a) {
        free(codec_a);
    }
    memset(cfg, 0, sizeof(aav_cfg_input_t));
} // end of transcoder_cfg_deinit_common

void app_transcoder_cfg_deinit(aav_cfg_transcode_t *cfg) {
    transcoder_cfg_deinit_common(&cfg->input);
    transcoder_cfg_deinit_common((aav_cfg_input_t *)&cfg->output);
    aav_cfg_resolution_a_t *rso_a = &cfg->output.resolution.audio;
    aav_cfg_resolution_v_t *rso_v = &cfg->output.resolution.video;
    if (rso_v->pixels.entries)
        free(rso_v->pixels.entries);
    if (rso_v->fps.entries)
        free(rso_v->fps.entries);
    if (rso_a->bitrate_kbps.entries)
        free(rso_a->bitrate_kbps.entries);
    if (cfg->output.image.mask.basepath) {
        free(cfg->output.image.mask.basepath);
        cfg->output.image.mask.basepath = NULL;
    }
    memset(rso_a, 0, sizeof(aav_cfg_resolution_a_t));
    memset(rso_v, 0, sizeof(aav_cfg_resolution_v_t));
} // end of app_transcoder_cfg_deinit
