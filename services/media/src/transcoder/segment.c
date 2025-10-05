#include <string.h>
#include <sys/stat.h>

#include "utils.h"
#include "transcoder/file_processor.h"

int atfp_segment_init(atfp_segment_t *seg_cfg) {
    seg_cfg->transfer.eof_reached = 0;
    seg_cfg->transfer.nbytes = 0;
    int ret = SHA1_Init(&seg_cfg->checksum); // 0 means error
    return ret == 0;
}

int atfp_segment_final(atfp_segment_t *seg_cfg, json_t *info) {
#define MD_HEX_SZ ((SHA_DIGEST_LENGTH << 1) + 1) // 20 * 2 + NULL bytes
    const char *filename = (const char *)strrchr(seg_cfg->fullpath._asa_dst.data, (int)'/');
    if (!filename) {
        return 1; // invalid key
    } else if (json_object_get(info, filename)) {
        return 2; // duplicate not allowed
    } else {
        filename += 1; // skip slash char
    }
    unsigned char md[SHA_DIGEST_LENGTH] = {0}, md_hex[MD_HEX_SZ] = {0};
    SHA1_Final(&md[0], &seg_cfg->checksum);
    app_chararray_to_hexstr((char *)&md_hex[0], (size_t)(MD_HEX_SZ - 1), (char *)&md[0], SHA_DIGEST_LENGTH);
    md_hex[MD_HEX_SZ - 1] = 0x0;
#undef MD_HEX_SZ
    json_t *item = json_object();
    json_object_set_new(item, "size", json_integer(seg_cfg->transfer.nbytes));
    json_object_set_new(item, "checksum", json_string((char *)&md_hex[0]));
    json_object_set_new(info, filename, item);
    OPENSSL_cleanse(&seg_cfg->checksum, sizeof(seg_cfg->checksum));
    return 0;
} // end of atfp_segment_final

static int atfp__format_file_fullpath(char *out, size_t out_sz, const char *basepath, const char *filename) {
    size_t basepath_sz = strlen(basepath);
    size_t filename_sz = strlen(filename);
    size_t sz_required = basepath_sz + filename_sz + 1;
    int    ret = 1; // error, insufficient memory space
    if (sz_required < out_sz) {
        memset(out, 0x0, sizeof(char) * out_sz);
        strncat(out, basepath, basepath_sz);
        strncat(out, "/", 1);
        strncat(out, filename, filename_sz);
        assert(out[out_sz - 1] == 0x0);
        ret = 0; // ok
    }
    return ret;
} // end of atfp__format_file_fullpath

static int atfp__format_segment_fullpath(
    char *out, size_t out_sz, const char *basepath, atfp_segment_t *seg_cfg, int chosen_idx
) {
    if (seg_cfg->rdy_list.size <= chosen_idx)
        return 1;
    if (seg_cfg->filename.pattern.max_num_digits == 0)
        return 2;
    size_t  basepath_sz = strlen(basepath);
    uint8_t max_num_digits = seg_cfg->filename.pattern.max_num_digits;
    size_t  sz_required = basepath_sz + seg_cfg->filename.prefix.sz + 1 + max_num_digits;
    int     ret = 0;
    if (sz_required < out_sz) {
        memset(out, 0x0, sizeof(char) * out_sz);
        strncat(out, basepath, basepath_sz);
        strncat(out, "/", 1);
        strncat(out, seg_cfg->filename.prefix.data, seg_cfg->filename.prefix.sz);
        int     seg_num = seg_cfg->rdy_list.entries[chosen_idx];
        uint8_t seg_num_str_sz = max_num_digits + 1;
        char    seg_num_str[seg_num_str_sz];
        size_t  nwrite = snprintf(&seg_num_str[0], seg_num_str_sz, seg_cfg->filename.pattern.data, seg_num);
        assert(nwrite == max_num_digits);
        seg_num_str[nwrite] = 0x0;
        strncat(out, &seg_num_str[0], max_num_digits);
        assert(out[out_sz - 1] == 0x0);
    } else {
        ret = 3; // insufficient memory space
        fprintf(
            stderr, "[transcoder][segment] line:%d, sz_required:%ld, out_sz:%ld \n", __LINE__, sz_required,
            out_sz
        );
    }
    return ret;
} // end of atfp__format_segment_fullpath

static void _atfp__transfer_basic_setup(
    asa_op_base_cfg_t *asa_dst, asa_op_localfs_cfg_t *asa_local, atfp_segment_t *seg_cfg
) {
    asa_dst->op.open.mode = S_IRUSR | S_IWUSR;
    asa_dst->op.open.flags = O_WRONLY | O_CREAT | O_EXCL;
    asa_dst->op.write.offset = -1;
    asa_dst->op.open.dst_path = seg_cfg->fullpath._asa_dst.data;
    asa_local->super.op.open.mode = S_IRUSR;
    asa_local->super.op.open.flags = O_RDONLY;
    asa_local->super.op.read.offset = -1;
    asa_local->super.op.open.dst_path = seg_cfg->fullpath._asa_local.data;
    // shares the same buffer
    asa_local->super.op.read.dst = asa_dst->op.write.src;
    size_t srcbuf_max_nbytes = asa_dst->op.write.src_max_nbytes;
    asa_local->super.op.read.dst_max_nbytes = srcbuf_max_nbytes;
    asa_local->super.op.read.dst_sz = srcbuf_max_nbytes;
}

