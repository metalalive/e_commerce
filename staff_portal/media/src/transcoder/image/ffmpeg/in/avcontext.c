#include "transcoder/image/ffmpeg.h"

static void  _atfp_img_src__avctx_init_decoder(atfp_av_ctx_t *_avctx, json_t *err_info)
{
    AVFormatContext  *_fmt_ctx = _avctx ->fmt_ctx;
    if(_fmt_ctx->nb_streams != 1) {
        json_object_set_new(err_info, "transcoder", json_string("[ff_in] invalid image type"));
        fprintf(stderr, "[atfp][img][ff-in] line:%d, currently this processor only support input"
                " muxed with only one (video) stream \n", __LINE__);
        return;
    }
    int ret = avformat_find_stream_info(_fmt_ctx, NULL);
    if(ret < 0) {
        json_object_set_new(err_info, "transcoder", json_string("failed to analyze stream info"));
        json_object_set_new(err_info, "err_code", json_integer(ret));
    } else {
        AVCodecContext **dec_ctxs = av_mallocz_array(_fmt_ctx->nb_streams, sizeof(AVCodecContext *));
        _avctx->stream_ctx.decode = dec_ctxs;
        AVStream *stream  = _fmt_ctx->streams[0];
        AVCodec  *decoder = avcodec_find_decoder(stream->codecpar->codec_id);
        if(!decoder) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to find decoder for the stream"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
            return;
        }
        AVCodecContext *codec_ctx = avcodec_alloc_context3(decoder);
        dec_ctxs[0] = codec_ctx;
        if(!codec_ctx) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to create decoder context of the stream"));
            return;
        }
        ret = avcodec_parameters_to_context(codec_ctx, stream->codecpar);
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to copy parameters from stream to decoder context"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
            return;
        }
        if(codec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
            codec_ctx->framerate = av_guess_frame_rate(_fmt_ctx, stream, NULL);
            ret = avcodec_open2(codec_ctx, decoder, NULL);
            if(codec_ctx->time_base.num == 0) { // for encoder, timebase should NOT be zero
                codec_ctx->time_base.num = 1;
                av_log(NULL, AV_LOG_INFO, "[atfp][img][ff-in][decoder] line:%d, zero time base \n", __LINE__);
            }
        } else {
            ret = AVERROR_INVALIDDATA; // unsupported stream type
        }
        if(ret < 0) {
            json_object_set_new(err_info, "transcoder", json_string("[ff_in] failed to open decoder for stream"));
            json_object_set_new(err_info, "err_code", json_integer(ret));
        }
    }
} // end of  _atfp_img_src__avctx_init_decoder

void atfp__image_src__avctx_init (atfp_av_ctx_t *_avctx, const char *filepath, json_t *err_info)
{
    AVFormatContext *_fmt_ctx = NULL;
    int ret = avformat_open_input(&_fmt_ctx, filepath, NULL, NULL);
    *_avctx = (atfp_av_ctx_t){.fmt_ctx=_fmt_ctx, .decoder_flag=1};
    if (ret == 0) {
        _atfp_img_src__avctx_init_decoder (_avctx, err_info);
    } else {
        json_object_set_new(err_info, "transcoder",
                json_string("[ff_in] failed to initialize input format context"));
        json_object_set_new(err_info, "err_code", json_integer(ret));
    }
} // end of  atfp__image_src__avctx_init


void atfp__image_src__avctx_deinit (atfp_av_ctx_t *_avctx)
{
    AVCodecContext **dec_ctxs = _avctx->stream_ctx.decode;
    AVPacket  *pkt = &_avctx->intermediate_data.decode.packet;
    AVFrame   *frm = &_avctx->intermediate_data.decode.frame;
    if(dec_ctxs) {
        if(dec_ctxs[0])
            avcodec_free_context(&dec_ctxs[0]);
        av_freep(&_avctx->stream_ctx.decode);
    }
    if(frm->format != -1)
        av_frame_unref(frm);
    if(pkt->pos != -1)
        av_packet_unref(pkt);
    if(_avctx->fmt_ctx)
        avformat_close_input(&_avctx->fmt_ctx);
    *_avctx = (atfp_av_ctx_t) {0};
} // end of  atfp__image_src__avctx_deinit


