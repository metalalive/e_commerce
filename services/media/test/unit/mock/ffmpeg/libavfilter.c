#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

AVFilterInOut *avfilter_inout_alloc(void)
{ return (AVFilterInOut *) mock(); }

void avfilter_inout_free(AVFilterInOut **inout_p)
{
    if(inout_p) {
        AVFilterInOut *inout = *inout_p;
        mock(inout);
        *inout_p = NULL;
    }
}

AVFilterGraph *avfilter_graph_alloc(void)
{ return (AVFilterGraph *) mock(); }

void avfilter_graph_free(AVFilterGraph **graph_p)
{
    AVFilterGraph *graph = *graph_p;
    mock(graph);
    *graph_p = NULL;
}

int avfilter_graph_parse_ptr(AVFilterGraph *graph, const char *filters,
        AVFilterInOut **inputs_p, AVFilterInOut **outputs_p, void *log_ctx)
{
    AVFilterInOut *inputs  = *inputs_p;
    AVFilterInOut *outputs = *outputs_p;
    return (int) mock(graph, filters, inputs, outputs);
}

int avfilter_graph_config(AVFilterGraph *graph, void *log_ctx)
{ return (int) mock(graph); }

const AVFilter *avfilter_get_by_name(const char *name)
{ return (const AVFilter *) mock(name); }

int avfilter_graph_create_filter (AVFilterContext **filt_ctx_p, const AVFilter *filt,
         const char *name, const char *args, void *opaque, AVFilterGraph *graph_ctx)
{ return (int) mock(filt_ctx_p, filt, name, args, graph_ctx); }

void avfilter_free(AVFilterContext *filt)
{ mock(filt); }

int av_buffersrc_add_frame_flags(AVFilterContext *filt_ctx, AVFrame *frm, int flags)
{ return (int) mock(filt_ctx, frm); }

int av_buffersink_get_frame(AVFilterContext *filt_ctx, AVFrame *frm)
{ return (int) mock(filt_ctx, frm); }

