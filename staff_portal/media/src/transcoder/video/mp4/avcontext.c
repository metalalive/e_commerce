#include <assert.h>
#include <unistd.h>
#include <string.h>
#include <libavutil/error.h>

#include "app_cfg.h"
#include "rpc/core.h"
#include "transcoder/video/mp4.h"
#include "transcoder/video/ffmpeg.h"

static int  atfp_avio__read_local_tmpbuf(void *opaque, uint8_t *buf, int required_size)
{ // the API functions in libavformat always access file in blocking way,
    // non-blocking call currently hasn't been supported yet.
    if(!opaque)
        return AVERROR(EINVAL) ;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)opaque;
    asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
    asa_op_base_cfg_t     *asa_src   = atfp_asa_map_get_source(_map);
    atfp_mp4_t  *mp4proc = (atfp_mp4_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    int  hdr_tmp_fd =  asa_local->file.file;
    size_t  nbytes_total  = mp4proc->internal.preload_pkts .size;
    size_t  nbytes_used   = lseek(hdr_tmp_fd, 0, SEEK_CUR);
    size_t  nbytes_avail  = nbytes_total - nbytes_used;
    size_t  nbytes_copy  = FFMIN(nbytes_avail, required_size);
    int nread = 0;
    if(nbytes_copy == 0) {
        nread = AVERROR_EOF;
    } else {
        nread = read(hdr_tmp_fd, buf, nbytes_copy);
        if(nread == 0)
            nread = AVERROR_EOF;
    }
    return nread;
} // end of atfp_avio__read_local_tmpbuf


static ASA_RES_CODE  atfp_mp4__serial_to_segment_pos(json_t *spec, size_t in_offset, int *out_chunk_idx, size_t *out_offset)
{
    ASA_RES_CODE result = ASTORAGE_RESULT_COMPLETE;
    json_t *fchunks_sz = json_object_get(spec, "parts_size");
    json_t *sz_item = NULL;
    int idx = 0, done = 0;
    json_array_foreach(fchunks_sz, idx, sz_item) {
        size_t sz = (size_t)json_integer_value(sz_item);
        if(in_offset > sz) {
            in_offset -= sz;
        } else {
            *out_chunk_idx = idx;
            *out_offset = in_offset;
            done = 1;
            break;
        }
    }
    if(!done) // invalid `in_offset` exceeding the limit
        result = ASTORAGE_RESULT_ARG_ERROR;
    return result;
} // end of atfp_mp4__serial_to_segment_pos


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


static uint8_t estimate_nb_initial_packet__continue_fn(atfp_av_ctx_t *avctx, size_t start_pos, size_t end_pos)
{
    uint8_t should_continue = 0;
    AVFormatContext *fmt_ctx = avctx->fmt_ctx;
    for(int idx = 0; (!should_continue) && (idx < fmt_ctx->nb_streams); idx++) {
         int curr_pkt_idx = avctx-> stats[idx].index_entry.preloading;
         should_continue = (avctx->async_limit.num_init_pkts) > curr_pkt_idx;
    }
    return should_continue;
}

static uint8_t estimate_nb_subsequent_packet__continue_fn (atfp_av_ctx_t *avctx, size_t start_pos, size_t end_pos)
{
    uint8_t should_continue = (end_pos - start_pos) < avctx->async_limit.max_nbytes_bulk;
    return should_continue;
}


