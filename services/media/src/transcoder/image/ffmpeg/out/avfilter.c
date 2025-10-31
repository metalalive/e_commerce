#include <libgen.h>
#include "app_cfg.h"
#include "transcoder/image/ffmpeg.h"
#include "transcoder/common/ffmpeg.h"
#include "utils.h"

#define AVFILTER_INPUT_PAD_LABEL  "nodestart"
#define AVFILTER_OUTPUT_PAD_LABEL "nodesink"

#define FILT_SPEC_MASK  "movie=%s/%s/%s"
#define FILT_SPEC_SCALE "scale=%u:%u"
#define FILT_SPEC_CROP  "crop=%u:%u:%d:%d"

#if 1
    #define FILT_SPEC_PATTERN \
        FILT_SPEC_MASK "," FILT_SPEC_SCALE "[mask];" \
                       "[" AVFILTER_INPUT_PAD_LABEL "]" FILT_SPEC_CROP "," FILT_SPEC_SCALE "[fg];" \
                       "[fg][mask] overlay=0:0 [" AVFILTER_OUTPUT_PAD_LABEL "]"
#else
    #define FILT_SPEC_PATTERN \
        "[" AVFILTER_INPUT_PAD_LABEL "]" FILT_SPEC_CROP "," FILT_SPEC_SCALE "[" AVFILTER_OUTPUT_PAD_LABEL "]"
#endif
static int _atfp_img__gen_filter_spec(json_t *filt_spec, char *out, size_t out_sz) {
    int         err = 0;
    json_t     *_msk_item = json_object_get(filt_spec, "mask");
    json_t     *_crop_item = json_object_get(filt_spec, "crop");
    json_t     *_scale_item = json_object_get(filt_spec, "scale");
    uint32_t    scale_h = json_integer_value(json_object_get(_scale_item, "height"));
    uint32_t    scale_w = json_integer_value(json_object_get(_scale_item, "width"));
    uint32_t    crop_h = json_integer_value(json_object_get(_crop_item, "height"));
    uint32_t    crop_w = json_integer_value(json_object_get(_crop_item, "width"));
    int         crop_pos_x = json_integer_value(json_object_get(_crop_item, "x"));
    int         crop_pos_y = json_integer_value(json_object_get(_crop_item, "y"));
    const char *_msk_patt_label = json_string_value(json_object_get(_msk_item, "pattern"));
    // TODO, save path to mask file in filter spec, instead of calling global
    // config object and retrieving the field.
    app_cfg_t     *acfg = app_get_global_cfg();
    aav_cfg_img_t *_imgcfg = &acfg->transcoder.output.image;
    const char    *sys_basepath = acfg->env_vars.sys_base_path;
    const char    *msk_idxpath = _imgcfg->mask.indexpath;
#define RUNNER(fullpath) atfp_image_mask_pattern_index(fullpath)
    json_t *msk_fmap = PATH_CONCAT_THEN_RUN(sys_basepath, msk_idxpath, RUNNER);
#undef RUNNER
    if (msk_fmap) { // avfilter_graph_parse_ptr() will examine existence of the mask pattern file
#if 1
        size_t buf_sz = strlen(msk_idxpath);
        char   buf[buf_sz];
        strcpy(buf, msk_idxpath);
        char *msk_basepath = dirname(buf);
        assert(msk_basepath);
        const char *patt_filename = json_string_value(json_object_get(msk_fmap, _msk_patt_label));
        size_t      nwrite = snprintf(
            out, out_sz, FILT_SPEC_PATTERN, sys_basepath, msk_basepath, patt_filename, scale_w, scale_h,
            crop_w, crop_h, crop_pos_x, crop_pos_y, scale_w, scale_h
        );
#else
        size_t nwrite = snprintf(
            out, out_sz, FILT_SPEC_PATTERN, crop_w, crop_h, crop_pos_x, crop_pos_y, scale_w, scale_h
        );
#endif
        if (nwrite >= out_sz) {
            err = AVERROR(ENOMEM);
            av_log(
                NULL, AV_LOG_ERROR,
                "[atfp][img][ff_out][filter] line:%d,"
                " failed to print filter spec \n",
                __LINE__
            );
        }
        json_decref(msk_fmap);
    } else {
        err = AVERROR(EINVAL);
    }
    return err;
} // end of  _atfp_img__gen_filter_spec
#undef FILT_SPEC_MASK
#undef FILT_SPEC_SCALE
#undef FILT_SPEC_CROP
#undef FILT_SPEC_PATTERN

