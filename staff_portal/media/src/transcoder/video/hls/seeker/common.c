#include "storage/cfg_parser.h"
#include "transcoder/video/hls.h"

#define  DEINIT_IF_EXISTS(var, fn_name) \
    if(var) { \
        fn_name((void *)var); \
        (var) = NULL; \
    }

static void _atfp_hls__deinit_asasrc_final (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    if(asa_src->op.scandir.fileinfo.data) {
        for(int idx = 0; idx < asa_src->op.scandir.fileinfo.size; idx++) {
            asa_dirent_t  *e = &asa_src->op.scandir.fileinfo.data[idx];
            DEINIT_IF_EXISTS(e->name, free);
        }
    }
    DEINIT_IF_EXISTS(asa_src->op.scandir.fileinfo.data, free);
    DEINIT_IF_EXISTS(asa_src->op.scandir.path, free);
    DEINIT_IF_EXISTS(asa_src->op.open.dst_path, free);
    free(asa_src);
}

static void _atfp_hls__stream_seeker_asasrc_deinit (asa_op_base_cfg_t *asa_src)
{
    asa_src->op.close.cb = _atfp_hls__deinit_asasrc_final;
    ASA_RES_CODE result  = asa_src->storage->ops.fn_close(asa_src);
    if(result != ASTORAGE_RESULT_ACCEPT)
        _atfp_hls__deinit_asasrc_final(asa_src, result);
}

static void _atfp_hls__stream_seeker_asalocal_deinit (asa_op_base_cfg_t *_asa_local)
{
    atfp_t *processor = (atfp_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    DEINIT_IF_EXISTS(processor->transfer.streaming_dst.block.data, free);
    DEINIT_IF_EXISTS(processor, free);
}


void  atfp_hls_stream_seeker__init_common (atfp_hls_t *hlsproc, ASA_RES_CODE (*usr_asa_cmd)(asa_op_base_cfg_t *, atfp_t *))
{
    atfp_t *processor = & hlsproc->super;
    json_t *err_info  =  processor->data.error;
    json_t *spec  =  processor->data.spec;
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    uint32_t  _usr_id = processor->data.usr_id;
    uint32_t  _upld_req_id = processor->data.upld_req_id;
    if(!usr_asa_cmd || !err_info || !spec || _usr_id==0 || _upld_req_id==0) {
        fprintf(stderr, "[hls][stream_seeker][common] line:%d, missing argument from caller \r\n",  __LINE__ );
        goto error;
    } else if(asa_src) {
        fprintf(stderr, "[hls][stream_seeker][common] line:%d, asa_src field reserved for internal use. \r\n",  __LINE__ );
        goto error;
    }
    const char *storage_alias = json_string_value(json_object_get(spec, "storage_alias"));
    size_t  _rdbuf_max_sz = json_integer_value(json_object_get(spec, "buf_max_sz")); // TODO, rename to  rdbuf_max_sz
    size_t  _num_usrargs_asa = json_integer_value(json_object_get(spec, "num_usrargs_asa_src"));
    if(!storage_alias || _rdbuf_max_sz==0 || _num_usrargs_asa==0) {
        fprintf(stderr, "[hls][stream_seeker][common] line:%d, missing argument in spec \r\n",  __LINE__ );
        goto error;
    }
    asa_cfg_t *storage = app_storage_cfg_lookup(storage_alias);
    asa_src =  app_storage__init_asaobj_helper (storage, _num_usrargs_asa, _rdbuf_max_sz, 0);
    if(!asa_src) {
        fprintf(stderr, "[hls][stream_seeker][common] line:%d, missing argument \r\n",  __LINE__ );
        goto error;
    }
    void *loop = (void *) json_integer_value(json_object_get(spec, "loop"));
    if(!strcmp(storage_alias, "localfs")) // TODO
        ((asa_op_localfs_cfg_t *)asa_src)->loop = loop;
    hlsproc->asa_local.loop = loop;
    asa_src->deinit = _atfp_hls__stream_seeker_asasrc_deinit;
    hlsproc->asa_local.super.deinit = _atfp_hls__stream_seeker_asalocal_deinit;
    ASA_RES_CODE result = usr_asa_cmd(asa_src, processor);
    if(result != ASTORAGE_RESULT_ACCEPT) {
        fprintf(stderr, "[hls][stream_seeker][common] line:%d, failed to perform storage operation \r\n",  __LINE__ );
        goto error;
    }
    asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG] = processor;
    processor->data.storage.handle = asa_src;
    return;
error:
    json_object_set_new(err_info, "transcoder", json_string("[hls] internal error"));
    if(asa_src)
        asa_src->deinit(asa_src);
} // end of  atfp_hls_stream_seeker__init_common
