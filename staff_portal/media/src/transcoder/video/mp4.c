#include "transcoder/file_processor.h"

#define   PREFETCHED_HEADER_FILENAME    "prefetched_header"

// {
//             size_t filepath_sz = strlen(basepath) + 1 + strlen(PREFETCHED_HEADER_FILENAME) + 1; // include NULL-terminated byte
//             char filepath[filepath_sz];
//             size_t nwrite = snprintf(&filepath[0], filepath_sz, "%s/%s", basepath, PREFETCHED_HEADER_FILENAME);
//             filepath[nwrite++] = 0x0;
//         close(cfg->cb_args.entries[4]);
// }


static void atfp__video_mp4__init(atfp_t *processor)
{
    processor->transcoded_info = json_array();
    processor -> data.callback(processor);
} // end of atfp__video_mp4__init

static void atfp__video_mp4__deinit(atfp_t *processor)
{
    if(processor->transcoded_info) {
        json_decref(processor->transcoded_info);
        processor->transcoded_info = NULL;
    }
    free(processor);
} // end of atfp__video_mp4__deinit

static void atfp__video_mp4__processing(atfp_t *processor)
{
    json_t  *item = json_object();
    json_object_set_new(item, "filename", json_string("fake_transcoded_file.mp4"));
    json_object_set_new(item, "size", json_integer(8193));
    json_object_set_new(item, "checksum", json_string("f09d77e32572b562863518c6"));
    json_array_append_new(processor->transcoded_info, item);
    processor -> data.callback(processor);
} // end of atfp__video_mp4__processing


atfp_ops_entry_t  atfp_ops_video_mp4 = {
    .mimetype = "video/mp4",
    .ops = {
        .init = atfp__video_mp4__init,
        .deinit = atfp__video_mp4__deinit,
        .processing = atfp__video_mp4__processing,
    },
};
