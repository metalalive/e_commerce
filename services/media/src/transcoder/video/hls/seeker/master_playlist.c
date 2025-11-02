#include "datatypes.h"
#include "transcoder/video/hls.h"
#define NUM_USRARGS_ASA_SRC (ATFP_INDEX__IN_ASA_USRARG + 1)

static ASA_RES_CODE atfp_hls__open_src_mst_plist(atfp_hls_t *);

static void _atfp_hls__close_src_mst_plist_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &hlsproc->super;
    json_t     *err_info = processor->data.error;
    if (processor->transfer.streaming_dst.block.data) {
        processor->data.callback(processor);
    } else { // no valid master playlist has been read, try next one
        result = atfp_hls__open_src_mst_plist(hlsproc);
        if (result != ASTORAGE_RESULT_ACCEPT) {
            if (result != ASTORAGE_RESULT_EOF_SCAN)
                json_object_set_new(err_info, "transcoder", json_string("[hls] internal error"));
            processor->data.callback(processor);
        }
    }
} // end of  _atfp_hls__close_src_mst_plist_cb

static void _atfp_hls__read_src_mst_plist_cb(
    asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread
) { // read ext-x-stream-inf tag, then write it to collected master playlist
    atfp_hls_t *hlsproc = (atfp_hls_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &hlsproc->super;
    // NOTE, this application assumes the read buffer is sufficient to read whole beginning
    // part of ext-x tags, so there is only one read operation to the source playlist
    size_t _eof_reached = nread <= asa_src->op.read.dst_sz;
    if (!_eof_reached) {
        fprintf(stderr, "[hls][mst_plist] line:%d, insufficient read buffer \r\n", __LINE__);
        goto done;
    }
    processor->transfer.streaming_dst.flags.eof_reached = _eof_reached;
    asa_src->op.read.dst[nread] = 0x0;
    int entry_idx = asa_src->op.scandir.fileinfo.rd_idx - 1;
    if (result != ASTORAGE_RESULT_COMPLETE)
        goto done;
    char *stream_inf_start = strstr(
        asa_src->op.read.dst, "\n"
                              "#EXT-X-STREAM-INF"
    );
    if (!stream_inf_start) {
        fprintf(stderr, "[hls][mst_plist] line:%d, invalid content \r\n", __LINE__);
        goto done;
    }
    char *stream_inf_end = strstr(stream_inf_start + 1, "\n"); // skip new-line chars
    if (!stream_inf_end)
        goto done;
    stream_inf_end += 1; // including the new-line chars
    { // construct URL of each media playlist, write it to the end of copied data in read buffer
        json_t       *spec = processor->data.spec;
        const char   *host_domain = json_string_value(json_object_get(spec, "host_domain"));
        const char   *host_path = json_string_value(json_object_get(spec, "host_path"));
        const char   *doc_id = json_string_value(json_object_get(spec, "doc_id"));
        json_t       *qp_labels = json_object_get(spec, "query_param_label");
        const char   *doc_id_label = json_string_value(json_object_get(qp_labels, "doc_id"));
        const char   *detail_label = json_string_value(json_object_get(qp_labels, "detail"));
        asa_dirent_t *ver_entry = &asa_src->op.scandir.fileinfo.data[entry_idx];
        size_t        nb_buf_avail =
            asa_src->op.read.dst_max_nbytes - ((size_t)stream_inf_end - (size_t)asa_src->op.read.dst);
#define HLS__URL_PATTERN "https://%s%s?%s=%s&%s=%s/%s\n"
        size_t nwrite = snprintf(
            stream_inf_end, nb_buf_avail, HLS__URL_PATTERN, host_domain, host_path, doc_id_label, doc_id,
            detail_label, ver_entry->name, HLS_PLAYLIST_FILENAME
        );
        assert(nb_buf_avail > nwrite); // build path of secondary playlist
#undef HLS__URL_PATTERN
        stream_inf_end += nwrite; // including the new generaated URL
    }
    char  *wr_buf = (hlsproc->internal.num_plist_merged++ == 0) ? asa_src->op.read.dst : stream_inf_start;
    size_t wr_sz = (size_t)stream_inf_end - (size_t)wr_buf;
    processor->transfer.streaming_dst.block.data = wr_buf;
    processor->transfer.streaming_dst.block.len = wr_sz;
done:
    asa_src->op.close.cb = _atfp_hls__close_src_mst_plist_cb;
    result = asa_src->storage->ops.fn_close(asa_src);
    if (result != ASTORAGE_RESULT_ACCEPT)
        _atfp_hls__close_src_mst_plist_cb(asa_src, result);
} // end of  _atfp_hls__read_src_mst_plist_cb

static void _atfp_hls__open_src_mst_plist_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &hlsproc->super;
    asa_src->op.open.dst_path = NULL;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_src->op.read.dst_sz = asa_src->op.read.dst_max_nbytes - 1;
        asa_src->op.read.cb = _atfp_hls__read_src_mst_plist_cb;
        result = asa_src->storage->ops.fn_read(asa_src);
        if (result != ASTORAGE_RESULT_ACCEPT)
            fprintf(stderr, "[hls][mst_plist] line:%d, error on reading file \r\n", __LINE__);
    } else { // it is possible to have other video quality encoded with non-HLS format
        fprintf(stderr, "[hls][mst_plist] line:%d, error on opening file \r\n", __LINE__);
        result = atfp_hls__open_src_mst_plist(hlsproc);
    }
    if (result != ASTORAGE_RESULT_ACCEPT) {
        if (result != ASTORAGE_RESULT_EOF_SCAN)
            json_object_set_new(processor->data.error, "storage", json_string("[hls] internal error"));
        processor->data.callback(processor);
    }
} // end of  _atfp_hls__open_src_mst_plist_cb