#define ASA_DST__COPY_PATH_PREFIX(_seg_cfg, __asa_dst) \
    char  *_segf_fullpath_asa_dst_ptr = _seg_cfg->fullpath._asa_dst.data; \
    size_t _segf_fullpath_asa_dst_sz = _seg_cfg->fullpath._asa_dst.sz; \
    if (__asa_dst->op.mkdir.path.prefix) { \
        size_t asa_dst_path_prefix_sz = strlen(__asa_dst->op.mkdir.path.prefix); \
        if (asa_dst_path_prefix_sz > 0) { \
            assert(_segf_fullpath_asa_dst_sz > (asa_dst_path_prefix_sz + 1)); \
            memset(_segf_fullpath_asa_dst_ptr, 0x0, sizeof(char) * _segf_fullpath_asa_dst_sz); \
            strncat(_segf_fullpath_asa_dst_ptr, __asa_dst->op.mkdir.path.prefix, asa_dst_path_prefix_sz); \
            strncat(_segf_fullpath_asa_dst_ptr, "/", 1); \
            _segf_fullpath_asa_dst_ptr += asa_dst_path_prefix_sz + 1; \
            _segf_fullpath_asa_dst_sz -= asa_dst_path_prefix_sz + 1; \
        } \
    }

ASA_RES_CODE atfp__segment_start_transfer(
    asa_op_base_cfg_t *asa_dst, asa_op_localfs_cfg_t *asa_local, atfp_segment_t *seg_cfg, int chosen_idx
) {
    ASA_RES_CODE result = ASTORAGE_RESULT_DATA_ERROR;
    if (!asa_dst || !asa_local || !seg_cfg || !asa_dst->op.write.src ||
        asa_dst->op.write.src_max_nbytes == 0 || !asa_dst->op.open.cb || !asa_local->super.op.open.cb) {
        goto done;
    }
    ASA_DST__COPY_PATH_PREFIX(seg_cfg, asa_dst)
    int ret1 = atfp__format_segment_fullpath(
        _segf_fullpath_asa_dst_ptr, _segf_fullpath_asa_dst_sz, asa_dst->op.mkdir.path.origin, seg_cfg,
        chosen_idx
    );
    int ret2 = atfp__format_segment_fullpath(
        seg_cfg->fullpath._asa_local.data, seg_cfg->fullpath._asa_local.sz,
        asa_local->super.op.mkdir.path.origin, seg_cfg, chosen_idx
    );
    if (!ret1 && !ret2) {
        _atfp__transfer_basic_setup(asa_dst, asa_local, seg_cfg);
        seg_cfg->transfer.curr_idx = chosen_idx;
        result = asa_local->super.storage->ops.fn_open(&asa_local->super);
    } else if (ret1 == 1) {
        result = ASTORAGE_RESULT_COMPLETE; // do nothing
    }
done:
    return result;
} // end of atfp__segment_start_transfer

ASA_RES_CODE atfp__file_start_transfer(
    asa_op_base_cfg_t *asa_dst, asa_op_localfs_cfg_t *asa_local, atfp_segment_t *seg_cfg,
    const char *filename_local, const char *filename_dst
) {
    ASA_RES_CODE result = ASTORAGE_RESULT_DATA_ERROR;
    if (!asa_dst || !asa_local || !seg_cfg || !asa_dst->op.write.src || !filename_local || !filename_dst) {
        goto done;
    } else if (!asa_dst->op.open.cb || !asa_local->super.op.open.cb) {
        goto done;
    }
    ASA_DST__COPY_PATH_PREFIX(seg_cfg, asa_dst)
    int ret1 = atfp__format_file_fullpath(
        _segf_fullpath_asa_dst_ptr, _segf_fullpath_asa_dst_sz, asa_dst->op.mkdir.path.origin, filename_dst
    );
    int ret2 = atfp__format_file_fullpath(
        seg_cfg->fullpath._asa_local.data, seg_cfg->fullpath._asa_local.sz,
        asa_local->super.op.mkdir.path.origin, filename_local
    );
    if (!ret1 && !ret2) {
        _atfp__transfer_basic_setup(asa_dst, asa_local, seg_cfg);
        result = asa_local->super.storage->ops.fn_open(&asa_local->super);
    }
done:
    return result;
} // end of atfp__file_start_transfer

