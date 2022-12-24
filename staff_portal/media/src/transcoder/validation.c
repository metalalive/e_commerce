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
    int is_editing = -1, ret[2] = {0};
    ret[0] = strncmp(resource_type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    ret[1] = strncmp(resource_type, APP_FILETYPE_LABEL_IMAGE, sizeof(APP_FILETYPE_LABEL_IMAGE) - 1);
    if(ret[0] == 0) { // video
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
    } else if(ret[1] == 0) { // image
        json_t *_mask_item  = json_object_get(output_new, "mask");
        json_t *_crop_item  = json_object_get(output_new, "crop");
        json_t *_scale_item = json_object_get(output_new, "scale");
#define   NUM_UINT16_COLUMNS  6
        uint16_t stored[NUM_UINT16_COLUMNS] = {0}, equal[2] = {0};
        for(int idx = 0; idx < NUM_UINT16_COLUMNS; idx++) {
            int column_idx =  1 + idx;
            if(existing->values[column_idx])
                 stored[idx] = (uint16_t) strtoul(existing->values[column_idx], NULL, 10);
        }
        uint16_t newdata[NUM_UINT16_COLUMNS] = {
            (uint16_t)json_integer_value(json_object_get(_scale_item, "height")),
            (uint16_t)json_integer_value(json_object_get(_scale_item, "width")),
            (uint16_t)json_integer_value(json_object_get(_crop_item, "height")),
            (uint16_t)json_integer_value(json_object_get(_crop_item, "width")),
            (uint16_t)json_integer_value(json_object_get(_crop_item, "x")),
            (uint16_t)json_integer_value(json_object_get(_crop_item, "y")),
        };
        equal[0] = memcmp(&stored[0], &newdata[0], sizeof(uint16_t) * NUM_UINT16_COLUMNS) == 0;
#undef    NUM_UINT16_COLUMNS
        const char *msk_patt_new = json_string_value(json_object_get(_mask_item, "pattern"));
        const char *msk_patt_stored = existing->values[7];
        if(msk_patt_new && msk_patt_stored) {
            equal[1] = strncmp(msk_patt_new, msk_patt_stored, strlen(msk_patt_stored)) == 0;
        } else if (!msk_patt_new && !msk_patt_stored) {
            equal[1] = 1;
        }
        is_editing = (!equal[0]) || (!equal[1]);
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
#define  SQL_PATT_VIDEO "EXECUTE IMMEDIATE 'SELECT `version`,`height_pixel`,`width_pixel`,`framerate` FROM" \
       " `transcoded_video_metadata` WHERE `file_id` = ? and `version` IN (%s)' USING FROM_BASE64('%s'), %s;"
#define  SQL_PATT_IMAGE "EXECUTE IMMEDIATE 'SELECT `version`,`scale_h`,`scale_w`,`crop_h`,`crop_w`,`crop_x`,`crop_y`,`mask_patt`" \
       " FROM `transformed_image_metadata` WHERE `file_id` = ? and `version` IN (%s)' USING FROM_BASE64('%s'), %s;"
    const char *out = NULL;
    int ret = strncmp(res_typ, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) {
        out = SQL_PATT_VIDEO;
        *out_sz = sizeof(SQL_PATT_VIDEO);
    } else {
        ret = strncmp(res_typ, APP_FILETYPE_LABEL_IMAGE, sizeof(APP_FILETYPE_LABEL_IMAGE) - 1);
        if(ret == 0) {
            out = SQL_PATT_IMAGE;
            *out_sz = sizeof(SQL_PATT_IMAGE);
        } else {
            fprintf(stderr, "[transcoder][validate] line:%d, unknown resource type:%s \n", __LINE__, res_typ);
            *out_sz = 0;
        }
    }
    return out;
#undef  SQL_PATT_VIDEO
#undef  SQL_PATT_IMAGE
} // end of  atfp_transcoded_version_sql_pattern


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


static int _validate_image_req__cropping (json_t *item, json_t *err_info, aav_cfg_img_t *img_cfg)
{
    json_t *_crop_w_item, *_crop_h_item, *_crop_pos_x_item, *_crop_pos_y_item;
    json_t *err_detail = json_object();
    int  err = 0;
    _crop_w_item = json_object_get(item, "width");
    _crop_h_item = json_object_get(item, "height");
    _crop_pos_x_item = json_object_get(item, "x");
    _crop_pos_y_item = json_object_get(item, "y");
#define  RUN_CODE(j_item, _err_label, _max_limit)  \
    if(j_item) { \
        int _value = json_integer_value(j_item); \
        if(_value <= 0) \
            json_object_set_new(err_detail, _err_label, json_string("zero or negative number")); \
        else if(_value > _max_limit) \
            json_object_set_new(err_detail, _err_label, json_string("exceeding limit")); \
    }
    RUN_CODE(_crop_w_item, "width", img_cfg->limit.width)
    RUN_CODE(_crop_h_item, "height", img_cfg->limit.height)
    RUN_CODE(_crop_pos_x_item,  "x", img_cfg->limit.width)
    RUN_CODE(_crop_pos_y_item,  "y", img_cfg->limit.height)
    if(json_object_size(err_detail) == 0) {
        json_decref(err_detail);
    } else {
        json_object_set_new(err_info, "crop", err_detail);
        err = 1;
    }
    return err;
#undef  RUN_CODE
} // end of  _validate_image_req__cropping

static int _validate_image_req__scaling (json_t *_crop_item, json_t *_scale_item,
        json_t *err_info, aav_cfg_img_t *img_cfg)
{
    int  err = 0;
    json_t *err_detail = json_object();
    json_t *_crop_w_item  = json_object_get(_crop_item, "width");
    json_t *_crop_h_item  = json_object_get(_crop_item, "height");
    json_t *_scale_w_item = json_object_get(_scale_item, "width");
    json_t *_scale_h_item = json_object_get(_scale_item, "height");
#define  RUN_CODE(j1_item, j2_item, _err_label, _max_limit)  \
    if(j1_item) { \
        int _value1 = json_integer_value(j1_item); \
        if(_value1 <= 0) \
            json_object_set_new(err_detail, _err_label, json_string("zero or negative number")); \
        else if(_value1 > _max_limit) \
            json_object_set_new(err_detail, _err_label, json_string("exceeding limit")); \
        if(j2_item) { \
            int _value2 = json_integer_value(j2_item); \
            if(_value2 > 0 && _value1 > _value2) \
                json_object_set_new(err_detail, _err_label, json_string("greater than cropped size")); \
        } \
    }
    RUN_CODE(_scale_w_item, _crop_w_item, "width",  img_cfg->limit.width)
    RUN_CODE(_scale_h_item, _crop_h_item, "height", img_cfg->limit.height)
    if(json_object_size(err_detail) == 0) {
        json_decref(err_detail);
    } else {
        json_object_set_new(err_info, "scale", err_detail);
        err = 1;
    }
    return err;
#undef  RUN_CODE
} // end of  _validate_image_req__scaling

static int _validate_image_req__mask (json_t *item, json_t *err_info, aav_cfg_img_t *img_cfg)
{
    int err = 0, idx = 0;
    json_t *err_detail = json_object();
    const char *_patt_label = json_string_value(json_object_get(item, "pattern"));
    if(_patt_label) {
        size_t patt_label_sz = strlen(_patt_label);
        for(idx = 0; idx < patt_label_sz; idx++) {
            if(isalnum((int)_patt_label[idx]) == 0) {
                json_object_set_new(err_detail, "pattern", json_string("invalid character"));
                break;
            }
        } // end of loop
#define  MASK_INDEX_FILENAME  "index.json"
#define  FILEPATH_PATTERN  "%s/%s"
        if(json_object_size(err_detail) == 0) {
            size_t filepath_sz = sizeof(MASK_INDEX_FILENAME) + sizeof(FILEPATH_PATTERN)
                       + strlen(img_cfg->mask.basepath);
            char filepath[filepath_sz];
            size_t nwrite = snprintf(&filepath[0], filepath_sz, FILEPATH_PATTERN,
                    img_cfg->mask.basepath, MASK_INDEX_FILENAME);
            assert(nwrite < filepath_sz);
            // NOTE: currently there are only few mask patterns in use, so the file names are
            //  recorded in plain text file, if it grows larger then move these records to database.
            json_t *msk_fmap = json_load_file(&filepath[0], 0, NULL);
            if(msk_fmap) {
                json_t *filename_item = json_object_getn(msk_fmap, _patt_label, patt_label_sz);
                if(filename_item && json_is_string(filename_item)) {
                    json_object_set(item, "_patt_fname", filename_item);
                } else {
                    json_object_set_new(err_detail, "pattern", json_string("not exists"));
                }
                json_decref(msk_fmap);
            }
        }
#undef   FILEPATH_PATTERN
#undef   MASK_INDEX_FILENAME
    } // if  _patt_label not null
    if(json_object_size(err_detail) == 0) {
        json_decref(err_detail);
    } else {
        json_object_set_new(err_info, "mask", err_detail);
        err = 1;
    }
    return err;
} // end of  _validate_image_req__mask


static void _atfp_validate_image_request (json_t *spec, json_t *err_info)
{
    const char *_version = NULL;
    json_t *outputs = json_object_get(spec, "outputs"), *output = NULL;
    app_cfg_t  *acfg = app_get_global_cfg();
    aav_cfg_img_t *_imgcfg = &acfg->transcoder.output.image;
    if(!outputs || !json_is_object(outputs) || json_object_size(outputs) == 0) {
        json_object_set_new(err_info, "outputs", json_string("missing field"));
        return;
    } // set limit on max number of transcoding requests
    json_object_foreach(outputs, _version, output) {
        if(!json_is_object(output) || json_object_size(output) == 0) {
            json_object_set_new(err_info, "output", json_string("empty"));
        } else {
            json_t *mask_item  = json_object_get(output, "mask");
            json_t *crop_item  = json_object_get(output, "crop");
            json_t *scale_item = json_object_get(output, "scale");
            if(!mask_item && !crop_item && !scale_item) {
                json_object_set_new(err_info, "output", json_string("missing necessary attributes"));
                goto iter_end;
            }
            if(crop_item && json_is_object(crop_item)) {
                int err = _validate_image_req__cropping (crop_item, err_info, _imgcfg);
                if(err) goto iter_end;
            }
            if(scale_item && json_is_object(scale_item)) {
                int err = _validate_image_req__scaling (crop_item, scale_item, err_info, _imgcfg);
                if(err) goto iter_end;
            }
            if(scale_item && json_is_object(scale_item)) {
                int err = _validate_image_req__mask (mask_item, err_info, _imgcfg);
                if(err) goto iter_end;
            }
        }
iter_end:
        if(json_object_size(err_info) == 0) {
            json_object_set_new(output, "__internal__", json_object());
        } else {
            json_object_set_new(err_info, "version", json_string(_version));
            break;
        }
    } // end of loop
} // end of  _atfp_validate_image_request


int atfp_validate_transcode_request (const char *resource_type, json_t *spec, json_t *err_info)
{
    int ret = strncmp(resource_type, APP_FILETYPE_LABEL_VIDEO, sizeof(APP_FILETYPE_LABEL_VIDEO) - 1);
    if(ret == 0) {
        _atfp_validate_video_request (spec, err_info);
    } else {
        ret = strncmp(resource_type, APP_FILETYPE_LABEL_IMAGE, sizeof(APP_FILETYPE_LABEL_IMAGE) - 1);
        if(ret == 0) {
            _atfp_validate_image_request (spec, err_info);
        } else {
            json_object_set_new(err_info, "transcoder", json_string("unsupported format"));
        }
    }
    return  json_object_size(err_info);
} // end of  atfp_validate_transcode_request
