#include <assert.h>
#include <string.h>
#include <h2o/memory.h>
#include "transcoder/video/mp4.h"

#define   LOCAL_BUFFER_FILENAME    "local_buffer"

static void atfp_mp4__avinput_init_done_cb (atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) == 0) {
        atfp_mp4__validate_source_format(mp4proc);
    }
    if(json_object_size(err_info) == 0) {
        // TODO, intialize output format context
    }
    processor -> data.callback(processor);
}


static void atfp_mp4__preload_stream_info__done(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(json_object_size(err_info) == 0) {
        ASA_RES_CODE result = atfp_mp4__avinput_init(mp4proc, 5, atfp_mp4__avinput_init_done_cb);
        if(result != ASTORAGE_RESULT_ACCEPT)
            json_object_set_new(err_info, "libav", json_string("[mp4] failed to init avformat context"));
    }
    if(json_object_size(err_info) > 0)
        processor -> data.callback(processor);
}


static void atfp__video_mp4__open_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{ // start loading mmp4 header from input resource to local temp buffer
    atfp_mp4_t *mp4proc = (atfp_mp4_t *) H2O_STRUCT_FROM_MEMBER(atfp_mp4_t, local_tmpbuf_handle, cfg);
    atfp_t *processor = & mp4proc -> super;
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

static void atfp__video_mp4__mkdir_local_tmpbuf_cb (asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *) H2O_STRUCT_FROM_MEMBER(atfp_mp4_t, local_tmpbuf_handle, cfg);
    atfp_t *processor = & mp4proc -> super;
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        cfg->op.open.cb = atfp__video_mp4__open_local_tmpbuf_cb;
        cfg->op.open.mode  = S_IRUSR | S_IWUSR;
        cfg->op.open.flags = O_RDWR | O_CREAT;
        size_t tmpbuf_basepath_sz = strlen(processor->data.local_tmpbuf_basepath);
        size_t tmpbuf_filename_sz = strlen(LOCAL_BUFFER_FILENAME);
        size_t tmpbuf_fullpath_sz = tmpbuf_basepath_sz + 1 + tmpbuf_filename_sz + 1;
        cfg->op.open.dst_path = calloc(tmpbuf_fullpath_sz, sizeof(char));
        {
            char *ptr = cfg->op.open.dst_path;
            strncat(ptr, processor->data.local_tmpbuf_basepath, tmpbuf_basepath_sz);
            strncat(ptr, "/", 1);
            strncat(ptr, LOCAL_BUFFER_FILENAME, tmpbuf_filename_sz);
        }
        result = app_storage_localfs_open(cfg);
        if(result != ASTORAGE_RESULT_ACCEPT) 
            json_object_set_new(err_info, "storage", json_string("failed to issue open operation for local temp buffer"));
    } else {
        json_object_set_new(err_info, "storage", json_string("failed to create folder for local temp buffer"));
    }
    if(json_object_size(err_info) > 0) 
        processor -> data.callback(processor);
} // end of atfp__video_mp4__mkdir_local_tmpbuf_cb


static void atfp__video_mp4__init(atfp_t *processor)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *)processor;
    asa_op_localfs_cfg_t *asa_cfg_local = &mp4proc->local_tmpbuf_handle;
    asa_cfg_local->loop = processor->data.loop;
    processor->filechunk_seq.curr = 0;
    processor->filechunk_seq.next = 0;
    processor->filechunk_seq.eof_reached = 0;
    { // create folder for local temp buffer
        size_t local_tmpbuf_basepath_sz = strlen(processor->data.local_tmpbuf_basepath) + 1;
        char *ptr = calloc(local_tmpbuf_basepath_sz << 1, sizeof(char));
        asa_cfg_local->super.op.mkdir.mode = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
        asa_cfg_local->super.op.mkdir.cb =  atfp__video_mp4__mkdir_local_tmpbuf_cb;
        asa_cfg_local->super.op.mkdir.path.origin = ptr;
        asa_cfg_local->super.op.mkdir.path.curr_parent = ptr + local_tmpbuf_basepath_sz;
        memcpy(asa_cfg_local->super.op.mkdir.path.origin, processor->data.local_tmpbuf_basepath,
                local_tmpbuf_basepath_sz - 1);
    }
    ASA_RES_CODE result = app_storage_localfs_mkdir((asa_op_base_cfg_t *)asa_cfg_local);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        json_object_set_new(processor->data.error, "storage",
                json_string("failed to issue mkdir operation for local temp buffer"));
        processor -> data.callback(processor);
    }
} // end of atfp__video_mp4__init


static void  atfp_mp4__close_local_tmpbuf_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{
    atfp_mp4_t *mp4proc = (atfp_mp4_t *) H2O_STRUCT_FROM_MEMBER(atfp_mp4_t, local_tmpbuf_handle, cfg);
    free(mp4proc);
} // end of atfp_mp4__close_local_tmpbuf_cb


static void atfp__video_mp4__deinit(atfp_t *processor)
{
    atfp_mp4_t *mp4_proc = (atfp_mp4_t *)processor;
    asa_op_localfs_cfg_t *asa_cfg_local = &mp4_proc->local_tmpbuf_handle;
    if(processor->transcoded_info) {
        json_decref(processor->transcoded_info);
        processor->transcoded_info = NULL;
    }
    if(asa_cfg_local->super.op.mkdir.path.origin) {
        free(asa_cfg_local->super.op.mkdir.path.origin);
        asa_cfg_local->super.op.mkdir.path.origin = NULL;
        asa_cfg_local->super.op.mkdir.path.curr_parent = NULL;
    }
    if(asa_cfg_local->super.op.open.dst_path) {
        free(asa_cfg_local->super.op.open.dst_path);
        asa_cfg_local->super.op.open.dst_path = NULL;
    }
    atfp_mp4__avinput_deinit(mp4_proc);
    asa_cfg_local->super.op.close.cb =  atfp_mp4__close_local_tmpbuf_cb;
    ASA_RES_CODE result = app_storage_localfs_close((asa_op_base_cfg_t *)asa_cfg_local);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        free(processor);
    } // local temp buffer may already be closed
} // end of atfp__video_mp4__deinit


static void atfp__video_mp4__processing(atfp_t *processor)
{
    processor->transcoded_info = json_array();
    json_t  *item = json_object();
    json_object_set_new(item, "filename", json_string("fake_transcoded_file.mp4"));
    json_object_set_new(item, "size", json_integer(8193));
    json_object_set_new(item, "checksum", json_string("f09d77e32572b562863518c6"));
    json_array_append_new(processor->transcoded_info, item);
    processor -> data.callback(processor);
} // end of atfp__video_mp4__processing


static size_t  atfp__video_mp4__get_obj_size(void)
{
    return sizeof(atfp_mp4_t);
} // end of atfp__video_mp4__get_obj_size


atfp_ops_entry_t  atfp_ops_video_mp4 = {
    .mimetype = "video/mp4",
    .ops = {
        .init = atfp__video_mp4__init,
        .deinit = atfp__video_mp4__deinit,
        .processing = atfp__video_mp4__processing,
        .get_obj_size = atfp__video_mp4__get_obj_size,
    },
};
