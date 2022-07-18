#include <assert.h>
#include <unistd.h>
#include <string.h>
#include <h2o/memory.h>
#include <libavformat/avformat.h>
#include <libavformat/avio.h>

#include "transcoder/video/mp4.h"


static int  atfp_avio__read_header(void *opaque, uint8_t *buf, int required_size)
{
    if(!opaque) {
        return AVERROR(EINVAL) ;
    }
    asa_op_localfs_cfg_t *local_tmpbuf = (asa_op_localfs_cfg_t *)opaque;
    int hdr_tmp_fd =  local_tmpbuf->file.file;
    int nread = read(hdr_tmp_fd, buf, required_size);
    if(nread == 0)
        nread = AVERROR_EOF;
    return nread;
    
} // end of atfp_avio__read_header


static __attribute__((optimize("O0"))) size_t  atfp_mp4__mdat_body_pos(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *fchunks_sz = json_object_get(processor->data.spec, "parts_size");
    json_t *sz_item = NULL;
    int idx = 0;
    size_t out = mp4proc->internal.mdat.pos;
    json_array_foreach(fchunks_sz, idx, sz_item) {
        if(idx >= mp4proc->internal.mdat.fchunk_seq) { break; }
        out += (size_t)json_integer_value(sz_item);
    }
    return out;
} // end of atfp_mp4__mdat_body_pos


void  atfp_mp4__avinput_deinit(atfp_mp4_t *mp4proc)
{
    if(mp4proc->avinput.fmt_ctx) {
        AVFormatContext *fmt_ctx = mp4proc->avinput.fmt_ctx;
        AVIOContext *avio_ctx = fmt_ctx -> pb;
        if(avio_ctx) {
            av_freep(&avio_ctx->buffer);
            avio_context_free(&fmt_ctx->pb);            
        }
        avformat_close_input((AVFormatContext **)&mp4proc->avinput.fmt_ctx);
    }
} // end of atfp_mp4__avinput_deinit


static void atfp_mp4__preload_first_few_frames_done (atfp_mp4_t *mp4proc) 
{
    mp4proc->internal.callback.avinput_init_done(mp4proc);
}

ASA_RES_CODE  atfp_mp4__avinput_init (atfp_mp4_t *mp4proc, void (*cb)(atfp_mp4_t *))
{
    asa_op_localfs_cfg_t *local_tmpbuf = (asa_op_localfs_cfg_t *)&mp4proc->local_tmpbuf_handle;
#define  AVIO_CTX_BUFFER_SIZE 2048    
    uint8_t  *avio_ctx_buffer = NULL;
    AVFormatContext *fmt_ctx = NULL;
    int ret = 0;
    if (!(fmt_ctx = avformat_alloc_context()))
        goto error;
    mp4proc->internal.callback.avinput_init_done = cb;
    mp4proc->avinput.fmt_ctx = fmt_ctx;
    avio_ctx_buffer = av_malloc(AVIO_CTX_BUFFER_SIZE);
    fmt_ctx -> pb = avio_alloc_context(avio_ctx_buffer, AVIO_CTX_BUFFER_SIZE, 0,
              local_tmpbuf, atfp_avio__read_header, NULL, NULL); // &app_seek_packet
    if (!fmt_ctx->pb || !avio_ctx_buffer)
        goto error;
    { // libavformat accesses input file synchronously
        lseek(local_tmpbuf->file.file, 0, SEEK_SET);
        ret = avformat_open_input(&fmt_ctx, NULL, NULL, NULL);
        if(ret >= 0) {
            fmt_ctx->pb ->pos = atfp_mp4__mdat_body_pos(mp4proc);
        } else {
            goto error;
        }
    } 
    return  atfp_mp4__preload_initial_packets (mp4proc, 5, atfp_mp4__preload_first_few_frames_done);
error:
    atfp_mp4__avinput_deinit(mp4proc);
    return ASTORAGE_RESULT_UNKNOWN_ERROR;
#undef  AVIO_CTX_BUFFER_SIZE    
} // end of atfp_mp4__avinput_init

