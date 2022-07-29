#include <assert.h>
#include <string.h>
#include <h2o/memory.h>
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

static void atfp_hls__create_local_workfolder_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    atfp_hls__av_init(hlsproc);
    processor -> data.callback(processor);
} // end of atfp_hls__create_local_workfolder_cb


static void atfp__video_hls__init(atfp_t *processor)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t *asaobj = processor -> data.storage.handle;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local_srcdata = atfp_asa_map_get_localtmp(_map);
    hlsproc->asa_local.loop = asa_local_srcdata->loop;
    {
        // NOTE, if multiple destination file-processors work concurrently,  there should be multiple
        // local storage handles , each of which stores transcoded file for specific spec
        const char *local_tmpfile_basepath = asa_local_srcdata->super.op.mkdir.path.origin;
        const char *version = json_string_value(json_object_get(processor->data.spec, "version"));
        size_t path_sz = strlen(local_tmpfile_basepath) + 1 + sizeof(ATFP_TEMP_TRANSCODING_FOLDER_NAME)
                          + 1 + strlen(version) + 1; // include NULL-terminated byte
        char fullpath[path_sz];
        size_t nwrite = snprintf(&fullpath[0], path_sz, "%s/%s/%s", local_tmpfile_basepath,
                 ATFP_TEMP_TRANSCODING_FOLDER_NAME, version);
        fullpath[nwrite++] = 0x0; // NULL-terminated
        hlsproc->asa_local.super.op.mkdir.path.origin = strndup(&fullpath[0], nwrite);
        hlsproc->asa_local.super.op.mkdir.path.curr_parent = calloc(nwrite, sizeof(char));
    }
    hlsproc->asa_local.super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
    hlsproc->asa_local.super.op.mkdir.cb = atfp_hls__create_local_workfolder_cb;
    ASA_RES_CODE  asa_result = app_storage_localfs_mkdir(&hlsproc->asa_local.super);
    if(asa_result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to issue create-folder operation for internal local tmp buf"));
        processor -> data.callback(processor);
    }
} // end of atfp__video_hls__init


static void atfp__video_hls__deinit(atfp_t *processor)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    char *path = NULL;
    path = hlsproc->asa_local.super.op.mkdir.path.origin;
    if(path) {
        free(path);
        hlsproc->asa_local.super.op.mkdir.path.origin = NULL;
    }
    path = hlsproc->asa_local.super.op.mkdir.path.curr_parent;
    if(path) {
        free(path);
        hlsproc->asa_local.super.op.mkdir.path.curr_parent = NULL;
    }
    atfp_hls__av_deinit((atfp_hls_t *)processor);
    free(processor);
} // end of atfp__video_hls__deinit


static void atfp__video_hls__processing(atfp_t *processor)
{
    processor -> data.callback(processor);
}

static uint8_t  atfp__video_hls__has_done_processing(atfp_t *processor)
{ return 1; }


static atfp_t *atfp__video_hls__instantiate(void) {
    // at this point, `atfp_av_ctx_t` should NOT be incomplete type
    size_t tot_sz = sizeof(atfp_hls_t) + sizeof(atfp_av_ctx_t);
    atfp_hls_t  *out = calloc(0x1, tot_sz);
    char *ptr = (char *)out + sizeof(atfp_hls_t);
    out->av = (atfp_av_ctx_t *) ptr;
    return &out->super;
}

static uint8_t    atfp__video_hls__label_match(const char *label) {
    const char *exp_labels[2] = {"hls", "application/x-mpegURL"};
    return atfp_common__label_match(label, 2, exp_labels);
}

atfp_ops_entry_t  atfp_ops_video_hls = {
    .backend_id = ATFP_BACKEND_LIB__FFMPEG,
    .ops = {
        .init   = atfp__video_hls__init,
        .deinit = atfp__video_hls__deinit,
        .processing  = atfp__video_hls__processing,
        .instantiate = atfp__video_hls__instantiate,
        .label_match = atfp__video_hls__label_match,
        .has_done_processing = atfp__video_hls__has_done_processing,
    },
};