static ASA_RES_CODE atfp_hls__open_src_mst_plist(atfp_hls_t *hlsproc) {
    ASA_RES_CODE       result;
    atfp_t            *processor = &hlsproc->super;
    asa_op_base_cfg_t *asa_src = processor->data.storage.handle;
    json_t            *err_info = processor->data.error;
    uint32_t           max_num_files = asa_src->op.scandir.fileinfo.size;
    uint32_t           curr_rd_idx = asa_src->op.scandir.fileinfo.rd_idx;
    asa_dirent_t      *entry = NULL;
    int                idx = curr_rd_idx;
    for (idx = curr_rd_idx; (!entry) && (idx < max_num_files); idx++) {
        asa_dirent_t *e = &asa_src->op.scandir.fileinfo.data[idx];
        if (e->type != ASA_DIRENT_DIR)
            continue;
        if (strlen(e->name) != APP_TRANSCODED_VERSION_SIZE)
            continue;
        entry = e;
    } // end of loop
    asa_src->op.scandir.fileinfo.rd_idx = idx;
    processor->transfer.streaming_dst.flags.is_final = idx >= max_num_files;
    if (entry) {
        size_t basepath_sz = strlen(asa_src->op.scandir.path);
        size_t filename_sz = sizeof(HLS_MASTER_PLAYLIST_FILENAME);
        size_t filepath_sz = basepath_sz + 1 + APP_TRANSCODED_VERSION_SIZE + 1 + filename_sz + 1;
        char   filepath[filepath_sz];
        size_t nwrite = snprintf(
            &filepath[0], filepath_sz, "%s/%s/%s", asa_src->op.scandir.path, entry->name,
            HLS_MASTER_PLAYLIST_FILENAME
        );
        assert(filepath_sz >= nwrite);
        asa_src->op.open.dst_path = &filepath[0];
        asa_src->op.open.mode = S_IRUSR;
        asa_src->op.open.flags = O_RDONLY;
        asa_src->op.open.cb = _atfp_hls__open_src_mst_plist_cb;
        result = asa_src->storage->ops.fn_open(asa_src);
        if (result != ASTORAGE_RESULT_ACCEPT) {
            fprintf(stderr, "[hls][mst_plist] line:%d, failed to open file \r\n", __LINE__);
            json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
            asa_src->op.open.dst_path = NULL;
        }
    } else { // end of video version iteration, no more media playlist
        fprintf(stderr, "[hls][mst_plist] line:%d, reach end of iteration on version folders \r\n", __LINE__);
        result = ASTORAGE_RESULT_EOF_SCAN;
        if (hlsproc->internal.num_plist_merged == 0) {
            json_object_set_new(err_info, "_http_resp_code", json_integer(404));
            json_object_set_new(
                err_info, "transcoder", json_string("[hls] source master playlist not found")
            );
        }
    }
    return result;
} // end of  atfp_hls__open_src_mst_plist

