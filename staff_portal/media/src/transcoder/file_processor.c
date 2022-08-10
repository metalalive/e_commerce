#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>
#include "transcoder/file_processor.h"

extern atfp_ops_entry_t  atfp_ops_video_mp4;
extern atfp_ops_entry_t  atfp_ops_video_hls;

static  const atfp_ops_entry_t * _atfp_ops_table[] = {
    &atfp_ops_video_mp4,
    &atfp_ops_video_hls,
    NULL,
}; // end of _atfp_ops_table


static const atfp_ops_entry_t * atfp_file_processor_lookup(const char *label)
{
    const atfp_ops_entry_t *found = NULL;
    uint32_t idx = 0;
    for(idx = 0; !found && _atfp_ops_table[idx]; idx++) {
        const atfp_ops_entry_t *item  = _atfp_ops_table[idx];
        if(!item->ops.label_match)
            continue;
        if(item->ops.label_match(label))
            found = item;
    }
    return found;
} // end of atfp_file_processor_lookup


atfp_t *app_transcoder_file_processor(const char *label)
{
    atfp_t *out = NULL;
    const atfp_ops_entry_t *op_entry = atfp_file_processor_lookup(label);
    const atfp_ops_t  *ops = NULL;
    if(op_entry)
        ops = &op_entry->ops;
    if(ops)
        out = ops->instantiate();
    if(out) {
        out->ops = ops;
        out->backend_id = op_entry->backend_id;
    }
    return out;
} // end of app_transcoder_file_processor


uint8_t  atfp_common__label_match(const char *label, size_t num, const char **exp_labels)
{
    uint8_t  matched = 0;
    for(int idx = 0; !matched && idx < num; idx++) {
        int ret = strncmp(label, exp_labels[idx], strlen(exp_labels[idx]));
        matched = ret == 0;
    }
    return matched;
}


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


static void  atfp__close_curr_srcfchunk_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // only for source filechunk
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    uint8_t err = result != ASTORAGE_RESULT_COMPLETE;
    if(!err) {
        asa_cfg_t  *storage = processor->data.storage.config;
        int next_chunk_seq = (int) processor->filechunk_seq.next + 1;
        result = atfp_open_srcfile_chunk(asaobj, storage, processor->data.storage.basepath,
                     next_chunk_seq, asaobj->op.open.cb);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if(err) {
        processor->filechunk_seq.usr_cb(asaobj, result);
    }
}

static void  atfp__open_next_srcfchunk_cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result)
{ // only for source filechunk
    atfp_t *processor = (atfp_t *)asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if(result == ASTORAGE_RESULT_COMPLETE) {
        processor->filechunk_seq.curr = processor->filechunk_seq.next;
        processor->filechunk_seq.eof_reached = 0;
    }
    processor->filechunk_seq.usr_cb(asaobj, result);
}

