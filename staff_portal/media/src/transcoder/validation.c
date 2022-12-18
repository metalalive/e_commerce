#include <ctype.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>

#include "app_cfg.h"
#include "transcoder/file_processor.h"

void  atfp_validate_req_dup_version(const char *resource_type, json_t *spec, db_query_row_info_t *existing)
{
    const char *version_stored = existing->values[0];
    json_t *outputs_toplvl = json_object_get(spec, "outputs");
    json_t *output_new = json_object_get(outputs_toplvl, version_stored);
    json_t *output_internal = json_object_get(output_new, "__internal__");
    int is_editing = -1;
    int ret = strncmp(resource_type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) { // video
        uint16_t height_pxl_stored = (uint16_t) strtoul(existing->values[1], NULL, 10);
        uint16_t width_pxl_stored  = (uint16_t) strtoul(existing->values[2], NULL, 10);
        uint8_t  framerate_stored  = (uint8_t)  strtoul(existing->values[3], NULL, 10);
        const char *video_key = json_string_value(json_object_get(output_internal, "video_key"));
        json_t *elm_streams = json_object_get(spec, "elementary_streams");
        json_t *elm_st_entry = json_object_get(elm_streams, video_key);
        json_t *elm_st_attri = json_object_get(elm_st_entry, "attribute");
        uint16_t  height_pxl_new = (uint16_t) json_integer_value(json_object_get(elm_st_attri, "height_pixel"));
        uint16_t  width_pxl_new  = (uint16_t) json_integer_value(json_object_get(elm_st_attri, "width_pixel"));
        uint8_t   framerate_new  = (uint8_t)  json_integer_value(json_object_get(elm_st_attri, "framerate"));
        uint8_t height_pxl_edit = height_pxl_stored != height_pxl_new;
        uint8_t width_pxl_edit  = width_pxl_stored  != width_pxl_new ;
        uint8_t framerate_edit  = framerate_stored  != framerate_new ;
        is_editing = height_pxl_edit || width_pxl_edit || framerate_edit;
    }
    switch(is_editing) {
        case  1:
            // message-queue consumer (in later step) check this field and optionally rename exising version
            // folder (to stale state, so it would be deleted after new version is transcoded)
            json_object_set_new(output_internal, "is_update", json_true());
            break;
        case  0:
            // discard if the existing version doesn't change all the attributes
            // (no need to transcode again with the same attributes)
            json_object_del(outputs_toplvl, version_stored);
            break;
        case -1:
        default:
            break;
    }
} // end of  atfp_validate_req_dup_version

const char * atfp_transcoded_version_sql_pattern(const char *res_typ, size_t *out_sz)
{
#define  SQL_PATT_VIDEO "EXECUTE IMMEDIATE 'SELECT `version`, `height_pixel`, `width_pixel`, `framerate` FROM" \
       " `transcoded_video_metadata` WHERE `file_id` = ? and `version` IN (%s)' USING FROM_BASE64('%s'), %s;"
    const char *out = NULL;
    int ret = strncmp(res_typ, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) {
        out = SQL_PATT_VIDEO;
        *out_sz = sizeof(SQL_PATT_VIDEO);
    } else {
        fprintf(stderr, "[transcoder][validate] line:%d, unknown resource type:%s \n", __LINE__, res_typ);
        *out_sz = 0;
    }
    return out;
#undef  SQL_PATT_VIDEO
}


#define VALIDATE_CODEC_LABEL_COMMON(codec_type) \
{ \
    const char *codec_name = json_string_value(json_object_get(elm, "codec")); \
    if(codec_name) { \
        uint8_t verified = 0; \
        aav_cfg_codec_t  *encoder = &acfg->transcoder.output.encoder; \
        for(idx = 0; (!verified) && (idx < encoder-> codec_type .size); idx++) { \
            AVCodec *codec = (AVCodec *)encoder-> codec_type .entries[idx]; \
            verified = strncmp(codec->name, codec_name, strlen(codec->name)) == 0; \
        } \
        if(!verified) \
            json_object_set_new(err, "codec", json_string("unknown label")); \
    } else { \
        json_object_set_new(err, "codec", json_string("required")); \
    } \
}

