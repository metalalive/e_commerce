#include <assert.h>
#include <unistd.h>
#include <string.h>
#include <h2o/memory.h>
#include <libavutil/error.h>

#include "app_cfg.h"
#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

static int  atfp_avio__read_header(void *opaque, uint8_t *buf, int required_size)
{ // the API functions in libavformat always access file in blocking way,
    // non-blocking call currently hasn't been supported yet.
    if(!opaque)
        return AVERROR(EINVAL) ;
    asa_op_localfs_cfg_t *local_tmpbuf = (asa_op_localfs_cfg_t *)opaque;
    int hdr_tmp_fd =  local_tmpbuf->file.file;
    int nread = read(hdr_tmp_fd, buf, required_size);
    if(nread == 0)
        nread = AVERROR_EOF;
    return nread;
    
} // end of atfp_avio__read_header


static  size_t  atfp_mp4__mdat_body_pos(atfp_mp4_t *mp4proc)
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
    int idx = 0;
    AVFormatContext    *fmt_ctx = mp4proc->avinput.fmt_ctx;
    atfp_mp4_stream_ctx_t *sctx = mp4proc->avinput.stream_ctx;
    if(sctx) {
        int nb_streams = fmt_ctx ? fmt_ctx->nb_streams: 0;
        for(idx = 0; idx < nb_streams; idx++) {
            if(sctx[idx].dec_ctx)
                avcodec_free_context(&sctx[idx].dec_ctx);
        }
        av_freep(&mp4proc->avinput.stream_ctx);
    }
    if(fmt_ctx) {
        AVIOContext *avio_ctx = fmt_ctx -> pb;
        if(avio_ctx) {
            av_freep(&avio_ctx->buffer);
            avio_context_free(&fmt_ctx->pb);            
        }
        avformat_close_input((AVFormatContext **)&mp4proc->avinput.fmt_ctx);
    }
} // end of atfp_mp4__avinput_deinit


int  atfp_mp4__validate_source_format(atfp_mp4_t *mp4proc)
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    AVFormatContext  *fmt_ctx = mp4proc->avinput.fmt_ctx;
    atfp_mp4_stream_ctx_t *stream_ctx = mp4proc->avinput.stream_ctx;
    app_cfg_t *app_cfg = app_get_global_cfg();
    aav_cfg_input_t *aav_cfg_in = &app_cfg->transcoder.input;
    int idx = 0;
    void *demux_valid = NULL;
    void *codec_video_valid = NULL;
    void *codec_audio_valid = NULL;
    for(idx = 0; !demux_valid && idx < aav_cfg_in->demuxers.size; idx++) {
         void *ifmt2 = aav_cfg_in->demuxers .entries[idx];
         if(fmt_ctx->iformat == ifmt2)
             demux_valid = fmt_ctx->iformat;
    }
#define   GENCODE_FIND_CODEC(stream_type, valid_obj) \
    for(int jdx = 0; !valid_obj && jdx < aav_cfg_in->decoder.stream_type.size; jdx++) { \
        void *codec2 = aav_cfg_in->decoder.stream_type.entries[jdx]; \
        if(codec1 == codec2) \
           valid_obj = codec2; \
    }
    for(idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        const struct AVCodec *codec1 = stream_ctx[idx].dec_ctx ->codec;
        if(codec1->type == AVMEDIA_TYPE_VIDEO) {
            GENCODE_FIND_CODEC(video, codec_video_valid);
        } else if (codec1->type == AVMEDIA_TYPE_AUDIO) {
            GENCODE_FIND_CODEC(audio, codec_audio_valid);
        }
    } // end of stream context iteration
#undef  GENCODE_FIND_CODEC
    if(!demux_valid) {
        json_object_set_new(err_info, "transcoder", json_string("[mp4] unsupported demuxer"));
    } else if(!codec_video_valid) {
        json_object_set_new(err_info, "transcoder", json_string("[mp4] unsupported video codec"));
    } else if(!codec_audio_valid) {
        json_object_set_new(err_info, "transcoder", json_string("[mp4] unsupported audio codec"));
    }
    int err = json_object_size(err_info) > 0;
    return err;
} // end of atfp_mp4__validate_source_format


