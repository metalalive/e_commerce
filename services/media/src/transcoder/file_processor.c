#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>

#include "datatypes.h"
#include "transcoder/file_processor.h"

extern atfp_ops_entry_t * _atfp_ops_table [];

static const atfp_ops_entry_t * atfp_file_processor_lookup(const char *label)
{
    const atfp_ops_entry_t *found = NULL;
    uint32_t idx = 0;
    if(label) {
        for(idx = 0; !found && _atfp_ops_table[idx]; idx++) {
            const atfp_ops_entry_t *item  = _atfp_ops_table[idx];
            if(!item->ops.label_match)
                continue;
            if(item->ops.label_match(label))
                found = item;
        }
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


int  atfp_scandir_load_fileinfo (asa_op_base_cfg_t *asaobj, json_t *err_info)
{
    int idx = 0, err = 0;
    ASA_RES_CODE result ;
    size_t num_files = asaobj->op.scandir.fileinfo.size;
    asa_dirent_t *es = calloc(num_files, sizeof(asa_dirent_t));
    for(idx = 0; (!err) && (idx < num_files); idx++) {
        result =  asaobj->storage->ops.fn_scandir_next(asaobj, &es[idx]);
        if(result == ASTORAGE_RESULT_COMPLETE) { //  && es[idx].type == ASA_DIRENT_FILE
            es[idx].name = strdup(es[idx].name); // TODO, optimize with linked list of 1kb mem block
        } else {
            json_object_set_new(err_info, "transcode", json_string(
                "[storage] failed to retrieve next entry in scandir result"));
            err = 1;
        }
    } // end of loop
    if(!err) {
        asa_dirent_t  e = {0};
        result =  asaobj->storage->ops.fn_scandir_next(asaobj, &e);
        if(result != ASTORAGE_RESULT_EOF_SCAN) {
            json_object_set_new(err_info, "transcode", json_string(
                "[storage] unexpected entry found in scandir result"));
            err = 1;
        }
    }
    if(err) {
        for(idx = 0; idx < num_files; idx++) {
            if(es[idx].name)
                free(es[idx].name);
        }
        free(es);
    } else {
        asaobj->op.scandir.fileinfo.data = es;
    }
    return err;
} // end of atfp_scandir_load_fileinfo


int  atfp_check_fileupdate_required(atfp_data_t *data, const char *basepath,
        const char *filename, float threshold_secs)
{ // check if a file at local app server should be refreshed since its latest update
#define  ASA_SRC_BASEPATH_PATTERN  "%s/%d/%08x/%s"
    int  required = 1;
    size_t filepath_sz = sizeof(ASA_SRC_BASEPATH_PATTERN) + strlen(basepath) +
        USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(data->upld_req_id) + strlen(filename) + 1;
    char filepath[filepath_sz];
    size_t nwrite = snprintf(&filepath[0], filepath_sz, ASA_SRC_BASEPATH_PATTERN,
            basepath, data->usr_id, data->upld_req_id, filename);
    assert(filepath_sz >= nwrite);
    struct stat  statbuf = {0};
    int ret = stat(&filepath[0], &statbuf); // TODO, invoke stat() asynchronously
    if(!ret) {
        time_t  last_update = statbuf.st_mtime;
        time_t  curr_tm = time(NULL);
        double  num_seconds = difftime(curr_tm, last_update);
        required = (threshold_secs < num_seconds) || (statbuf.st_size == 0);
    } // otherwise, file not found, update required
    return required;
#undef  ASA_SRC_BASEPATH_PATTERN
} // end of atfp_check_fileupdate_required


#define  ATFP_IMG_MSK_IDX_FILENAME  "index.json"
#define  FILEPATH_PATTERN           "%s/%s"
json_t * atfp_image_mask_pattern_index (const char *_basepath)
{
    size_t filepath_sz = sizeof(ATFP_IMG_MSK_IDX_FILENAME) + sizeof(FILEPATH_PATTERN)
               + strlen(_basepath);
    char filepath[filepath_sz];
    size_t nwrite = snprintf(&filepath[0], filepath_sz, FILEPATH_PATTERN,
            _basepath, ATFP_IMG_MSK_IDX_FILENAME);
    assert(nwrite < filepath_sz);
    // NOTE: currently there are only few mask patterns in use, so the file names are
    //  recorded in plain text file, if it grows larger then move these records to database.
    return json_load_file(&filepath[0], 0, NULL);
}
#undef   FILEPATH_PATTERN
#undef   ATFP_IMG_MSK_IDX_FILENAME