static void _validate_video_req__output_v(json_t *elm, json_t *err_info)
{ // TODO, improve err-info structure
    size_t idx = 0;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    VALIDATE_CODEC_LABEL_COMMON(video);
    json_t *attribute = json_object_get(elm, "attribute");
    int height_pixel = (int) json_integer_value(json_object_get(attribute, "height_pixel"));
    int width_pixel  = (int) json_integer_value(json_object_get(attribute, "width_pixel"));
    int framerate    = (int) json_integer_value(json_object_get(attribute, "framerate"));
    if(height_pixel <= 0)
        json_object_set_new(err, "height_pixel", json_string("has to be positive integer"));
    if(width_pixel <= 0)
        json_object_set_new(err, "width_pixel", json_string("has to be positive integer"));
    if(framerate <= 0)
        json_object_set_new(err, "framerate", json_string("has to be positive integer"));
    if(json_object_size(err) == 0) {
        aav_cfg_resolution_v_t  *rso_v  = &acfg->transcoder.output.resolution.video;
        uint8_t rso_accepted = 0,  fps_accepted = 0;
        for(idx = 0; (!rso_accepted) && (idx < rso_v->pixels.size); idx++) {
            aav_cfg_resolution_pix_t *pix = &rso_v->pixels.entries[idx];
            rso_accepted = (pix->width == width_pixel) && (pix->height == height_pixel);
        }
        for(idx = 0; (!fps_accepted) && (idx < rso_v->fps.size); idx++)
            fps_accepted = (framerate == rso_v->fps.entries[idx]);
        if(!rso_accepted)
            json_object_set_new(err, "height_pixel", json_string("invalid resolution"));
        if(!fps_accepted)
            json_object_set_new(err, "framerate", json_string("invalid"));
    }
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(err_info, "elementary_streams", err);
    }
} // end of _validate_video_req__output_v

static void _validate_video_req__output_a(json_t *elm, json_t *err_info)
{
    size_t idx = 0;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    VALIDATE_CODEC_LABEL_COMMON(audio);
    json_t *attribute = json_object_get(elm, "attribute");
    int bitrate_kbps = (int) json_integer_value(json_object_get(attribute, "bitrate_kbps"));
    if(bitrate_kbps <= 0)
        json_object_set_new(err, "bitrate_kbps", json_string("has to be positive integer"));
    if(json_object_size(err) == 0) {
        aav_cfg_resolution_a_t  *rso_a  = &acfg->transcoder.output.resolution.audio;
        uint8_t accepted = 0;
        for(idx = 0; (!accepted) && (idx < rso_a->bitrate_kbps.size); idx++)
           accepted = bitrate_kbps == rso_a->bitrate_kbps.entries[idx];
        if(!accepted)
            json_object_set_new(err, "bitrate_kbps", json_string("invalid bitrate"));
    }
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(err_info, "elementary_streams", err);
    }
} // end of _validate_video_req__output_a


static void _validate_video_req__elm_streams (json_t *elm_streams, json_t *err_info)
{
    const char *key = NULL;
    json_t *elm_entry = NULL;
    if(!elm_streams || !json_is_object(elm_streams) || json_object_size(elm_streams) == 0) {
        json_object_set_new(err_info, "elementary_streams", json_string("missing field"));
        return;
    }
    json_object_foreach(elm_streams, key, elm_entry) {
        const char *st_type = json_string_value(json_object_get(elm_entry, "type"));
        const char *err_msg = NULL, *err_field = NULL;
        if(!key) {
            err_field = "key";       err_msg   = "missing";
        } else if(!st_type) {
            err_field = "type";      err_msg   = "missing";
        } else if(!json_object_get(elm_entry, "attribute")) {
            err_field = "attribute"; err_msg   = "missing";
        } else if(strncmp(st_type,"video",5) == 0) {
            _validate_video_req__output_v(elm_entry, err_info);
        } else if(strncmp(st_type,"audio",5) == 0) {
            _validate_video_req__output_a(elm_entry, err_info);
        } else {
            err_field = "type";     err_msg   = "unsupported";
        } // TODO, support subtitle and other types of streams
        if(err_msg && err_field) {
            json_t *err_detail = json_object_get(err_info, "elementary_streams");
            if(!err_detail) {
                err_detail = json_object();
                json_object_set_new(err_info, "elementary_streams", err_detail);
            }
            json_object_set_new(err_detail, err_field, json_string(err_msg));
        }
        if(json_object_size(err_info) > 0)
            break;
    } // end of elementary-stream-entry loop
} // end of _validate_video_req__elm_streams

