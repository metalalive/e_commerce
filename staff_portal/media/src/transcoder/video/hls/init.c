#include <assert.h>
#include <string.h>
#include <h2o/memory.h>

#include "storage/cfg_parser.h"
#include "transcoder/video/common.h"
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

static void atfp_hls__create_local_workfolder_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // TODO, check whether the folder was already creaated, delete all the files in it if exists
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    int err = hlsproc->internal.op.avctx_init(hlsproc);
    if(!err)
        err = hlsproc->internal.op.avfilter_init(hlsproc);
    if(!err)
        processor->transfer.dst.info = json_object();
    processor -> data.callback(processor);
} // end of atfp_hls__create_local_workfolder_cb


static void atfp__video_hls__init(atfp_t *processor)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t *asa_dst = processor -> data.storage.handle;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local_srcdata =  atfp_asa_map_get_localtmp(_map);
    asa_op_localfs_cfg_t  *asa_local_dstdata = &hlsproc->asa_local;
    if(asa_dst->op.write.src_max_nbytes == 0 || !asa_dst->op.write.src) {
        json_object_set_new(processor->data.error, "storage", json_string("[hls] no write buffer provided in asaobj"));
        processor -> data.callback(processor);
        return;
    } else if (processor->transfer.dst.flags) {
        json_object_set_new(processor->data.error, "transcoder", json_string("[hls] asaobj is not cleaned"));
        processor -> data.callback(processor);
        return;
    } {
        void **cb_args_entries = calloc(NUM_USRARGS_HLS_ASA_LOCAL, sizeof(void *));
        cb_args_entries[ATFP_INDEX__IN_ASA_USRARG]   = (void *) processor;
        cb_args_entries[ASAMAP_INDEX__IN_ASA_USRARG] = (void *) _map;
        asa_local_dstdata->super.cb_args.entries = cb_args_entries;
        asa_local_dstdata->super.cb_args.size = NUM_USRARGS_HLS_ASA_LOCAL;
    } {
        // NOTE, if multiple destination file-processors work concurrently,  there should be multiple
        // local storage handles , each of which stores transcoded file for specific spec
        const char *local_tmpfile_basepath = asa_local_srcdata->super.op.mkdir.path.origin;
        const char *_version = processor->data.version;
        size_t path_sz = strlen(local_tmpfile_basepath) + 1 + sizeof(ATFP__TEMP_TRANSCODING_FOLDER_NAME)
                          + 1 + strlen(_version) + 1; // include NULL-terminated byte
        char fullpath[path_sz];
        size_t nwrite = snprintf(&fullpath[0], path_sz, "%s/%s/%s", local_tmpfile_basepath,
                 ATFP__TEMP_TRANSCODING_FOLDER_NAME, _version);
        fullpath[nwrite++] = 0x0; // NULL-terminated
        asa_local_dstdata->super.op.mkdir.path.origin = strndup(&fullpath[0], nwrite);
        asa_local_dstdata->super.op.mkdir.path.curr_parent = calloc(nwrite, sizeof(char));
    } {
        size_t playlist_name_sz = sizeof(HLS_PLAYLIST_FILENAME) - 1;
        size_t pktmap_name_sz = sizeof(HLS_FMP4_FILENAME) - 1;
        size_t segment_name_sz = sizeof(HLS_SEGMENT_FILENAME_PREFIX) - 1 + HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS;
        size_t filename_max_sz = MAX(playlist_name_sz, pktmap_name_sz);
        filename_max_sz = MAX(filename_max_sz, segment_name_sz) + 2; // extra slash char, and NUL-terminated char
        size_t fullpath_sz_local = strlen(asa_local_dstdata->super.op.mkdir.path.origin) + filename_max_sz;
        size_t fullpath_sz_dst   = strlen(asa_dst->op.mkdir.path.origin) + filename_max_sz;
        char  *asa_dst_fullpath_buf = calloc(fullpath_sz_dst, sizeof(char));
        char  *asa_local_fullpath_buf = calloc(fullpath_sz_local, sizeof(char));
        hlsproc->internal.segment = (atfp_segment_t) {
            .filename = {
                .prefix = {
                    .data = strdup(HLS_SEGMENT_FILENAME_PREFIX),
                    .sz = sizeof(HLS_SEGMENT_FILENAME_PREFIX) - 1,
                },
                .pattern = {
                    .data = strdup(HLS_SEGMENT_FILENAME_NUM_FORMAT),
                    .sz = sizeof(HLS_SEGMENT_FILENAME_NUM_FORMAT) - 1,
                    .max_num_digits = (uint8_t) HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS
                },
            },
            .fullpath = {
                ._asa_local = {.sz = fullpath_sz_local, .data = asa_local_fullpath_buf},
                ._asa_dst = {.sz = fullpath_sz_dst, .data = asa_dst_fullpath_buf},
            },
            .checksum = {0}, .transfer = {0} // implicitly reset `curr_idx` field to 0
        };
    } {
        asa_local_dstdata->loop = asa_local_srcdata->loop;
        asa_local_dstdata->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_local_dstdata->super.op.mkdir.cb = atfp_hls__create_local_workfolder_cb;
    }
    ASA_RES_CODE  asa_result = app_storage_localfs_mkdir(&asa_local_dstdata->super, 1);
    if(asa_result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to issue create-folder operation for internal local tmp buf"));
        processor -> data.callback(processor);
    }
} // end of atfp__video_hls__init


