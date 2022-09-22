#include <unistd.h>
#include <string.h>
#include "transcoder/video/common.h"
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

#define  HLS_PLAYLIST_TYPE__VOD   2
#define  HLS_SEGMENT_TYPE__FMP4   1
#define  HLS_TIME__IN_SECONDS     30 // TODO, adjust time period of each segment with respect to total duration
#define  HLS_DELETE_THRESHOLD__IN_SECONDS     3

void  atfp_hls__av_deinit(atfp_hls_t *hlsproc)
{
    AVFormatContext *fmt_ctx  = hlsproc->av->fmt_ctx;
    atfp_stream_enc_ctx_t *enc_ctxs = hlsproc->av ->stream_ctx.encode;
    AVPacket  *pkt = & hlsproc->av->intermediate_data.encode.packet;
    AVFrame   *frm = & hlsproc->av->intermediate_data.encode.frame;
    av_packet_unref(pkt);
    av_frame_unref(frm);
    // ffmpeg internally allocates memory via call to avformat_write_header(), to free up all
    // internal space, one has to call av_write_trailer() exactly once for each transcoding request.
    atfp_hls__av_local_write_finalize(hlsproc->av);
    if(enc_ctxs) {
        int nb_streams = fmt_ctx ? fmt_ctx->nb_streams: 0;
        for(int idx = 0; idx < nb_streams; idx++) {
            if(enc_ctxs[idx].enc_ctx)
                avcodec_free_context(&enc_ctxs[idx].enc_ctx);
            if(enc_ctxs[idx].filter_graph) {
                avfilter_graph_free(&enc_ctxs[idx].filter_graph);
            }
        }
        av_freep(&hlsproc->av ->stream_ctx.encode);
    }
    if(fmt_ctx) {
        avformat_free_context(fmt_ctx);
        hlsproc->av->fmt_ctx = NULL;
    }
} // end of atfp_hls__av_deinit


static int  atfp_hls__av_setup_options(AVFormatContext *fmt_ctx, const char *local_basepath)
{
    // if((fmt_ctx->oformat->flags & AVFMT_NOFILE) == 0)
    //     goto error;
    AVDictionary *options = NULL; // TODO, parameterize
    av_dict_set_int(&options, "hls_playlist_type", (int64_t)HLS_PLAYLIST_TYPE__VOD, 0); // vod
    av_dict_set_int(&options, "hls_segment_type", (int64_t)HLS_SEGMENT_TYPE__FMP4, 0); // fmp4
    av_dict_set_int(&options, "hls_time", (int64_t)HLS_TIME__IN_SECONDS, 0);
    av_dict_set_int(&options, "hls_delete_threshold", (int64_t)HLS_DELETE_THRESHOLD__IN_SECONDS, 0);
    // 1000 KB, not implemented yet as of ffmpeg v4.3.4
    //// av_dict_set_int(&options, "hls_segment_size", (int64_t)1024000, 0);
    // will be prepended to each segment entry in final playlist, TODO, enable this option and finish the playback API
    //// av_dict_set(&options, "hls_base_url", "/file?id=x4eyy5i&segment=", 0);
    {
        size_t path_sz = strlen(local_basepath) + 1 + sizeof(HLS_SEGMENT_FILENAME_TEMPLATE) + 1;
        char   path[path_sz];
        size_t nwrite = snprintf(&path[0], path_sz, "%s/%s", local_basepath, HLS_SEGMENT_FILENAME_TEMPLATE);
        path[nwrite++] = 0;
        av_dict_set(&options, "hls_segment_filename",  &path[0], 0);
    }
    av_dict_set(&options, "hls_fmp4_init_filename", HLS_FMP4_FILENAME, 0);
    // At this point, avformat_write_header() does NOT write any bytes to playlist
    int err = avformat_write_header(fmt_ctx, &options);
    av_dict_free(&options);
    if (err == 0) {
        int is_output = 1;
        av_dump_format(fmt_ctx, 0, "some_output_file_path", is_output);
    } else {
        char errbuf[128];
        av_strerror(err, &errbuf[0], 128);
        av_log(NULL, AV_LOG_ERROR, "Error occurred when opening output file, %s \n", &errbuf[0]);
    }
    return err;
} // end of atfp_hls__av_setup_options