int  atfp__image_src__avctx_fetch_next_packet(atfp_av_ctx_t *_avctx)
{
    int ret = ATFP_AVCTX_RET__OK;
    AVFormatContext  *fmt_ctx = _avctx ->fmt_ctx;
    AVPacket  *pkt = &_avctx->intermediate_data.decode.packet;
    av_packet_unref(pkt);
    // assert(pkt->pos == -1);
    ret = av_read_frame(fmt_ctx, pkt);
    // Note , may also go backwards to previous frame using av_seek_frame() (if seekable)
    if(ret == AVERROR_EOF) {
        ret = 1; // end of file, avio_feof(fmt_ctx ->pb) might not be consistent
        // TODO, figure out why sometimes AVERROR_EOF is returned without setting `eof_reached`
    } else if (ret == ATFP_AVCTX_RET__OK) {
        int is_corrupted = (pkt->flags & (int)AV_PKT_FLAG_CORRUPT);
        ret = (pkt->stream_index < 0) || (is_corrupted != 0) || 
            (pkt->stream_index >= fmt_ctx->nb_streams);
        if(ret)
            ret = AVERROR_INVALIDDATA;
    }
    if(!ret)
        _avctx->intermediate_data.decode.num_decoded_frames = 0;
    return ret;
} // end of  atfp__image_src__avctx_fetch_next_packet


int  atfp__image_src__avctx_decode_curr_packet(atfp_av_ctx_t *_avctx)
{
    int ret = ATFP_AVCTX_RET__OK, got_frame = 0;
    uint16_t  num_decoded = _avctx->intermediate_data.decode.num_decoded_frames;
    AVPacket *pkt  = &_avctx->intermediate_data.decode.packet;
    AVFrame  *frm  = &_avctx->intermediate_data.decode.frame;
    int stream_idx = pkt->stream_index;
    AVFormatContext  *fmt_ctx = _avctx->fmt_ctx;
    AVStream         *stream = fmt_ctx->streams[stream_idx];
    AVCodecContext   *dec_ctx = _avctx->stream_ctx.decode[stream_idx];
    if(num_decoded == 0) {
        if(pkt->size > 0 && dec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
            av_packet_rescale_ts(pkt, stream->time_base, dec_ctx->time_base);
            ret =  avcodec_send_packet(dec_ctx, pkt);
        } else { // request to preload next packet
            ret = 1;
        } // the function only accepts packet from video stream, any sideband data will be discarded
    }
    if(ret < 0) {
        av_log(NULL, AV_LOG_ERROR, "[atfp][img][ff-in][av-ctx] line:%d, Failed to send packet to"
                " decoder, pos: 0x%08x size:%d \n", __LINE__, (uint32_t)pkt->pos, pkt->size);        
    } else if(ret == 1) {
        // skipped, new input data required
    } else {
        ret = avcodec_receive_frame(dec_ctx, frm); // internally call av_frame_unref() to clean up previous frame
        if(ret == ATFP_AVCTX_RET__OK) {
            frm->pts = frm->best_effort_timestamp;            
            got_frame = 1;
        } else if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
            // new input data required (for EOF), or the current packet doesn't contain
            //  useful frame to decode (EAGAIN)
            ret = 1;
        } else {
            av_log(NULL, AV_LOG_ERROR, "[atfp][img][ff-in][av-ctx] line:%d, Failed to get decoded"
                    " frame, pos: 0x%08x size:%d \n", __LINE__, (uint32_t)pkt->pos, pkt->size);
        }
    }
    if(got_frame)
        _avctx->intermediate_data.decode.num_decoded_frames = 1 + num_decoded;
    return ret;
} // end of  atfp__image_src__avctx_decode_curr_packet


uint8_t  atfp__image_src__avctx_has_done_decoding(atfp_av_ctx_t *_avctx)
{ return (uint8_t) _avctx->fmt_ctx->pb ->eof_reached; }