static AVFilterContext *_atfp_img__setup_filt_ctx(
    AVFilterGraph *filt_grf, const char *v_buf_label, const char *name, const char *arg, AVFilterInOut *_inout
) {
    AVFilterContext *ctx_out = NULL;
    const AVFilter  *buffer = avfilter_get_by_name(v_buf_label);
    if (!buffer) {
        av_log(
            NULL, AV_LOG_ERROR,
            "[atfp][img][ff_out][filter] line:%d, avfilter"
            " not found\n",
            __LINE__
        );
        goto done;
    }
    int err = avfilter_graph_create_filter(&ctx_out, buffer, name, arg, NULL, filt_grf);
    if ((err < 0) || (!ctx_out)) {
        av_log(
            NULL, AV_LOG_ERROR,
            "[atfp][img][ff_out][filter] line:%d, failed to"
            " create filter context, code:%d, obj:%p \n",
            __LINE__, err, ctx_out
        );
        if (ctx_out)
            avfilter_free(ctx_out);
        ctx_out = NULL;
    }
done:
    _inout->filter_ctx = ctx_out;
    _inout->pad_idx = 0;
    _inout->next = NULL;
    return ctx_out;
} // end of  _atfp_img__setup_filt_ctx

void atfp__image_dst__avfilt_init(
    atfp_av_ctx_t *src, atfp_av_ctx_t *dst, json_t *filt_spec, json_t *err_info
) {
    int                    idx = 0, err = 0;
    AVFormatContext       *ofmt_ctx = dst->fmt_ctx;
    AVCodecContext        *_img_dec_ctx = NULL;
    atfp_stream_enc_ctx_t *_img_enc_ctx = NULL;
    for (idx = 0; idx < ofmt_ctx->nb_streams; idx++) {
        enum AVMediaType codectype = ofmt_ctx->streams[idx]->codecpar->codec_type;
        if (codectype == AVMEDIA_TYPE_VIDEO) {
            _img_dec_ctx = src->stream_ctx.decode[idx];
            _img_enc_ctx = &dst->stream_ctx.encode[idx];
            break; // only consider the first found video stream
        }
    } // end of loop
    if (!_img_dec_ctx || !_img_enc_ctx || !_img_enc_ctx->enc_ctx) {
        av_log(
            NULL, AV_LOG_INFO,
            "[atfp][img][ff_out][filter] line:%d,"
            " no decoder/encoder provided \n",
            __LINE__
        );
        json_object_set_new(
            err_info, "transcoder", json_string("[img][ff_out][filter] missing decoder/encoder")
        );
        err = AVERROR(EINVAL);
    } else {
        AVFilterInOut *filt_out = avfilter_inout_alloc();
        AVFilterInOut *filt_in = avfilter_inout_alloc();
        _img_enc_ctx->filter_graph = avfilter_graph_alloc();
        if (!filt_out || !filt_in || !_img_enc_ctx->filter_graph) {
            err = AVERROR(ENOMEM);
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out][filter]"
                            " failed to alloc memory for filter in/out")
            );
            goto done;
        }
#define NBYTES_FILTER_SPEC_RAW 256
        char filter_spec_raw[NBYTES_FILTER_SPEC_RAW] = {0};
        char filter_src_arg[NBYTES_FILTER_SPEC_RAW] = {0};
        err = _atfp_img__gen_filter_spec(filt_spec, &filter_spec_raw[0], NBYTES_FILTER_SPEC_RAW);
        if (err) {
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out][filter]"
                            " failed to generate filter spec")
            );
            goto done;
        }
        {
            int nwrite = snprintf(
                &filter_src_arg[0], NBYTES_FILTER_SPEC_RAW,
                "video_size=%dx%d:pix_fmt=%d:time_base=%d/%d:pixel_aspect=%d/%d", _img_dec_ctx->width,
                _img_dec_ctx->height, _img_dec_ctx->pix_fmt, _img_dec_ctx->time_base.num,
                _img_dec_ctx->time_base.den, _img_dec_ctx->sample_aspect_ratio.num,
                _img_dec_ctx->sample_aspect_ratio.den
            );
            if (nwrite >= NBYTES_FILTER_SPEC_RAW) {
                av_log(
                    NULL, AV_LOG_ERROR,
                    "[atfp][img][ff-out][filter] line:%d, filter"
                    " source arg error \n",
                    __LINE__
                );
                json_object_set_new(
                    err_info, "transcoder",
                    json_string("[img][ff-out][filter]"
                                " failed to generate filter spec")
                );
                err = AVERROR(ENOMEM);
                goto done;
            }
        }