static void _atfp_config_dst_video_codecctx(AVCodecContext *dst, AVCodecContext *src, json_t *o_spec, json_t *elm_st_map)
{
    uint32_t height = 0, width = 0;
    uint8_t  fps    = 0;
    ATFP_VIDEO__READ_SPEC(o_spec, elm_st_map, height, width, fps);
    dst->height = FFMIN(src->height, height);
    dst->width  = FFMIN(src->width , width);
    dst->framerate = (AVRational){num:fps, den:1};
    if (dst->codec->pix_fmts) // take first format from list of supported formats
        dst->pix_fmt = dst->codec->pix_fmts[0];
    else
        dst->pix_fmt = src->pix_fmt;
    dst->sample_aspect_ratio = src->sample_aspect_ratio;
    dst->time_base = src->time_base;
    dst->bit_rate  = src->bit_rate;    
} // end of _atfp_config_dst_video_codecctx

static void _atfp_config_dst_audio_coderctx(AVCodecContext *dst, AVCodecContext *src, json_t *o_spec, json_t *elm_st_map)
{
    json_t *elm_st_key_obj = json_object_get(json_object_get(o_spec, "__internal__"), "audio_key");
    const char *elm_st_key = json_string_value(elm_st_key_obj);
    json_t *attribute  = json_object_get(json_object_get(elm_st_map, elm_st_key), "attribute");
    uint32_t bitrate_kbps = json_integer_value(json_object_get(attribute, "bitrate_kbps"));
    dst->bit_rate  = FFMIN(src->bit_rate, (bitrate_kbps * 1000 - 1));
    dst->sample_rate = src->sample_rate;
    dst->channel_layout = src->channel_layout;
    dst->channels = av_get_channel_layout_nb_channels(dst->channel_layout);
    // take first format from list of supported formats
    dst->sample_fmt = src->codec->sample_fmts[0];
    dst->time_base  = (AVRational){1, dst->sample_rate};
} // end of _atfp_config_dst_audio_coderctx


static int atfp_hls__av_encoder_init(atfp_av_ctx_t *avctx_dst, atfp_av_ctx_t *avctx_src,
        json_t *o_spec, json_t *elm_st_map, json_t *err_info)
{
    AVFormatContext *ofmt_ctx  = avctx_dst->fmt_ctx;
    AVFormatContext *ifmt_ctx  = avctx_src->fmt_ctx;
    AVCodecContext       **dec_ctxs = avctx_src ->stream_ctx.decode;
    atfp_stream_enc_ctx_t *enc_ctxs = av_mallocz_array(ifmt_ctx->nb_streams, sizeof(atfp_stream_enc_ctx_t));
    avctx_dst ->stream_ctx.encode = enc_ctxs;
    int err = 0, idx = 0;
    for(idx = 0; idx < ifmt_ctx->nb_streams; idx++) {
        AVStream  *stream_out = avformat_new_stream(ofmt_ctx, NULL);
        if(!stream_out) {
            json_object_set_new(err_info, "transcoder", json_string("[hls] failed to create stream"));
            err = AVERROR(ENOMEM);
            break;
        }
        AVCodecContext *dec_ctx = dec_ctxs[idx];
        if(dec_ctx->codec_type == AVMEDIA_TYPE_VIDEO || dec_ctx->codec_type == AVMEDIA_TYPE_AUDIO) {
            const AVCodec *encoder = avcodec_find_encoder(dec_ctx->codec_id); // do NOT reference dec_ctx->codec;
            if(!encoder) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] invalid decoder ID"));
                err = AVERROR(EINVAL);
                break;
            }
            AVCodecContext *enc_ctx = avcodec_alloc_context3(encoder);
            enc_ctxs[idx].enc_ctx = enc_ctx;
            if(!enc_ctx) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] failed to create encoder context of the stream"));
                err = AVERROR(ENOMEM);
                break;
            }
            if(dec_ctx->codec_type == AVMEDIA_TYPE_VIDEO) {
                _atfp_config_dst_video_codecctx(enc_ctx, dec_ctx, o_spec, elm_st_map);
            } else if(dec_ctx->codec_type == AVMEDIA_TYPE_AUDIO) {
                _atfp_config_dst_audio_coderctx(enc_ctx, dec_ctx, o_spec, elm_st_map);
            }
            err = avcodec_open2(enc_ctx, encoder, NULL);
            if(err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] failed to open encoder context of the stream"));
                break;
            }
            err = avcodec_parameters_from_context(stream_out->codecpar, enc_ctx);
            if (err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[hls] Failed to copy encoder parameters to output stream"));
                break;
            }
            stream_out->time_base = enc_ctx->time_base;            
        } else { // for other valid stream types
            AVStream  *stream_in  = ifmt_ctx->streams[idx];
            err = avcodec_parameters_copy(stream_out->codecpar, stream_in->codecpar);
            if (err < 0) {
                json_object_set_new(err_info, "transcoder", json_string("[mp4] Failed to copy parameters from input stream"));
                break;
            }
            stream_out->time_base = stream_in->time_base;            
        }
    } // end of stream iteration
    return err;
} // end of atfp_hls__av_encoder_init


