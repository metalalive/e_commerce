#include <assert.h>
#include <string.h>
#include <h2o/memory.h>
#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

#define   LOCAL_BUFFER_FILENAME    "local_buffer"

static void  atfp_mp4__postpone_usr_callback(uv_async_t* handle)
{
    atfp_t *processor = handle -> data;
    processor -> data.callback(processor);
}

static void atfp_mp4__avinput_init_done_cb (atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) == 0) {
        atfp_mp4__validate_source_format(mp4proc);
    }
    if(json_object_size(err_info) == 0) {
        asa_op_base_cfg_t *asa_src = mp4proc -> super.data.storage.handle;
        atfp_asa_map_t *_map = (atfp_asa_map_t *)asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
        int ret = uv_async_init(asa_local->loop, &mp4proc->async, atfp_mp4__postpone_usr_callback);
        if(ret < 0) 
            json_object_set_new(err_info, "libav", json_string("[mp4] failed to init internal async handle"));
    }
    processor -> data.callback(processor);
} // end of atfp_mp4__avinput_init_done_cb


static void atfp_mp4__preload_stream_info__done(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) == 0) {
        ASA_RES_CODE result = atfp_mp4__av_init(mp4proc, atfp_mp4__avinput_init_done_cb);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "libav", json_string("[mp4] failed to init avformat context"));
    }
    if(json_object_size(err_info) > 0)
        processor -> data.callback(processor);
}


static void atfp__video_mp4__open_local_tmpbuf_cb (asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // start loading mmp4 header from input resource to local temp buffer
    atfp_asa_map_t  *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *asa_src = atfp_asa_map_get_source(map);
    atfp_t *processor = asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)processor;
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        result = atfp_mp4__preload_stream_info(mp4proc, atfp_mp4__preload_stream_info__done);
        if(result != ASTORAGE_RESULT_ACCEPT) 
            json_object_set_new(err_info, "storage", json_string("failed to issue read operation to mp4 input"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to open local temp buffer"));
    }
    if(json_object_size(err_info) > 0) 
        processor -> data.callback(processor);
} // end of atfp__video_mp4__open_local_tmpbuf_cb


static void atfp__video_mp4__init(atfp_t *processor)
{
    asa_op_base_cfg_t *asaobj = processor->data.storage.handle;
    atfp_asa_map_t    *map = asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(map);
    const char *local_tmpbuf_basepath = asa_local->super.op.mkdir.path.origin;
    processor->filechunk_seq.curr = 0;
    processor->filechunk_seq.next = 0;
    processor->filechunk_seq.eof_reached = 0;
    {
        asa_local->super.op.open.cb = atfp__video_mp4__open_local_tmpbuf_cb;
        asa_local->super.op.open.mode  = S_IRUSR | S_IWUSR;
        asa_local->super.op.open.flags = O_RDWR | O_CREAT;
        size_t tmpbuf_basepath_sz = strlen(local_tmpbuf_basepath);
        size_t tmpbuf_filename_sz = strlen(LOCAL_BUFFER_FILENAME);
        size_t tmpbuf_fullpath_sz = tmpbuf_basepath_sz + 1 + tmpbuf_filename_sz + 1;
        char *ptr = calloc(tmpbuf_fullpath_sz, sizeof(char));
        asa_local->super.op.open.dst_path = ptr;
        strncat(ptr, local_tmpbuf_basepath, tmpbuf_basepath_sz);
        strncat(ptr, "/", 1);
        strncat(ptr, LOCAL_BUFFER_FILENAME, tmpbuf_filename_sz);
    }
    if(app_storage_localfs_open(&asa_local->super) != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("failed to issue open operation for local temp buffer"));
        processor -> data.callback(processor);
    }
} // end of atfp__video_mp4__init


static void  atfp_mp4__close_local_tmpbuf_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    free(processor);
} // end of atfp_mp4__close_local_tmpbuf_cb

static void  atfp_mp4__close_async_handle_cb (uv_handle_t* handle)
{
    atfp_t *processor = handle->data;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    atfp_asa_map_t    *map = asa_src->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t *asa_local = atfp_asa_map_get_localtmp(map);
    ASA_RES_CODE  result;
    if(asa_local) {
        if(asa_local->super.op.open.dst_path) {
            free(asa_local->super.op.open.dst_path);
            asa_local->super.op.open.dst_path = NULL;
        }
        asa_local->super.op.close.cb =  atfp_mp4__close_local_tmpbuf_cb;
        result = app_storage_localfs_close(&asa_local->super);
    } else {
        result = ASTORAGE_RESULT_COMPLETE;
    }
    if(result != ASTORAGE_RESULT_ACCEPT) {
        free(processor);
    } // local temp buffer may already be closed
} // end of atfp_mp4__close_async_handle_cb