ASA_RES_CODE  atfp_switch_to_srcfile_chunk(atfp_t *processor, int chunk_seq, asa_open_cb_t cb)
{ // close current filechunk then optionally open the next one if exists.
    ASA_RES_CODE result;
    json_t *filechunks_size = json_object_get(processor->data.spec, "parts_size");
    uint32_t  final_filechunk_id  = json_array_size(filechunks_size) - 1;
    uint32_t  next_filechunk_id   = (chunk_seq < 0) ? (processor->filechunk_seq.curr + 1): chunk_seq;
    if(final_filechunk_id >= next_filechunk_id) {
        asa_op_base_cfg_t *cfg = processor->data.storage.handle;
        asa_cfg_t     *storage = processor->data.storage.config;
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


atfp_asa_map_t  *atfp_asa_map_init(uint8_t num_dst)
{
    size_t  tot_sz = sizeof(atfp_asa_map_t) + sizeof(_asamap_dst_entry_t) * num_dst;
    atfp_asa_map_t *obj = calloc(1, tot_sz);
    char *ptr = (char *)obj + sizeof(atfp_asa_map_t);
    obj->dst.entries = (_asamap_dst_entry_t *) ptr;
    obj->dst.capacity = num_dst;
    return obj;
}

void  atfp_asa_map_deinit(atfp_asa_map_t *obj)
{
   if(obj)
       free(obj);
}

#define  ASAMAP_SET_OBJ_COMMON_CODE(map, asaobj_new0, field_p) \
{ \
    asa_op_base_cfg_t  *asaobj_old = NULL; \
    asa_op_base_cfg_t  *asaobj_new = asaobj_new0; \
    asa_op_base_cfg_t **field = field_p; \
    if(map) { \
        asaobj_old = *field; \
        *field = asaobj_new; \
    } \
    if(asaobj_new && asaobj_new->cb_args.entries) \
        asaobj_new ->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = (void *)map; \
    if(asaobj_old && asaobj_old->cb_args.entries) \
        asaobj_old ->cb_args.entries[ASAMAP_INDEX__IN_ASA_USRARG] = NULL; \
}

void  atfp_asa_map_set_source(atfp_asa_map_t *map, asa_op_base_cfg_t *asaobj)
{
    ASAMAP_SET_OBJ_COMMON_CODE(map, asaobj, &map->src);
}

void  atfp_asa_map_set_localtmp(atfp_asa_map_t *map, asa_op_localfs_cfg_t *asaobj)
{
    ASAMAP_SET_OBJ_COMMON_CODE(map, &asaobj->super, (asa_op_base_cfg_t **)&map->local_tmp);
}

uint8_t  atfp_asa_map_add_destination(atfp_asa_map_t *map, asa_op_base_cfg_t *asaobj)
{
    if(!map || !asaobj)
        goto error;
    size_t num_used = map->dst.size;
    if(num_used >= map->dst.capacity)
        goto error;
    _asamap_dst_entry_t  *entry = &map->dst.entries[num_used++];
    map->dst.size = num_used;
    ASAMAP_SET_OBJ_COMMON_CODE(map, asaobj, &entry->handle);
    return 0;
error:
    return 1;
}

uint8_t  atfp_asa_map_remove_destination(atfp_asa_map_t *map, asa_op_base_cfg_t *asaobj)
{
    uint8_t err = 0;
    if(!map || !asaobj) {
        err = 1;
        goto done;
    }
    _asamap_dst_entry_t   empty_entry = {0};
    _asamap_dst_entry_t  *curr_entry = NULL, *found = NULL;
    size_t num_used = map->dst.size;
    for(int idx = 0; idx < num_used; idx++) {
        curr_entry = &map->dst.entries[idx];
        if(found) { // idx > 0 always holds true
            map->dst.entries[idx - 1] = curr_entry ? *curr_entry: empty_entry;
        } else if(curr_entry && curr_entry->handle == asaobj) {
            found = curr_entry;
            map->dst.entries[idx] = empty_entry;
        }
    }
    if(found) {
        map->dst.entries[num_used - 1] = empty_entry;
        map->dst.size--;
    }
    err = found == NULL;
done:
    return err;
} // end of atfp_asa_map_remove_destination


asa_op_localfs_cfg_t  *atfp_asa_map_get_localtmp(atfp_asa_map_t *map)
{ return map->local_tmp; }

asa_op_base_cfg_t  *atfp_asa_map_get_source(atfp_asa_map_t *map)
{ return map->src; }

asa_op_base_cfg_t  *atfp_asa_map_iterate_destination(atfp_asa_map_t *map)
{
    _asamap_dst_entry_t  *entry = NULL;
    uint8_t  iter_idx = map->dst.iter_idx;
    if(map->dst.size > iter_idx) {
        entry = &map->dst.entries[iter_idx++];
        map->dst.iter_idx = iter_idx;
    }
    return  entry ? entry->handle: NULL;
}

void     atfp_asa_map_reset_dst_iteration(atfp_asa_map_t *map)
{ map->dst.iter_idx = 0; }

#define  ASAMAP_DST_WORKING__COMMON_CODE(map, asaobj, bitval) \
{ \
    _asamap_dst_entry_t  *entry = NULL; \
    for(int idx = 0; idx < map->dst.size; idx++) { \
        entry = &map->dst.entries[idx]; \
        if(entry && entry->handle == asaobj) { \
            entry->flags.working = bitval; \
            break; \
        } else { \
            entry = NULL; \
        } \
    } \
    return entry != NULL; \
}

uint8_t  atfp_asa_map_dst_start_working(atfp_asa_map_t *map, asa_op_base_cfg_t *asaobj)
{
    ASAMAP_DST_WORKING__COMMON_CODE(map, asaobj, 1);
}

uint8_t  atfp_asa_map_dst_stop_working(atfp_asa_map_t *map, asa_op_base_cfg_t *asaobj)
{
    ASAMAP_DST_WORKING__COMMON_CODE(map, asaobj, 0);
}

uint8_t  atfp_asa_map_all_dst_stopped(atfp_asa_map_t *map)
{
    _asamap_dst_entry_t  *entry = NULL;
    uint8_t any_dst_working = 0;
    for(int idx = 0; idx < map->dst.size; idx++) {
        entry = &map->dst.entries[idx];
        any_dst_working |= entry->flags.working;
    }
    return !any_dst_working;
}