#define  DEINIT_IF_EXISTS(var) \
    if(var) { \
        free(var); \
        (var) = NULL; \
    }
static  void  atfp_hls__asalocal_closefile_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    DEINIT_IF_EXISTS(asaobj->cb_args.entries);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.prefix);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.origin);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.curr_parent);
    DEINIT_IF_EXISTS(processor);
}
static  void  atfp_hls__asaremote_closefile_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.prefix);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.origin);
    DEINIT_IF_EXISTS(asaobj->op.mkdir.path.curr_parent);
    DEINIT_IF_EXISTS(asaobj);
}

static  uint8_t atfp__video_hls__deinit(atfp_t *processor)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *)processor;
    asa_op_base_cfg_t     *asa_dst = processor -> data.storage.handle;
    asa_op_localfs_cfg_t  *asa_local_dstdata = &hlsproc->asa_local;
    atfp_segment_t  *seg_cfg = &hlsproc->internal.segment;
    hlsproc->internal.op.avctx_deinit(hlsproc);
    char *tmp = NULL;
    // Note in this file processor, dst_path points to reference of seg_cfg->fullpath._asa_dst.data
    tmp = asa_dst->op.open.dst_path;
    if(tmp == seg_cfg->fullpath._asa_dst.data)
        asa_dst->op.open.dst_path = NULL;
    tmp = asa_local_dstdata->super.op.open.dst_path;
    if(tmp == seg_cfg->fullpath._asa_local.data)
        asa_local_dstdata->super.op.open.dst_path = NULL;
    DEINIT_IF_EXISTS(seg_cfg->fullpath._asa_dst.data);
    DEINIT_IF_EXISTS(seg_cfg->fullpath._asa_local.data);
    DEINIT_IF_EXISTS(seg_cfg->filename.prefix.data);
    DEINIT_IF_EXISTS(seg_cfg->filename.pattern.data);
    DEINIT_IF_EXISTS(seg_cfg-> rdy_list.entries);
    if(processor->transfer.dst.info) {
        json_decref(processor->transfer.dst.info);
        processor->transfer.dst.info = NULL;
    }
    // ensure file-descriptors in asa-local and asa-dst are closed
    ASA_RES_CODE  asa_result;
    uint32_t asaobj_flags = processor->transfer.dst.flags;
    uint8_t  asa_local_open  = (asaobj_flags & (1 << ATFP_TRANSFER_FLAG__ASALOCAL_OPEN)) != 0;
    uint8_t  asa_remote_open = (asaobj_flags & (1 << ATFP_TRANSFER_FLAG__ASAREMOTE_OPEN)) != 0;
    if (asa_local_open) {
        asa_local_dstdata->super.op.close.cb = atfp_hls__asalocal_closefile_cb;
        asa_result = app_storage_localfs_close(&asa_local_dstdata->super);
        if(asa_result != ASTORAGE_RESULT_ACCEPT) {
            atfp_hls__asalocal_closefile_cb(&asa_local_dstdata->super, ASTORAGE_RESULT_COMPLETE);
            asa_local_open = 0;
        }
    } else {
        atfp_hls__asalocal_closefile_cb(&asa_local_dstdata->super, ASTORAGE_RESULT_COMPLETE);
    }
    if (asa_remote_open) {
        asa_dst->op.close.cb = atfp_hls__asaremote_closefile_cb;
        asa_result =  asa_dst->storage->ops.fn_close(asa_dst);
        asa_remote_open = asa_result == ASTORAGE_RESULT_ACCEPT;
        if(asa_result != ASTORAGE_RESULT_ACCEPT)
            atfp_hls__asaremote_closefile_cb(asa_dst, ASTORAGE_RESULT_COMPLETE);
    } else {
        atfp_hls__asaremote_closefile_cb(asa_dst, ASTORAGE_RESULT_COMPLETE);
    }
    return  asa_remote_open || asa_local_open;
} // end of atfp__video_hls__deinit
#undef  DEINIT_IF_EXISTS


