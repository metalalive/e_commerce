#include <assert.h>
#include <string.h>
#include <h2o/memory.h>
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

static void atfp_hls__create_local_workfolder_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    int err = hlsproc->internal.op.avctx_init(hlsproc);
    if(!err)
        hlsproc->internal.op.avfilter_init(hlsproc);
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
    hlsproc->internal.op.avctx_deinit(hlsproc);
    free(processor);
} // end of atfp__video_hls__deinit


static void atfp__video_hls__processing(atfp_t *processor)
{
    int ret = 0;
    ASA_RES_CODE result = ASTORAGE_RESULT_COMPLETE;
    atfp_hls_t *hlsproc_dst = NULL, *hlsproc_src = NULL;
    {
        asa_op_base_cfg_t *asa_dst = processor -> data.storage.handle;
        atfp_asa_map_t    *_map = (atfp_asa_map_t *)asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(_map);
        hlsproc_src = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        hlsproc_dst = (atfp_hls_t *)processor;
    }
    while(!ret) {
        ret = hlsproc_dst->internal.op.filter(hlsproc_src->av, hlsproc_dst->av);
        if(ret) { continue; } // may return error (ret < 0), or no more frames to filter (ret == 1)
        int ret2 = 0;
        while(!ret2) {
            ret2 = hlsproc_dst->internal.op.encode(hlsproc_dst->av);
            if(ret2 == 0) {
                ret2 = hlsproc_dst->internal.op.write(hlsproc_dst->av);
            } else if(ret2 == 1) {
                // no more encoded frames to write, break to outer loop for next filtered frame
            }
            if(ret2 < 0) { ret = ret2; }
        }
    } // end of outer loop
    uint8_t  src_done = hlsproc_src->super.ops->has_done_processing(&hlsproc_src->super);
    uint8_t  flush_filt_done = hlsproc_dst->internal.op.has_done_flush_filter(hlsproc_src->av, hlsproc_dst->av);
    uint8_t  flush_enc_done  = hlsproc_dst->internal.op.has_done_flush_encoder(hlsproc_dst->av);
    if(src_done)
        hlsproc_dst->internal.op.filter = atfp_hls__av_filter__finalize_processing;
    if(flush_filt_done)
        hlsproc_dst->internal.op.encode = atfp_hls__av_encode__finalize_processing;
    if(flush_enc_done)
        ret = atfp_hls__av_local_white_finalize(hlsproc_dst->av);
    if(ret == 1) {
        result = hlsproc_dst->internal.op.move_to_storage(hlsproc_dst);
    } else { // ret < 0
        result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    }
    if(result != ASTORAGE_RESULT_ACCEPT)
        processor -> data.callback(processor);
} // end of atfp__video_hls__processing


static uint8_t  atfp__video_hls__has_done_processing(atfp_t *processor)
{
    atfp_hls_t *hlsproc_dst = (atfp_hls_t *)processor;
    return atfp_av__has_done_processing(hlsproc_dst->av);
}


static atfp_t *atfp__video_hls__instantiate(void) {
    // at this point, `atfp_av_ctx_t` should NOT be incomplete type
    size_t tot_sz = sizeof(atfp_hls_t) + sizeof(atfp_av_ctx_t);
    atfp_hls_t  *out = calloc(0x1, tot_sz);
    char *ptr = (char *)out + sizeof(atfp_hls_t);
    out->av = (atfp_av_ctx_t *) ptr;
    out->internal.op.avctx_init   = atfp_hls__av_init;
    out->internal.op.avctx_deinit = atfp_hls__av_deinit;
    out->internal.op.avfilter_init = atfp_hls__avfilter_init;
    out->internal.op.filter  = atfp_hls__av_filter_processing;
    out->internal.op.encode  = atfp_hls__av_encode_processing;
    out->internal.op.write   = atfp_hls__av_local_white;
    out->internal.op.move_to_storage = atfp_hls__try_flush_to_storage;
    out->internal.op.has_done_flush_filter = atfp_av_filter__has_done_flushing;
    out->internal.op.has_done_flush_encoder = atfp_av_encoder__has_done_flushing;
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
