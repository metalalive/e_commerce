#ifndef MEDIA__TRANSCODER__COMMON_FFMPEG_H
#define MEDIA__TRANSCODER__COMMON_FFMPEG_H
#ifdef __cplusplus
extern "C" {
#endif

#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavutil/opt.h>

static inline int
atfp_common__ffm_filter_processing(atfp_av_ctx_t *dst, AVFrame *frame_origin, int8_t stream_idx) {
    int                    ret = ATFP_AVCTX_RET__OK;
    AVFrame               *frame_filt = &dst->intermediate_data.encode.frame;
    atfp_stream_enc_ctx_t *st_encode_ctx = &dst->stream_ctx.encode[stream_idx];
    uint16_t               num_filtered_frms = dst->intermediate_data.encode.num_filtered_frms;
    if (num_filtered_frms == 0) {
        ret = av_buffersrc_add_frame_flags(
            st_encode_ctx->filt_src_ctx, frame_origin,
            AV_BUFFERSRC_FLAG_KEEP_REF
        ); // reference the same decoded frame in multiple filters
        if (ret < 0) {
            av_log(
                NULL, AV_LOG_ERROR,
                "[atfp][common][ffm] line:%d, error on filter graph,"
                "frm_ori:%p, str_idx:%d, err:%d \n",
                __LINE__, frame_origin, stream_idx, ret
            );
            goto done;
        }
    }
    av_frame_unref(frame_filt);
    ret = av_buffersink_get_frame(st_encode_ctx->filt_sink_ctx, frame_filt);
    if (ret == ATFP_AVCTX_RET__OK) {
        frame_filt->pict_type = AV_PICTURE_TYPE_NONE;
        dst->intermediate_data.encode.num_filtered_frms = 1 + num_filtered_frms;
    } else { // ret < 0
        if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
            ret = ATFP_AVCTX_RET__NEED_MORE_DATA; // the filter has finished filtering source frame,
                                                  // request for next one
            dst->intermediate_data.encode.num_filtered_frms = 0;
        } else {
            av_log(NULL, AV_LOG_WARNING, "error when pulling filtered frame from filters\n");
        }
    }
done:
    return ret;
} // end of  atfp_common__ffm_filter_processing

static inline int atfp_common__ffm_encode_processing(atfp_av_ctx_t *dst, AVFrame *frame, int8_t stream_idx) {
    int                    ret = 0;
    atfp_stream_enc_ctx_t *st_encode_ctx = &dst->stream_ctx.encode[stream_idx];
    AVPacket              *packet = &dst->intermediate_data.encode.packet;
    uint16_t               num_encoded_pkts = dst->intermediate_data.encode.num_encoded_pkts;
    if (num_encoded_pkts == 0) {
        ret = avcodec_send_frame(st_encode_ctx->enc_ctx, frame);
        if (ret < 0) {
            av_log(NULL, AV_LOG_ERROR, "Error sending a frame for encoding.\n");
            goto done;
        }
    }
#if 1
    ret = avcodec_receive_packet(st_encode_ctx->enc_ctx, packet);
#else
    if (num_encoded_pkts > 20)
        ret = AVERROR(EBUSY);
#endif
    if (ret == 0) {
        packet->stream_index = stream_idx;
        av_packet_rescale_ts(
            packet, st_encode_ctx->enc_ctx->time_base, dst->fmt_ctx->streams[stream_idx]->time_base
        );
        if (packet->duration == 0) // always happens to video encoder
            packet->duration = (frame && frame->pkt_duration) ? frame->pkt_duration : 1;
        dst->intermediate_data.encode.num_encoded_pkts = 1 + num_encoded_pkts;
    } else if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
        ret = ATFP_AVCTX_RET__NEED_MORE_DATA;
        dst->intermediate_data.encode.num_encoded_pkts = 0;
    } else { // ret < 0
        av_log(NULL, AV_LOG_ERROR, "Error on receiving encoded packet.\n");
    }
done:
    return ret;
} // end of atfp_common__ffm_encode_processing

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__COMMON_FFMPEG_H
