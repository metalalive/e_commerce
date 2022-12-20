#include <assert.h>
#include <string.h>
#include <h2o/memory.h>

#include "transcoder/video/common.h"
#include "transcoder/video/hls.h"


atfp_t  *atfp__video_hls__instantiate_transcoder(void)
{
    atfp_t  *out = atfp__video_hls__instantiate();
    if(out) {
        atfp_hls_t *hlsproc = (atfp_hls_t *)out;
        out->transfer.transcoded_dst.update_metadata = atfp_video__dst_update_metadata;
        out->transfer.transcoded_dst.remove_file = atfp_storage_video_remove_version;
        hlsproc->internal.op.avctx_init   = atfp_hls__av_init;
        hlsproc->internal.op.avctx_deinit = atfp_hls__av_deinit;
        hlsproc->internal.op.avfilter_init = atfp_hls__avfilter_init;
        hlsproc->internal.op.filter  = atfp_hls__av_filter_processing;
        hlsproc->internal.op.encode  = atfp_hls__av_encode_processing;
        hlsproc->internal.op.write   = atfp_hls__av_local_write;
        hlsproc->internal.op.finalize.filter = atfp_hls__av_filter__finalize_processing;
        hlsproc->internal.op.finalize.encode = atfp_hls__av_encode__finalize_processing;
        hlsproc->internal.op.finalize.write  = atfp_hls__av_local_write_finalize;
        hlsproc->internal.op.move_to_storage = atfp_hls__try_flush_to_storage;
        hlsproc->internal.op.has_done_flush_filter = atfp_av_filter__has_done_flushing;
        hlsproc->internal.op.has_done_flush_encoder = atfp_av_encoder__has_done_flushing;
    }
    return out;
} // end of atfp__video_hls__instantiate_transcoder

static void atfp_hls__create_local_workfolder_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // TODO, check whether the folder was already creaated, delete all the files in it if exists
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, asaobj);
    atfp_t *processor = &hlsproc -> super;
    int err = hlsproc->internal.op.avctx_init(hlsproc);
    if(!err)
        err = hlsproc->internal.op.avfilter_init(hlsproc);
    if(!err)
        processor->transfer.transcoded_dst.info = json_object();
    processor -> data.callback(processor);
} // end of atfp_hls__create_local_workfolder_cb


static void  _atfp__hls_init__dst_version_folder_cb (asa_op_base_cfg_t *asa_dst, ASA_RES_CODE result)
{
    atfp_hls_t  *hlsproc = (atfp_hls_t *) asa_dst->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc -> super;
    if(result != ASTORAGE_RESULT_COMPLETE) {
        fprintf(stderr, "[transcoder][hls][init] line:%d, job_id:%s, result:%d \n",
                __LINE__, processor->data.rpc_receipt->job_id.bytes, result);
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to create version folder at remote storage"));
        processor->data.callback(processor);
        return;
    }
    processor->transfer.transcoded_dst.flags.version_created = 1;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local_srcdata =  atfp_asa_map_get_localtmp(_map);
    asa_op_localfs_cfg_t  *asa_local_dstdata = &hlsproc->asa_local;
    {
        void **cb_args_entries = calloc(NUM_USRARGS_HLS_ASA_LOCAL, sizeof(void *));
        cb_args_entries[ATFP_INDEX__IN_ASA_USRARG]   = (void *) processor;
        cb_args_entries[ASAMAP_INDEX__IN_ASA_USRARG] = (void *) _map;
        asa_local_dstdata->super.cb_args.entries = cb_args_entries;
        asa_local_dstdata->super.cb_args.size = NUM_USRARGS_HLS_ASA_LOCAL;
    } {
        // NOTE, if multiple destination file-processors work concurrently,  there should be multiple
        // local storage handles , each of which handles transcoded file for specific version
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
        size_t mst_playlist_name_sz = sizeof(HLS_MASTER_PLAYLIST_FILENAME) - 1;
        size_t l2_playlist_name_sz = sizeof(HLS_PLAYLIST_FILENAME) - 1;
        size_t pktmap_name_sz = sizeof(HLS_FMP4_FILENAME) - 1;
        size_t segment_name_sz = sizeof(HLS_SEGMENT_FILENAME_PREFIX) - 1 + HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS;
        size_t filename_max_sz = MAX(l2_playlist_name_sz, pktmap_name_sz);
        filename_max_sz = MAX(filename_max_sz, mst_playlist_name_sz);
        filename_max_sz = MAX(filename_max_sz, segment_name_sz) + 2; // extra slash char, and NUL-terminated char
        size_t fullpath_sz_local = strlen(asa_local_dstdata->super.op.mkdir.path.origin) + filename_max_sz;
        size_t fullpath_sz_dst   = strlen(asa_dst->op.mkdir.path.prefix) + 1 + 
            strlen(asa_dst->op.mkdir.path.origin) + filename_max_sz;
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
    result = app_storage_localfs_mkdir(&asa_local_dstdata->super, 1);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("[hls] failed to issue create-folder operation for internal local tmp buf"));
        processor -> data.callback(processor);
    }
} // end of _atfp__hls_init__dst_version_folder_cb


void  atfp__video_hls__init_transcode(atfp_t *processor)
{
    asa_op_base_cfg_t *asa_dst = processor -> data.storage.handle;
    if(asa_dst->op.write.src_max_nbytes == 0 || !asa_dst->op.write.src) {
        json_object_set_new(processor->data.error, "storage", json_string("[hls] no write buffer provided in asaobj"));
    } else if (processor->transfer.transcoded_dst.flags.asalocal_open ||
            processor->transfer.transcoded_dst.flags.asaremote_open) {
        json_object_set_new(processor->data.error, "transcoder", json_string("[hls] asaobj is not cleaned"));
    } else {
        atfp_storage_video_create_version(processor, _atfp__hls_init__dst_version_folder_cb);
    }
    if(json_object_size(processor->data.error) > 0) {
        processor -> data.callback(processor);
        fprintf(stderr, "[transcoder][hls][init] line:%d, job_id:%s, avinput or validation error \n",
                __LINE__, processor->data.rpc_receipt->job_id.bytes);
    }
} // end of atfp__video_hls__init_transcode


void atfp__video_hls__proceeding_transcode(atfp_t *processor)
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
    if(ret == ATFP_AVCTX_RET__NEED_MORE_DATA) { // will return `ASTORAGE_RESULT_ACCEPT`
        result = hlsproc_dst->internal.op.move_to_storage(hlsproc_dst);
    } else if(ret < ATFP_AVCTX_RET__OK) {
        result = ASTORAGE_RESULT_UNKNOWN_ERROR;
    }
    assert(result != ASTORAGE_RESULT_COMPLETE);
} // end of atfp__video_hls__proceeding_transcode


uint8_t  atfp__video_hls__has_done_processing(atfp_t *processor)
{
    atfp_hls_t *hlsproc_dst = (atfp_hls_t *)processor;
    return atfp_av__has_done_processing(hlsproc_dst->av);
}