int  atfp_hls__av_init(atfp_hls_t *hlsproc)
{
    int err = 0;
    AVFormatContext *fmt_ctx = NULL;
    atfp_t  *processor = &hlsproc->super;
    assert(processor->data.version);
    assert(processor->data.spec);
    json_t  *req_spec  =  processor->data.spec;
    json_t  *output  = json_object_get(json_object_get(req_spec, "outputs"), processor->data.version);
    json_t  *elm_st_map = json_object_get(req_spec, "elementary_streams");
    const char *local_basepath = hlsproc->asa_local.super.op.mkdir.path.origin;
    { // Note everything in `spec` field has to be validated at app server
        const char *fmt_name = json_string_value(json_object_get(output, "container"));
        size_t playlist_path_sz = strlen(local_basepath) + 1 + sizeof(HLS_PLAYLIST_FILENAME);
        char   playlist_path [playlist_path_sz];
        size_t nwrite = snprintf(&playlist_path[0], playlist_path_sz, "%s/%s",
                local_basepath, HLS_PLAYLIST_FILENAME);
        playlist_path[nwrite++] = 0;
        err = avformat_alloc_output_context2(&fmt_ctx, NULL, fmt_name, &playlist_path[0]);
        hlsproc->av->fmt_ctx = fmt_ctx;
    } // does output format context require low-level AVIO context ?
    atfp_av_ctx_t   *avctx_src = NULL;
    if (!err) {
        atfp_t   *fp_dst = &hlsproc->super;
        asa_op_base_cfg_t  *asa_dst = fp_dst->data.storage.handle;
        atfp_asa_map_t     *map = asa_dst->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG];
        asa_op_base_cfg_t  *asa_src = atfp_asa_map_get_source(map);
        // TODO, refactor in case app uses different multimedia codec library
        atfp_t   *fp_src =  asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
        if((fp_dst->backend_id != fp_src->backend_id) || (fp_dst->backend_id == ATFP_BACKEND_LIB__UNKNOWN))
        {
            json_object_set_new(fp_dst ->data.error, "transcoder", json_string("[hls] invalid backend library in source file processor"));
            err = AVERROR(EINVAL);
        }
        // from this point, it ensures both src/dst sides apply the same backend
        // library for transcoding process.
        avctx_src = ((atfp_hls_t *)fp_src)->av; // TODO, better design for identifying source format
    }
    if (!err)
        err  = atfp_hls__av_encoder_init(hlsproc->av, avctx_src, output,
                  elm_st_map, processor->data.error);
    if (!err)
        err  = atfp_hls__av_setup_options(fmt_ctx, local_basepath);
    // TODO, add function and data structure to monitor how many segment files are done by encoding
    //  function and ready to traansfer to destination storage. This is for certain types of output
    //  formats which support content segmentation such as HLS or mpeg-DASH
    if(err) {
        json_t *err_detail = json_object_get(processor->data.error, "transcoder");
        if(!err_detail) {
            json_object_set_new(processor->data.error, "transcoder",
                json_string("[hls] failed to initialize output format context"));
        }
        atfp_hls__av_deinit(hlsproc);
    } else {
        hlsproc->av->intermediate_data.encode._final.file_header_wrote = 1;
    }
    return err;
} // end of atfp_hls__av_init


