#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>
#include "transcoder/file_processor.h"

extern atfp_ops_entry_t  atfp_ops_video_mp4;

static  const atfp_ops_entry_t * _atfp_ops_table[] = {
    &atfp_ops_video_mp4,
    NULL,
}; // end of _atfp_ops_table


static const atfp_ops_t * atfp_file_processor_lookup(const char *mimetype)
{
    const atfp_ops_t *found = NULL;
    uint32_t idx = 0;
    for(idx = 0; !found && _atfp_ops_table[idx]; idx++) {
        const atfp_ops_entry_t *item  = _atfp_ops_table[idx];
        int ret = strncmp(mimetype, item->mimetype, strlen(item->mimetype));
        if(ret == 0)
            found = &item->ops;
    }
    return found;
} // end of atfp_file_processor_lookup


atfp_t *app_transcoder_file_processor(const char *mimetype)
{
    atfp_t *out = NULL;
    const atfp_ops_t *ops = atfp_file_processor_lookup(mimetype);
    if(ops) {
        out = calloc(0x1, ops->get_obj_size());
        out->ops = ops;
    }
    return out;
} // end of app_transcoder_file_processor

ASA_RES_CODE  atfp_open_srcfile_chunk(
        asa_op_base_cfg_t *cfg,
        asa_cfg_t  *storage,
        const char *basepath,
        int        chunk_seq,
        asa_open_cb_t  cb )
{
#define  MAX_INT32_DIGITS  10
    ASA_RES_CODE result = ASTORAGE_RESULT_ACCEPT;
    { // update file path for each media segment, open the first file chunk
        size_t filepath_sz = strlen(basepath) + 1 + MAX_INT32_DIGITS + 1; // assume NULL-terminated string
        char filepath[filepath_sz];
        size_t nwrite = snprintf(&filepath[0], filepath_sz, "%s/%d", basepath, chunk_seq);
        filepath[nwrite++] = 0x0;
        if(cfg->op.open.dst_path) {
            free(cfg->op.open.dst_path);
        }
        cfg->op.open.dst_path = strndup(&filepath[0], nwrite);
    }
    cfg->op.open.cb = cb;
    cfg->op.open.mode  = S_IRUSR;
    cfg->op.open.flags = O_RDONLY;
    result = storage->ops.fn_open(cfg);
    return result;
#undef  MAX_INT32_DIGITS
} // end of  atfp_open_srcfile_chunk


static void  atfp__close_curr_srcfchunk_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{ // only for source filechunk
    atfp_t *processor = (atfp_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    uint8_t err = result != ASTORAGE_RESULT_COMPLETE;
    if(!err) {
        asa_cfg_t  *storage = processor->data.src.storage.config;
        int next_chunk_seq = (int) processor->filechunk_seq.next + 1;
        result = atfp_open_srcfile_chunk(cfg, storage, processor->data.src.basepath,
                     next_chunk_seq, cfg->op.open.cb);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        processor->filechunk_seq.usr_cb(cfg, result);
    }
}

static void  atfp__open_next_srcfchunk_cb(asa_op_base_cfg_t *cfg, ASA_RES_CODE result)
{ // only for source filechunk
    atfp_t *processor = (atfp_t *)cfg->cb_args.entries[ATFP_INDEX_IN_ASA_OP_USRARG];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        processor->filechunk_seq.curr = processor->filechunk_seq.next;
        processor->filechunk_seq.eof_reached = 0;
    }
    processor->filechunk_seq.usr_cb(cfg, result);
}

ASA_RES_CODE  atfp_switch_to_srcfile_chunk(atfp_t *processor, int chunk_seq, asa_open_cb_t cb)
{ // close current filechunk then optionally open the next one if exists.
    ASA_RES_CODE result;
    json_t *filechunks_size = json_object_get(processor->data.spec, "parts_size");
    uint32_t  final_filechunk_id  = json_array_size(filechunks_size) - 1;
    uint32_t  next_filechunk_id   = (chunk_seq < 0) ? (processor->filechunk_seq.curr + 1): chunk_seq;
    if(final_filechunk_id >= next_filechunk_id) {
        asa_op_base_cfg_t *cfg = processor->data.src.storage.handle;
        asa_cfg_t     *storage = processor->data.src.storage.config;
        cfg->op.close.cb = atfp__close_curr_srcfchunk_cb;
        cfg->op.open.cb  = atfp__open_next_srcfchunk_cb;
        processor->filechunk_seq.next = next_filechunk_id;
        processor->filechunk_seq.usr_cb = cb;
        result = storage->ops.fn_close(cfg);
    } else {
        result = ASTORAGE_RESULT_DATA_ERROR;
    }
    return result;
} // end of atfp_switch_to_srcfile_chunk


int  atfp_estimate_src_filechunk_idx(json_t *spec, int chunk_idx_start, size_t *pos)
{
    json_t *fchunks_sz = json_object_get(spec, "parts_size");
    size_t max_num_fchunks  = (size_t) json_array_size(fchunks_sz);
    int    chunk_idx_dst   =  chunk_idx_start;
    size_t fread_offset = *pos;
    for (; chunk_idx_dst < max_num_fchunks; chunk_idx_dst++) {
        size_t chunk_sz = (size_t) json_integer_value(json_array_get(fchunks_sz, chunk_idx_dst));
        if(fread_offset > chunk_sz) {
            fread_offset -= chunk_sz;
        } else {
            break;
        }
    }
    if(chunk_idx_dst < max_num_fchunks) {
        *pos = fread_offset;
    } else { // destination file chunk NOT found
        chunk_idx_dst = -1; 
    }
    return chunk_idx_dst;
} // end of atfp_estimate_src_filechunk_idx