static  size_t   atfp_ffmpeg__estimate_nb_pkt_preload(atfp_av_ctx_t *avctx, size_t start_pkt_pos,
        uint8_t (*continue_fn)(atfp_av_ctx_t*, size_t, size_t) )
{ // TODO, make it testable, this private function becomes complicated
    size_t  farest_pos = start_pkt_pos;
    AVFormatContext *fmt_ctx = avctx->fmt_ctx;
    uint8_t  end_of_streams = 1;
    do {
        AVStream *stream = NULL;
        int curr_pkt_idx = -1;
        end_of_streams = 1;
        for (int idx = 0; (!stream) && (idx < fmt_ctx->nb_streams); idx++) {
            stream = fmt_ctx->streams[idx];
            curr_pkt_idx = avctx-> stats[idx].index_entry.preloading;
            end_of_streams &= (curr_pkt_idx >= stream->nb_index_entries);
            if(!end_of_streams) {
                size_t pkt_pos = stream-> index_entries[curr_pkt_idx].pos;
                if(farest_pos != pkt_pos)
                    stream = NULL;
            } else {
                stream = NULL;
            }
        }
        if(stream && curr_pkt_idx >= 0) {
            farest_pos += stream-> index_entries[curr_pkt_idx].size;
            avctx->stats[ stream->index ].index_entry.preloading = curr_pkt_idx + 1;
        } // TODO, handle data corruption case
    } while(!end_of_streams && continue_fn(avctx, start_pkt_pos, farest_pos));
    return farest_pos - start_pkt_pos;
} // end of atfp_ffmpeg__estimate_nb_pkt_preload


static void  atfp_mp4__av_packet_deinit(AVPacket  *packet)
{
    av_packet_unref(packet);
    // *packet = (AVPacket) {0};
    packet->stream_index = -1;
}

void  atfp_mp4__av_deinit(atfp_mp4_t *mp4proc)
{
    int idx = 0;
    AVFormatContext    *fmt_ctx = mp4proc->av->fmt_ctx;
    AVCodecContext   **dec_ctxs = mp4proc->av->stream_ctx.decode;
    AVPacket  *pkt = & mp4proc->av->intermediate_data.decode.packet;
    AVFrame   *frm = & mp4proc->av->intermediate_data.decode.frame;
    if(mp4proc-> av-> stats)
        av_freep(&mp4proc-> av-> stats);
    if(dec_ctxs) {
        int nb_streams = fmt_ctx ? fmt_ctx->nb_streams: 0;
        for(idx = 0; idx < nb_streams; idx++) {
            if(dec_ctxs[idx])
                avcodec_free_context(&mp4proc->av->stream_ctx.decode[idx]);
        }
        av_freep(&mp4proc->av->stream_ctx.decode);
    }
    atfp_mp4__av_packet_deinit(pkt);
    av_frame_unref(frm);
    if(fmt_ctx) {
        AVIOContext *avio_ctx = fmt_ctx -> pb;
        if(avio_ctx) {
            av_freep(&avio_ctx->buffer);
            avio_context_free(&fmt_ctx->pb);            
        }
        avformat_close_input((AVFormatContext **)&mp4proc->av->fmt_ctx);
    }
} // end of atfp_mp4__av_deinit


int  atfp_av__validate_source_format(atfp_av_ctx_t *avctx, json_t *err_info)
{
    AVFormatContext  *fmt_ctx = avctx->fmt_ctx;
    AVCodecContext **dec_ctxs = avctx->stream_ctx.decode;
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
        const struct AVCodec *codec1 = dec_ctxs[idx]->codec;
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
} // end of atfp_av__validate_source_format