static void atfp__video_mp4__deinit(atfp_t *processor)
{
    atfp_mp4_t *mp4_proc = (atfp_mp4_t *)processor;
    if(processor->transcoded_info) {
        json_decref(processor->transcoded_info);
        processor->transcoded_info = NULL;
    }
    atfp_mp4__av_deinit(mp4_proc);
    uv_handle_t *async_handle = (uv_handle_t *)&mp4_proc->async;
    if(uv_has_ref(async_handle)) {
        uv_close(async_handle, atfp_mp4__close_async_handle_cb);
    } else { // not initialized yet
        atfp_mp4__close_async_handle_cb(async_handle);
    }
} // end of atfp__video_mp4__deinit


static void _atfp_mp4__processing_one_frame(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    int frame_avail = 0, pkt_avail = 0;
    int err = 0;
    do {
        err = mp4proc->internal.op.decode_pkt(mp4proc->av);
        if(!err) {
            frame_avail = 1;
        } else if(err == 1) { // new packet required
            err =  mp4proc->internal.op.next_pkt(mp4proc->av);
            if(!err) {
                pkt_avail = 1;
            } else if(err == 1) { // another preload operation required
                ASA_RES_CODE result = mp4proc->internal.op.preload(mp4proc, ATFP_MP4__DEFAULT_NBYTES_BULK,
                        _atfp_mp4__processing_one_frame );
                err = (result == ASTORAGE_RESULT_ACCEPT)? 0: -1;
                break;
            } else {
                json_object_set_new(err_info, "transcoder", json_string("[mp4] error when getting next packet from local temp buffer"));
                break;
            }
        } else {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to decode next packet"));
            break;
        }
    } while (!frame_avail);

    if(frame_avail) {
        // invoke the callback asynchronously if a new frame is decoded successfully, which avoids
        // recursive calls between source and destination file processors when there are too many
        // packets fetched and decoded successfully, which may leads to stack overflow.
        err = uv_async_send(&mp4proc->async);
        if(err < 0)
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to send async event with decoded frame"));
    }
    if(err)
        processor -> data.callback(processor);
    // TODO, return following data when processing is done successfully
    ////atfp_t *processor = &mp4proc -> super;
    ////processor->transcoded_info = json_array();
    ////json_t  *item = json_object();
    ////json_object_set_new(item, "filename", json_string("fake_transcoded_file.mp4"));
    ////json_object_set_new(item, "size", json_integer(8193));
    ////json_object_set_new(item, "checksum", json_string("f09d77e32572b562863518c6"));
    ////json_array_append_new(processor->transcoded_info, item);
} // end of _atfp_mp4__processing_one_frame


static void atfp__video_mp4__processing(atfp_t *processor)
{
    _atfp_mp4__processing_one_frame((atfp_mp4_t *)processor);
} // end of atfp__video_mp4__processing

static uint8_t  atfp__video_mp4__has_done_processing(atfp_t *processor)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)processor;
    return  atfp_ffmpeg_avctx__has_done_decoding(mp4proc->av);
}


static atfp_t *atfp__video_mp4__instantiate(void) {
    // at this point, `atfp_av_ctx_t` should NOT be incomplete type
    size_t tot_sz = sizeof(atfp_mp4_t) + sizeof(atfp_av_ctx_t);
    atfp_mp4_t  *out = calloc(0x1, tot_sz);
    out->internal.op.decode_pkt  = atfp_mp4__av_decode_packet;
    out->internal.op.next_pkt    = atfp_ffmpeg__next_local_packet;
    out->internal.op.preload     = atfp_mp4__av_preload_packets;
    out->async.data = out;
    char *ptr = (char *)out + sizeof(atfp_mp4_t);
    out->av = (atfp_av_ctx_t *)ptr;
    return &out->super;
}

static uint8_t    atfp__video_mp4__label_match(const char *label) {
    const char *exp_labels[3] = {"video/mp4", "mp4", "mov"};
    return atfp_common__label_match(label, 3, exp_labels);
}

const atfp_ops_entry_t  atfp_ops_video_mp4 = {
    .backend_id = ATFP_BACKEND_LIB__FFMPEG,
    // TODO, indicate the operations are for source or destination
    .ops = {
        .init   = atfp__video_mp4__init,
        .deinit = atfp__video_mp4__deinit,
        .processing  = atfp__video_mp4__processing,
        .instantiate = atfp__video_mp4__instantiate,
        .label_match = atfp__video_mp4__label_match,
        .has_done_processing = atfp__video_mp4__has_done_processing,
    },
};