#undef NBYTES_FILTER_SPEC_RAW
        // the filtering components should be connected as following :
        // filt_out --> filt_src_ctx --> filt_graph(spec parsed) --> filt_sink_ctx --> filt_in
        _img_enc_ctx->filt_src_ctx = _atfp_img__setup_filt_ctx(
            _img_enc_ctx->filter_graph, "buffer", "in", &filter_src_arg[0], filt_out
        );
        _img_enc_ctx->filt_sink_ctx =
            _atfp_img__setup_filt_ctx(_img_enc_ctx->filter_graph, "buffersink", "out", NULL, filt_in);
        if (!_img_enc_ctx->filt_src_ctx || !_img_enc_ctx->filt_sink_ctx) {
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out][filter]"
                            " failed to create filter context")
            );
            err = AVERROR(EINVAL);
            goto done;
        } // Endpoints for the filter graph.
        err = av_opt_set_bin(
            _img_enc_ctx->filt_sink_ctx, "pix_fmts", (const uint8_t *)&_img_enc_ctx->enc_ctx->pix_fmt,
            sizeof(_img_enc_ctx->enc_ctx->pix_fmt), AV_OPT_SEARCH_CHILDREN
        );
        if (err < 0) {
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out]"
                            "[filter] failed to set option pix_fmts at the sink")
            );
            goto done;
        }
        filt_out->name = av_strdup(AVFILTER_INPUT_PAD_LABEL);
        filt_in->name = av_strdup(AVFILTER_OUTPUT_PAD_LABEL);
        err =
            avfilter_graph_parse_ptr(_img_enc_ctx->filter_graph, filter_spec_raw, &filt_in, &filt_out, NULL);
        if (err < 0) {
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out][filter]"
                            " failed to build filter graph")
            );
            av_log(
                NULL, AV_LOG_ERROR, "[atfp][img][ff_out][filter] line:%d, error:%d, raw-spec: %s \n",
                __LINE__, err, filter_spec_raw
            );
            goto done;
        } // TODO, valgrind will crash if the function returns with error, figure out why
        err = avfilter_graph_config(_img_enc_ctx->filter_graph, NULL);
        if (err < 0)
            json_object_set_new(
                err_info, "transcoder",
                json_string("[img][ff_out]"
                            "[filter] failed to configure filter graph")
            );
    done:
        if (filt_in)
            avfilter_inout_free(&filt_in);
        if (filt_out)
            avfilter_inout_free(&filt_out);
    }
    if (json_object_size(err_info) > 0)
        json_object_set_new(err_info, "err_code", json_integer(err));
} // end of atfp__image_dst__avfilt_init

int atfp__image_dst__filter_frame(atfp_av_ctx_t *src, atfp_av_ctx_t *dst) {
    int ret = AVERROR(EINVAL);
    if (src && dst) {
        AVPacket *pkt_ori = &src->intermediate_data.decode.packet;
        if (pkt_ori->pos >= 0) {
            AVFrame *frame_ori = &src->intermediate_data.decode.frame;
            AVFrame *frame_filt = &dst->intermediate_data.encode.frame;
            // always save filtered frame to stream 0
            ret = atfp_common__ffm_filter_processing(dst, frame_ori, 0);
            assert(ret != AVERROR(EINVAL));
            if (ret == ATFP_AVCTX_RET__OK)
                frame_filt->pict_type = frame_ori->pict_type;
        } else { // invalid input packet
            ret = ATFP_AVCTX_RET__NEED_MORE_DATA;
        }
    }
    return ret;
} // end of  atfp__image_dst__filter_frame

int atfp__image_dst__flushing_filter(atfp_av_ctx_t *src, atfp_av_ctx_t *dst) {
    (void)src;
    int ret = ATFP_AVCTX_RET__OK;
    if (!dst->intermediate_data.encode._final.filt_flush_done) {
        ret = atfp_common__ffm_filter_processing(dst, NULL, 0);
        if (ret == ATFP_AVCTX_RET__NEED_MORE_DATA)
            dst->intermediate_data.encode._final.filt_flush_done = 1;
    }
    return ret;
}

int atfp__image_dst__has_done_flush_filter(atfp_av_ctx_t *_avctx) {
    return _avctx->intermediate_data.encode._final.filt_flush_done;
}