static void atfp_mp4__preload_initial_packets_done (atfp_mp4_t *mp4proc) 
{
    atfp_t *processor = &mp4proc -> super;
    json_t *err_info = processor->data.error;
    AVFormatContext  *fmt_ctx = mp4proc->av ->fmt_ctx;
    asa_op_base_cfg_t *asaobj = mp4proc -> super.data.storage.handle;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
    int idx = 0, ret = 0;
    if(json_object_size(err_info) > 0) // error from storage
        goto done;
    lseek(asa_local->file.file, 0, SEEK_SET);
    ret = avformat_find_stream_info(fmt_ctx, NULL);
    if(ret < 0) {
        json_object_set_new(err_info, "transcoder", json_string("failed to analyze stream info"));
        goto done;
    }
    AVCodecContext **dec_ctxs = av_mallocz_array(fmt_ctx->nb_streams, sizeof(AVCodecContext *));
    mp4proc->av->stream_ctx.decode = dec_ctxs;
    for(idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        AVStream *stream  = fmt_ctx->streams[idx];
        AVCodec  *decoder = avcodec_find_decoder(stream->codecpar->codec_id);
        if(!decoder) {
            json_object_set_new(err_info, "transcoder", json_string("[mp4] failed to find decoder for the stream"));
            break;
        }
        mp4proc->av->stats[idx].index_entry.preloaded = mp4proc->av -> stats[idx].index_entry.preloading;
        AVCodecContext *codec_ctx = avcodec_alloc_context3(decoder);
        dec_ctxs[idx] = codec_ctx;
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
            case AVMEDIA_TYPE_DATA:
            case AVMEDIA_TYPE_SUBTITLE:
                break;
            case AVMEDIA_TYPE_NB:
            case AVMEDIA_TYPE_ATTACHMENT:
            case AVMEDIA_TYPE_UNKNOWN:
            default:
                ret = AVERROR_INVALIDDATA; // unsupported stream type
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
    mp4proc->internal.callback.av_init_done(mp4proc);
} // end of atfp_mp4__preload_initial_packets_done


ASA_RES_CODE  atfp_mp4__av_init (atfp_mp4_t *mp4proc, void (*cb)(atfp_mp4_t *))
{
    asa_op_base_cfg_t *asaobj = mp4proc -> super.data.storage.handle;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asaobj->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
#define  AVIO_CTX_BUFFER_SIZE 2048    
    uint8_t  *avio_ctx_buffer = NULL;
    AVFormatContext *fmt_ctx = NULL;
    int ret = 0, idx = 0;
    if (!(fmt_ctx = avformat_alloc_context()))
        goto error;
    mp4proc->internal.callback.av_init_done = cb;
    *mp4proc->av = (atfp_av_ctx_t){.fmt_ctx=fmt_ctx, .decoder_flag=1};
    avio_ctx_buffer = av_malloc(AVIO_CTX_BUFFER_SIZE);
    fmt_ctx -> pb = avio_alloc_context(avio_ctx_buffer, AVIO_CTX_BUFFER_SIZE, 0,
              _map, atfp_avio__read_local_tmpbuf, NULL, NULL); // &app_seek_packet
    if (!fmt_ctx->pb || !avio_ctx_buffer)
        goto error;
    { // libavformat accesses input file synchronously
        lseek(asa_local->file.file, 0, SEEK_SET);
        ret = avformat_open_input(&fmt_ctx, NULL, NULL, NULL);
        if(ret < 0) 
            goto error;
        size_t pos_wholefile = atfp_mp4__mdat_body_pos(mp4proc);
        mp4proc->internal.mdat.pos_wholefile = pos_wholefile;
        fmt_ctx->pb ->pos = pos_wholefile;
        // erase content in local temp buffer, load initial packets from source
        lseek(asa_local->file.file, 0, SEEK_SET);
        mp4proc-> av-> stats = calloc(fmt_ctx->nb_streams, sizeof(atfp_stream_stats_t));
    } { // set up limit of preloading bytes size for async decoding
        uint8_t  min__num_init_pkts = ATFP_MP4__DEFAULT_NUM_INIT_PKTS;
        mp4proc->av->intermediate_data.decode.tot_num_pkts_avail = 0;
        for(idx = 0; idx < fmt_ctx->nb_streams; idx++) {
            AVStream *stream  = fmt_ctx->streams[idx];
            min__num_init_pkts = FFMIN(min__num_init_pkts, stream->nb_index_entries);
            mp4proc->av->intermediate_data.decode.tot_num_pkts_avail += stream ->nb_index_entries;
        }
        mp4proc->av->intermediate_data.decode.tot_num_pkts_fixed = mp4proc->av->intermediate_data.decode.tot_num_pkts_avail;
        mp4proc->av->intermediate_data.decode.report_interval = 0.15f;
        mp4proc->av->async_limit.num_init_pkts = min__num_init_pkts;
    }
    size_t  nbytes_to_load = atfp_ffmpeg__estimate_nb_pkt_preload(mp4proc->av,
            mp4proc->internal.mdat.pos_wholefile,  estimate_nb_initial_packet__continue_fn);
    // -------------
    int    chunk_idx  = mp4proc->internal.mdat.fchunk_seq;
    size_t offset     = mp4proc->internal.mdat.pos;
    ASA_RES_CODE asa_result = atfp_mp4__preload_packet_sequence (mp4proc, chunk_idx, offset,
            nbytes_to_load,  atfp_mp4__preload_initial_packets_done);
    if(asa_result != ASTORAGE_RESULT_ACCEPT)
        goto error;
    return asa_result;
error:
    json_object_set_new(mp4proc->super.data.error, "transcoder",
            json_string("[mp4] failed to initialize input format context"));
    atfp_mp4__av_deinit(mp4proc);
    return ASTORAGE_RESULT_OS_ERROR;
#undef  AVIO_CTX_BUFFER_SIZE    
} // end of atfp_mp4__av_init


static void  atfp_mp4__preload_subsequent_packets_done(atfp_mp4_t *mp4proc)
{ // TODO, seek first position in the local tmp buf
    asa_op_base_cfg_t *asasrc = mp4proc -> super.data.storage.handle;
    atfp_asa_map_t *_map = (atfp_asa_map_t *)asasrc->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
    asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
    lseek(asa_local->file.file, 0, SEEK_SET);
    AVFormatContext  *fmt_ctx = mp4proc->av ->fmt_ctx;
    for(int idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        mp4proc->av->stats[idx].index_entry.preloaded = mp4proc->av -> stats[idx].index_entry.preloading;
    }
    mp4proc->internal.callback.av_init_done(mp4proc);
} // end of atfp_mp4__preload_subsequent_packets_done


ASA_RES_CODE  atfp_mp4__av_preload_packets (atfp_mp4_t *mp4proc, size_t nbytes, void (*cb)(atfp_mp4_t *))
{
    ASA_RES_CODE result = ASTORAGE_RESULT_COMPLETE;
    size_t  start_pkt_pos = mp4proc->internal.mdat.pos_wholefile + mp4proc->internal.mdat.nb_preloaded;
    size_t  nbytes_to_load = 0;
    int     chunk_idx  = 0;
    size_t  offset     = 0;
    result = atfp_mp4__serial_to_segment_pos(mp4proc->super.data.spec, start_pkt_pos, &chunk_idx, &offset);
    if(result == ASTORAGE_RESULT_COMPLETE) {
        mp4proc->av->async_limit.max_nbytes_bulk = FFMIN(nbytes, mp4proc->internal.mdat.size);
        nbytes_to_load = atfp_ffmpeg__estimate_nb_pkt_preload(mp4proc->av, start_pkt_pos,
              estimate_nb_subsequent_packet__continue_fn);
    }
    if(nbytes_to_load > 0) {
        asa_op_base_cfg_t *asasrc = mp4proc -> super.data.storage.handle;
        atfp_asa_map_t *_map = (atfp_asa_map_t *)asasrc->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_localfs_cfg_t  *asa_local = atfp_asa_map_get_localtmp(_map);
        lseek(asa_local->file.file, 0, SEEK_SET);
        mp4proc->internal.callback.av_init_done = cb; // TODO, rename to avctx_done
        result = atfp_mp4__preload_packet_sequence (mp4proc, chunk_idx, offset,
            nbytes_to_load,  atfp_mp4__preload_subsequent_packets_done);
    }
    return result;
} // end of atfp_mp4__av_preload_packets



int  atfp_ffmpeg__next_local_packet(atfp_av_ctx_t *avctx)
{
    int ret = 0, idx = 0;
    size_t  num_pkts_avail = 0;
    AVFormatContext  *fmt_ctx = avctx ->fmt_ctx;
    for(idx = 0; idx < fmt_ctx->nb_streams; idx++) {
        size_t  preloaded = avctx->stats[idx].index_entry.preloaded;
        size_t  fetched   = avctx->stats[idx].index_entry.fetched;
        num_pkts_avail += preloaded - fetched;
    }
    if(num_pkts_avail > 0) {
        AVPacket  *pkt = &avctx->intermediate_data.decode.packet;
        atfp_mp4__av_packet_deinit(pkt);
        ret = av_read_frame(fmt_ctx, pkt);
        if(!ret) {
            int is_corrupted = (pkt->flags & (int)AV_PKT_FLAG_CORRUPT);
            ret = (pkt->stream_index < 0) || (is_corrupted != 0) || 
                (pkt->stream_index >= fmt_ctx->nb_streams);
            ret = ret * -1;
        }
        if(!ret) {
            idx = pkt->stream_index;
            avctx->stats[idx].index_entry.fetched ++;
            avctx->intermediate_data.decode.num_decoded_frames = 0;
            avctx->intermediate_data.decode.tot_num_pkts_avail -= 1;
        }
    } else { ret = 1; } // request to preload next packet
    return ret;
} // end of atfp_ffmpeg__next_local_packet


int  atfp_mp4__av_decode_packet(atfp_av_ctx_t *avctx)
{
    int ret = 0, got_frame = 0;
    uint16_t  num_decoded = avctx->intermediate_data.decode.num_decoded_frames;
    AVPacket *pkt  = &avctx->intermediate_data.decode.packet;
    AVFrame  *frm  = &avctx->intermediate_data.decode.frame;
    int stream_idx = pkt->stream_index;
    AVFormatContext  *fmt_ctx = avctx->fmt_ctx;
    AVStream         *stream = fmt_ctx->streams[stream_idx];
    AVCodecContext   *dec_ctx = avctx->stream_ctx.decode[stream_idx];
    if(num_decoded == 0) {
        if(pkt->size > 0) {
            av_packet_rescale_ts(pkt, stream->time_base, dec_ctx->time_base);
            // To handle codec-context draining and resume the operation, the alternative is to
            // flush internal state using avcodec_flush_buffers()
            ret =  avcodec_send_packet(dec_ctx, pkt);
        } else { // request to preload next packet
            ret = 1;
        }
    }
    if(ret < 0) {
        av_log(NULL, AV_LOG_ERROR, "Failed to send packet to decoder, pos: 0x%08x size:%d \n",
                (uint32_t)pkt->pos, pkt->size);        
    } else if(ret == 1) {
        // skipped, new input data required
    } else {
        ret = avcodec_receive_frame(dec_ctx, frm); // internally call av_frame_unref() to clean up previous frame
        if(ret == 0) {
            frm->pts = frm->best_effort_timestamp;            
            got_frame = 1;
        } else if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
            // new input data required (for EOF), or the current packet doesn't contain
            //  useful frame to decode (EAGAIN)
            ret = 1;
        } else {
            av_log(NULL, AV_LOG_ERROR, "Failed to get decoded frame, pos: 0x%08x size:%d \n",
                    (uint32_t)pkt->pos, pkt->size);
        }
    }
    if(got_frame)
        avctx->intermediate_data.decode.num_decoded_frames = 1 + num_decoded;
    return ret;
} // end of atfp_mp4__av_decode_packet