static int  _atfp_hls__av_encode_processing(atfp_av_ctx_t *dst, AVFrame *frame, int8_t stream_idx)
{
    int ret = 0;
    atfp_stream_enc_ctx_t  *st_encode_ctx = &dst->stream_ctx.encode[stream_idx];
    AVPacket  *packet = &dst->intermediate_data.encode.packet;
    uint16_t   num_encoded_pkts = dst->intermediate_data.encode. num_encoded_pkts;
    if(num_encoded_pkts == 0) {
        ret = avcodec_send_frame(st_encode_ctx->enc_ctx, frame);
        if (ret < 0) {
            av_log(NULL, AV_LOG_ERROR, "Error sending a frame for encoding.\n");
            goto done;
        }
    }
#if  1
    ret = avcodec_receive_packet(st_encode_ctx->enc_ctx, packet);
#else
    ret = AVERROR(EAGAIN);
#endif
    if (ret == 0) {
        packet-> stream_index = stream_idx;
        av_packet_rescale_ts(packet, st_encode_ctx->enc_ctx->time_base,
                dst->fmt_ctx->streams[stream_idx]->time_base);
        if(packet->duration == 0) { // always happens to video encoder
            packet->duration = (frame && frame->pkt_duration) ? frame->pkt_duration: 1;
        }
        dst->intermediate_data.encode.num_encoded_pkts = 1 + num_encoded_pkts;
    } else if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
        ret = ATFP_AVCTX_RET__NEED_MORE_DATA;
        dst->intermediate_data.encode.num_encoded_pkts = 0;
    } else { // ret < 0
        av_log(NULL, AV_LOG_ERROR, "Error on receiving encoded packet.\n");
    }
done:
    return ret;
} // end of _atfp_hls__av_encode_processing


int  atfp_hls__av_encode_processing(atfp_av_ctx_t *dst) {
    int ret = ATFP_AVCTX_RET__OK;
    if(dst) {
        int8_t stream_idx =  dst->intermediate_data.encode.stream_idx;
        AVFrame   *frame  = &dst->intermediate_data.encode.frame;
        ret = _atfp_hls__av_encode_processing(dst, frame, stream_idx);
    } else {
        ret = AVERROR(EINVAL);
    }
    return ret;
} // end of atfp_hls__av_encode_processing


int   atfp_hls__av_encode__finalize_processing(atfp_av_ctx_t *dst) {
    int ret = AVERROR(EINVAL);
    if(dst) {
        int8_t nb_streams_in = (int8_t) dst->intermediate_data.encode._final.filt_stream_idx;
        int8_t stream_idx    = (int8_t) dst->intermediate_data.encode._final.enc_stream_idx;
        if (nb_streams_in > stream_idx) {
            ret = _atfp_hls__av_encode_processing(dst, NULL, stream_idx);
            if((ret == 1) && (nb_streams_in > stream_idx)) {
                ++stream_idx;
            }
        } else { // packets already flushed from all encoders, skip
            ret = ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER;
        }
        dst->intermediate_data.encode._final.enc_stream_idx = stream_idx;
    }
    return ret;
} // end of atfp_hls__av_encode__finalize_processing


uint8_t  atfp_av_encoder__has_done_flushing(atfp_av_ctx_t *dst)
{
    int8_t nb_streams_in = (int8_t) dst->intermediate_data.encode._final.filt_stream_idx;
    int8_t stream_idx    = (int8_t) dst->intermediate_data.encode._final.enc_stream_idx;
    return  (nb_streams_in > 0) && (nb_streams_in == stream_idx);
}


int   atfp_hls__av_local_write(atfp_av_ctx_t *dst)
{
#if 1
    AVPacket  *packet = &dst->intermediate_data.encode.packet;
    int ret = av_interleaved_write_frame(dst->fmt_ctx, packet);
    return ret;
#else
    return ATFP_AVCTX_RET__OK;
#endif
}

int   atfp_hls__av_local_write_finalize(atfp_av_ctx_t *dst)
{
    uint8_t  trailer_wrote = dst->intermediate_data.encode._final.file_trailer_wrote;
    uint8_t  header_wrote  = dst->intermediate_data.encode._final.file_header_wrote;
    int ret = 0;
    if(header_wrote && !trailer_wrote) {
        ret = av_write_trailer(dst->fmt_ctx);
        trailer_wrote = 1;
    }
    dst->intermediate_data.encode._final.file_trailer_wrote = trailer_wrote;
    return (ret < 0) ? ret: ATFP_AVCTX_RET__NEED_MORE_DATA;
}

uint8_t  atfp_av__has_done_processing(atfp_av_ctx_t *dst) {
    return  dst->intermediate_data.encode._final.file_trailer_wrote ;
}