// -----------------------------------------------------------
// common callbacks used when transferring segemnt files from
// local transcoding server to remote destination storage
// -----------------------------------------------------------
// TODO, rename prefix of the common callbacks below --> `atfp_seg2dst__xxx`
void atfp__close_local_seg__cb(asa_op_base_cfg_t *asaobj, atfp_segment_t *seg_cfg, ASA_RES_CODE result) {
    int     err = 1;
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    processor->transfer.transcoded_dst.flags.asalocal_open = 0;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_final(seg_cfg, processor->transfer.transcoded_dst.info);
        processor->transfer.transcoded_dst.tot_nbytes_file += seg_cfg->transfer.nbytes;
        asa_op_base_cfg_t *asa_local = asaobj;
        asa_local->op.unlink.path = asa_local->op.open.dst_path;
        result = asa_local->storage->ops.fn_unlink(asa_local);
        err = result != ASTORAGE_RESULT_ACCEPT;
    } // TODO , flush transcoded detail info to local metadata file
    if (err) {
        json_object_set_new(
            processor->data.error, "storage",
            json_string("failed to close transferred segment file on local side")
        );
        processor->data.callback(processor);
    }
} // end of atfp__close_local_seg__cb

void atfp__unlink_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    int     err = 1;
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
        result = asa_dst->storage->ops.fn_close(asa_dst);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if (err) {
        json_object_set_new(
            processor->data.error, "storage",
            json_string("failed to unlink transferred segment file on local side")
        );
        processor->data.callback(processor);
    }
} // end of atfp__unlink_local_seg__cb

void atfp__open_local_seg__cb(asa_op_base_cfg_t *asaobj, ASA_RES_CODE result) {
    int     err = 1;
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        processor->transfer.transcoded_dst.flags.asalocal_open = 1;
        asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
        result = asa_dst->storage->ops.fn_open(asa_dst);
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if (err) {
        json_object_set_new(
            processor->data.error, "storage", json_string("failed to open local segment file for transfer")
        );
        processor->data.callback(processor);
    }
} // end of atfp__open_local_seg__cb

void atfp__open_dst_seg__cb(asa_op_base_cfg_t *asaobj, atfp_segment_t *seg_cfg, ASA_RES_CODE result) {
    atfp_t            *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    asa_op_base_cfg_t *_asa_local = asaobj;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        atfp_segment_init(seg_cfg);
        processor->transfer.transcoded_dst.flags.asaremote_open = 1;
#if 1
        result = _asa_local->storage->ops.fn_read(_asa_local);
#else
        result = ASTORAGE_RESULT_DATA_ERROR;
#endif
        if (result != ASTORAGE_RESULT_ACCEPT) {
            json_object_set_new(
                processor->data.error, "storage", json_string("failed to send read op in local segment file")
            );
            processor->data.callback(processor);
        }
    } else {
        json_object_set_new(
            processor->data.error, "storage",
            json_string("failed to open segment file on destination side for transfer")
        );
        fprintf(
            stderr, "[transcoder][segment] line:%d, job_id:%s, result:%d \n", __LINE__,
            processor->data.rpc_receipt->job_id.bytes, result
        );
        processor->data.callback(processor);
    }
} // end of atfp__open_dst_seg__cb

void atfp__read_local_seg__cb(
    asa_op_base_cfg_t *asaobj, atfp_segment_t *seg_cfg, ASA_RES_CODE result, size_t nread
) {
    int     err = 1;
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        seg_cfg->transfer.eof_reached = asaobj->op.read.dst_sz > nread;
        if (nread == 0) {
            asa_op_base_cfg_t *_asa_local = asaobj;
            result = _asa_local->storage->ops.fn_close(_asa_local);
        } else {
            SHA1_Update(&seg_cfg->checksum, asaobj->op.read.dst, nread);
            asa_op_base_cfg_t *asa_dst = processor->data.storage.handle;
            asa_dst->op.write.src_sz = nread;
            result = asa_dst->storage->ops.fn_write(asa_dst);
        }
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if (err) {
        json_object_set_new(
            processor->data.error, "storage",
            json_string("failed to read from local segment file for transfer")
        );
        processor->data.callback(processor);
    }
} // end of atfp__read_local_seg__cb

void atfp__write_dst_seg__cb(
    asa_op_base_cfg_t *asaobj, atfp_segment_t *seg_cfg, ASA_RES_CODE result, size_t nwrite
) {
    int     err = 1;
    atfp_t *processor = asaobj->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t *_asa_local = asaobj;
        seg_cfg->transfer.nbytes += nwrite;
        if (seg_cfg->transfer.eof_reached) { // switch to next segment (if exists)
            result = _asa_local->storage->ops.fn_close(_asa_local);
        } else {
            result = _asa_local->storage->ops.fn_read(_asa_local);
        }
        err = result != ASTORAGE_RESULT_ACCEPT;
    }
    if (err) {
        json_object_set_new(
            processor->data.error, "storage", json_string("failed to transfer to destination segment file")
        );
        processor->data.callback(processor);
    }
} // end of atfp__write_dst_seg__cb