void atfp_hls_stream__build_mst_plist__continue(atfp_hls_t *hlsproc) {
    atfp_t *processor = &hlsproc->super;
    processor->transfer.streaming_dst.block.data = NULL;
    processor->transfer.streaming_dst.block.len = 0;
    ASA_RES_CODE result = atfp_hls__open_src_mst_plist(hlsproc);
    if (result == ASTORAGE_RESULT_EOF_SCAN)
        processor->data.callback(processor);
}

static void atfp_hls__scandir_versions_cb(asa_op_base_cfg_t *asa_src, ASA_RES_CODE result) {
    atfp_hls_t *hlsproc = (atfp_hls_t *)asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t     *processor = &hlsproc->super;
    json_t     *err_info = processor->data.error;
    int         _http_resp_code = 500;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        int err = atfp_scandir_load_fileinfo(asa_src, err_info);
        if (!err) {
            uint32_t num_versions_found = asa_src->op.scandir.fileinfo.size;
            if (num_versions_found > 0) {
                // change to another function iterating different version folders for master
                // playlist
                hlsproc->internal.num_plist_merged = 0;
                hlsproc->internal.op.build_master_playlist = atfp_hls_stream__build_mst_plist__continue;
            } else {
                json_object_set_new(err_info, "hls", json_string("[storage] no version avaiable"));
                fprintf(
                    stderr, "[hls][mst_plist] line:%d, no version available in the path:%s \r\n", __LINE__,
                    asa_src->op.scandir.path
                );
            }
        } else {
            fprintf(
                stderr, "[hls][mst_plist] line:%d, error when loading file info from the path:%s \r\n",
                __LINE__, asa_src->op.scandir.path
            );
        }
    } else {
        _http_resp_code = 404;
        json_object_set_new(err_info, "storage", json_string("[hls] unknown source path"));
        fprintf(stderr, "[hls][mst_plist] line:%d, error on scandir, versions unknown \r\n", __LINE__);
    }
    if (json_object_size(err_info) > 0)
        json_object_set_new(err_info, "_http_resp_code", json_integer(_http_resp_code));
    processor->data.callback(processor);
} // end of  atfp_hls__scandir_versions_cb

static ASA_RES_CODE atfp_hls_stream__build_mst_plist__start(asa_op_base_cfg_t *asa_src, atfp_t *processor) {
    uint32_t _usr_id = processor->data.usr_id;
    uint32_t _upld_req_id = processor->data.upld_req_id;
#define ASA_SRC_BASEPATH_PATTERN "%d/%08x/%s"
    size_t scan_path_sz = sizeof(ASA_SRC_BASEPATH_PATTERN) + USR_ID_STR_SIZE +
                          UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(ATFP__COMMITTED_FOLDER_NAME) + 1;
    char  *scanning_path = calloc(scan_path_sz, sizeof(char));
    size_t nwrite = snprintf(
        &scanning_path[0], scan_path_sz, ASA_SRC_BASEPATH_PATTERN, _usr_id, _upld_req_id,
        ATFP__COMMITTED_FOLDER_NAME
    );
    assert(scan_path_sz >= nwrite);
    asa_src->op.scandir.path = scanning_path;
    asa_src->op.scandir.cb = atfp_hls__scandir_versions_cb;
    return asa_src->storage->ops.fn_scandir(asa_src);
#undef ASA_SRC_BASEPATH_PATTERN
}

void atfp_hls_stream__build_mst_plist(atfp_hls_t *hlsproc) {
    json_t *spec = hlsproc->super.data.spec;
    json_object_set_new(spec, "num_usrargs_asa_src", json_integer(NUM_USRARGS_ASA_SRC));
    atfp_hls_stream_seeker__init_common(hlsproc, atfp_hls_stream__build_mst_plist__start);
    hlsproc->asa_local.super.deinit = NULL; // always use default de-init function in ../init_stream.c
}
