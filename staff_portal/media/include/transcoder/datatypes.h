#ifndef MEDIA__TRANSCODER_DATATYPES_H
#define MEDIA__TRANSCODER_DATATYPES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>

typedef struct {
    H2O_VECTOR(uint16_t)  bitrate_kbps;
} aav_cfg_resolution_a_t;

typedef struct {
    uint32_t width;
    uint32_t height;
} aav_cfg_resolution_pix_t;

typedef struct {
    H2O_VECTOR(aav_cfg_resolution_pix_t) pixels;
    H2O_VECTOR(uint8_t) fps;
} aav_cfg_resolution_v_t;

typedef struct {
    aav_cfg_resolution_pix_t  limit;
    struct {
        char *basepath;
    } mask;
} aav_cfg_img_t;

typedef struct {
    H2O_VECTOR(void *) video;
    H2O_VECTOR(void *) audio;
} aav_cfg_codec_t;

typedef struct {
    H2O_VECTOR(void *) demuxers;
    aav_cfg_codec_t decoder;
} aav_cfg_input_t;

typedef struct {
    H2O_VECTOR(void *) muxers;
    aav_cfg_codec_t encoder;
    struct {
        aav_cfg_resolution_a_t audio;
        aav_cfg_resolution_v_t video;
    } resolution;
    aav_cfg_img_t  image;
} aav_cfg_output_t;

typedef struct {
    aav_cfg_input_t   input;
    aav_cfg_output_t  output;
} aav_cfg_transcode_t;

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER_DATATYPES_H
