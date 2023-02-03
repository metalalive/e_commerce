#ifndef MEDIA__TRANSCODER__VIDEO_COMMON_H
#define MEDIA__TRANSCODER__VIDEO_COMMON_H
#ifdef __cplusplus
extern "C" {
#endif
#include "transcoder/file_processor.h"

#define  ATFP_VIDEO__READ_SPEC(_o_spec, _elm_st_map, pix_height, pix_width, fps) \
{ \
    json_t *elm_st_key_obj = json_object_get(json_object_get(_o_spec, "__internal__"), "video_key"); \
    const char *elm_st_key = json_string_value(elm_st_key_obj); \
    json_t *attribute  = json_object_get(json_object_get(_elm_st_map, elm_st_key), "attribute"); \
    pix_height = json_integer_value(json_object_get(attribute, "height_pixel")); \
    pix_width  = json_integer_value(json_object_get(attribute, "width_pixel")); \
    fps        = json_integer_value(json_object_get(attribute, "framerate")); \
}

void  atfp_video__dst_update_metadata(atfp_t *, void *loop);
void  atfp_storage_video_remove_version(atfp_t *, const char *status);
void  atfp_storage_video_create_version(atfp_t *, asa_mkdir_cb_t);
const char *atfp_video__metadata_dbtable_name(void);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_COMMON_H