static void atfp_mp4__preload_initial_packets_done (atfp_mp4_t *mp4proc) 
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    AVFormatContext  *fmt_ctx = mp4proc->avinput.fmt_ctx;
    asa_op_localfs_cfg_t *local_tmpbuf = (asa_op_localfs_cfg_t *)&mp4proc->local_tmpbuf_handle;
    int idx = 0, ret = 0;
    if(json_object_size(err_info) > 0) // error from storage
        goto done;
    lseek(local_tmpbuf->file.file, 0, SEEK_SET);
    ret = avformat_find_stream_info(fmt_ctx, NULL);
    if(ret < 0) {
        json_object_set_new(err_info, "transcoder", json_string("failed to analyze stream info"));
        goto done;
    }
    atfp_mp4_stream_ctx_t *stream_ctx = av_mallocz_array(fmt_ctx->nb_streams, sizeof(atfp_mp4_stream_ctx_t));
    mp4proc->avinput.stream_ctx = stream_ctx;
    for(idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        AVStream *stream  = fmt_ctx->streams[idx];
        AVCodec  *decoder = avcodec_find_decoder(stream->codecpar->codec_id);
        if(!decoder) {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to find decoder for the stream"));
            break;
        }
        AVCodecContext *codec_ctx = avcodec_alloc_context3(decoder);
        stream_ctx[idx].dec_ctx = codec_ctx;
        if(!codec_ctx) {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to create decoder context of the stream"));
            break;
        }
        ret = avcodec_parameters_to_context(codec_ctx, stream->codecpar);
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to copy parameters from stream to decoder context"));
            break;
        }
        switch(codec_ctx->codec_type) {
            case AVMEDIA_TYPE_VIDEO:
                codec_ctx->framerate = av_guess_frame_rate(fmt_ctx, stream, NULL);
                // pass through          
            case AVMEDIA_TYPE_AUDIO:
                ret = avcodec_open2(codec_ctx, decoder, NULL);
                break;
            default:
                break;
        }
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to open decoder for stream"));
            break;
        }
    } // end of stream iteration
    if(0) {
        av_dump_format(fmt_ctx, 0, "some_input_file_path", 0);
    } // dump format for debugging
done:
    mp4proc->internal.callback.avinput_init_done(mp4proc);
} // end of atfp_mp4__preload_initial_packets_done


ASA_RES_CODE  atfp_mp4__avinput_init (atfp_mp4_t *mp4proc, size_t num_init_pkts, void (*cb)(atfp_mp4_t *))
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
        if(ret < 0) 
            goto error;
        fmt_ctx->pb ->pos = atfp_mp4__mdat_body_pos(mp4proc);
        // erase content in local temp buffer, load initial packets from source
        lseek(local_tmpbuf->file.file, 0, SEEK_SET);
    }
    size_t  first_pkt_pos = fmt_ctx->pb ->pos;
    size_t  farest_pos = 0;
    size_t  farest_sz  = 0;
    for (int idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        AVStream *stream = fmt_ctx->streams[idx];
        size_t max_num_pkts = FFMIN(num_init_pkts, stream->nb_index_entries);
        for (int jdx = 0; jdx < max_num_pkts; jdx++) {
            size_t  pkt_pos = stream-> index_entries[jdx].pos;
            if(farest_pos < pkt_pos) {
                farest_pos = pkt_pos;
                farest_sz  = stream-> index_entries[jdx].size;
            }
        }
    }
    size_t nbytes_to_load  = farest_pos + farest_sz - first_pkt_pos;
    int    chunk_idx  = mp4proc->internal.mdat.fchunk_seq;
    size_t offset     = mp4proc->internal.mdat.pos;
    return  atfp_mp4__preload_packet_sequence (mp4proc, chunk_idx, offset, nbytes_to_load,
               atfp_mp4__preload_initial_packets_done);
error:
    json_object_set_new(mp4proc->super.data.error, "transcoder",
            json_string("[mp4] failed to initialize AVFormatContext"));
    atfp_mp4__avinput_deinit(mp4proc);
    return ASTORAGE_RESULT_OS_ERROR;
#undef  AVIO_CTX_BUFFER_SIZE    
} // end of atfp_mp4__avinput_init