static void _atfp_validate_video_req__outputs_elm_st_map(json_t *output, json_t *elm_st_dict, json_t *err)
{
    json_t *elm_st_keys = json_object_get(output, "elementary_streams");
    if(!json_is_array(elm_st_keys)) {
        json_object_set_new(err, "elementary_streams", json_string("unknown streams to mux"));
        return;
    } 
    int idx = 0;
    json_t *key_item = NULL;
    uint8_t audio_stream_included = 0, video_stream_included = 0;
    char *audio_stream_key = NULL, *video_stream_key = NULL;
    json_array_foreach(elm_st_keys, idx, key_item) {
        const char *key = json_string_value(key_item);
        json_t *elm_entry = json_object_get(elm_st_dict, key);
        if(!elm_entry) { continue; }
        const char *st_type = json_string_value(json_object_get(elm_entry, "type"));
        if(strncmp(st_type,"audio",5) == 0) {
            audio_stream_key = audio_stream_key ? audio_stream_key: strdup(key);
            audio_stream_included++;
        } else if(strncmp(st_type,"video",5) == 0) {
            video_stream_key = video_stream_key ? video_stream_key: strdup(key);
            video_stream_included++;
        }
    }
    if(audio_stream_included == 1 && video_stream_included == 1) {
        json_t *internal = json_object();
        json_object_set_new(internal, "audio_key", json_string(audio_stream_key));
        json_object_set_new(internal, "video_key", json_string(video_stream_key));
        json_object_set_new(output, "__internal__", internal);
    } else { // TODO, does the app have to support 2 audio/video streams in the same media container ?
        json_object_set_new(err, "elementary_streams",
                json_string("each output item should have exact one audio stream and exact one video stream to mux"));
    }
    if(audio_stream_key) { free(audio_stream_key); }
    if(video_stream_key) { free(video_stream_key); }
} // end of _atfp_validate_video_req__outputs_elm_st_map

static  void _validate_video_req__outputs(json_t *outputs, json_t *elm_streams, json_t *err_info)
{
    if(!outputs || !json_is_object(outputs) || json_object_size(outputs) == 0) {
        json_object_set_new(err_info, "outputs", json_string("missing field"));
        return;
    } // TODO, set limit on max number of transcoding requests
    int idx = 0;
    const char *version = NULL;
    json_t *output = NULL;
    json_t *err = json_object();
    app_cfg_t  *acfg = app_get_global_cfg();
    json_object_foreach(outputs, version, output) {
        const char *container = json_string_value(json_object_get(output, "container"));
        if(strlen(version) == APP_TRANSCODED_VERSION_SIZE) {
            int err_ret = 0;
            for(idx = 0; (!err_ret) && (idx < APP_TRANSCODED_VERSION_SIZE); idx++)
                err_ret = isalnum((int)version[idx]) == 0;
            if(err_ret)
                json_object_set_new(err, "version", json_string("contains non-alphanumeric charater"));
        } else {
            json_object_set_new(err, "version", json_string("invalid length"));
        }
        uint8_t muxer_accepted = 0;
        for(idx = 0; (!muxer_accepted) && (idx < acfg->transcoder.output.muxers.size); idx++) {
            AVOutputFormat *muxer = (AVOutputFormat *) acfg->transcoder.output.muxers.entries[idx];
            muxer_accepted = strncmp(container, muxer->name, strlen(muxer->name)) == 0;
        }
        if(!muxer_accepted)
            json_object_set_new(err, "container", json_string("unknown muxer type"));
        _atfp_validate_video_req__outputs_elm_st_map(output, elm_streams, err);
        if(json_object_size(err) > 0)
            break;
    } // end of output-info iteration
    if(json_object_size(err) == 0) {
        json_decref(err);
    } else {
        json_object_set_new(err_info, "outputs", err);
    }
} // end of _validate_video_req__outputs

static void _atfp_validate_video_request (json_t *spec, json_t *err_info)
{
    _validate_video_req__elm_streams(json_object_get(spec, "elementary_streams"), err_info);
    if(json_object_size(err_info) == 0)
        _validate_video_req__outputs( json_object_get(spec, "outputs"),
                json_object_get(spec, "elementary_streams"), err_info );
}


int atfp_validate_transcode_request (const char *resource_type, json_t *spec, json_t *err_info)
{
    int ret = strncmp(resource_type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) {
        _atfp_validate_video_request (spec, err_info);
    } else { // TODO, support image transcoding (e.g. resize, crop)
        json_object_set_new(err_info, "transcoder", json_string("unsupported format"));
    }
    return  json_object_size(err_info);
} // end of  atfp_validate_transcode_request