uint8_t  atfp_ffmpeg_avctx__has_done_decoding(atfp_av_ctx_t *avctx)
{ return avctx->intermediate_data.decode.tot_num_pkts_avail == 0; }

void  atfp_ffmpeg_avctx__monitor_progress(atfp_av_ctx_t *avctx, arpc_receipt_t  *rpc_receipt)
{ // TODO, re-design the interface only for progress update, with respect to scalability of API servers
    if(!rpc_receipt || !avctx)
        return;
    size_t  tot_num_pkts   = avctx->intermediate_data.decode.tot_num_pkts_fixed;
    size_t  num_pkts_avail = avctx->intermediate_data.decode.tot_num_pkts_avail;
    size_t  num_pkts_done  = tot_num_pkts - num_pkts_avail;
    float   _percent_done = 1.0f * num_pkts_done / tot_num_pkts;
    float   diff = _percent_done - avctx->intermediate_data.decode.percent_done;
    float   percent_interval = avctx->intermediate_data.decode.report_interval;
    if(diff < percent_interval)
        return;
    json_t *progress_info = json_object();
    json_object_set_new(progress_info, "progress", json_real((double)_percent_done));
    app_rpc_task_send_reply(rpc_receipt, progress_info, 0);
    json_decref(progress_info);
    avctx->intermediate_data.decode.percent_done = _percent_done;
} // end of atfp_ffmpeg_avctx__monitor_progress