static void atfp__video_hls__processing(atfp_t *processor)
{
    int ret = ATFP_AVCTX_RET__OK;
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
        if(ret) {
            if(ret < ATFP_AVCTX_RET__OK)
                json_object_set_new(processor->data.error, "transcoder", json_string("[hls] error when filtering"));
            continue;
        } // may return error (ret < 0), or no more frames to filter (ret == 1)
        int ret2 = ATFP_AVCTX_RET__OK;
        while(!ret2) {
            ret2 = hlsproc_dst->internal.op.encode(hlsproc_dst->av);
            if(ret2 == ATFP_AVCTX_RET__OK) {
                ret2 = hlsproc_dst->internal.op.write(hlsproc_dst->av);
            } else if(ret2 == ATFP_AVCTX_RET__NEED_MORE_DATA) {
                // no more encoded frames to write, break to outer loop for next filtered frame
            } else if(ret2 == ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER) {
                // all packets already flushed from all encoders, break both loops
                ret = ret2;
            }
            if(ret2 < ATFP_AVCTX_RET__OK) {
                json_object_set_new(processor->data.error, "transcoder", json_string("[hls] error when encoding"));
                ret = ret2;
            }
        }
    } // end of outer loop
    uint8_t  src_done = hlsproc_src->super.ops->has_done_processing(&hlsproc_src->super);
    uint8_t  flush_filt_done = hlsproc_dst->internal.op.has_done_flush_filter(hlsproc_src->av, hlsproc_dst->av);
    uint8_t  flush_enc_done  = hlsproc_dst->internal.op.has_done_flush_encoder(hlsproc_dst->av);
    if(src_done) // switch functions as soon as source file processor no longer provides data
        hlsproc_dst->internal.op.filter = hlsproc_dst->internal.op.finalize.filter;
    if(flush_filt_done)
        hlsproc_dst->internal.op.encode = hlsproc_dst->internal.op.finalize.encode;
    if(flush_enc_done)
        ret = hlsproc_dst->internal.op.finalize.write(hlsproc_dst->av);
    if(ret == ATFP_AVCTX_RET__NEED_MORE_DATA) {
        result = hlsproc_dst->internal.op.move_to_storage(hlsproc_dst);
    } else if(ret < ATFP_AVCTX_RET__OK) {
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
    out->super.transfer.dst.update_metadata = atfp_video__dst_update_metadata;
    out->asa_local.super.storage = app_storage_cfg_lookup("localfs") ; 
    out->internal.op.avctx_init   = atfp_hls__av_init;
    out->internal.op.avctx_deinit = atfp_hls__av_deinit;
    out->internal.op.avfilter_init = atfp_hls__avfilter_init;
    out->internal.op.filter  = atfp_hls__av_filter_processing;
    out->internal.op.encode  = atfp_hls__av_encode_processing;
    out->internal.op.write   = atfp_hls__av_local_white;
    out->internal.op.finalize.filter = atfp_hls__av_filter__finalize_processing;
    out->internal.op.finalize.encode = atfp_hls__av_encode__finalize_processing;
    out->internal.op.finalize.write  = atfp_hls__av_local_white_finalize;
    out->internal.op.move_to_storage = atfp_hls__try_flush_to_storage;
    out->internal.op.has_done_flush_filter = atfp_av_filter__has_done_flushing;
    out->internal.op.has_done_flush_encoder = atfp_av_encoder__has_done_flushing;
    return &out->super;
} // end of atfp__video_hls__instantiate

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
