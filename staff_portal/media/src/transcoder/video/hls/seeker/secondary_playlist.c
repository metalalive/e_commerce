#include <math.h>

#include "app_cfg.h"
#include "transcoder/video/hls.h"
#define   NUM_USRARGS_ASA_SRC     (ATFP_INDEX__IN_ASA_USRARG + 1)

// temporarily save current read pointer `rd_ptr` in  asa_local.super.op.read.dst
// TODO, should there be dedicated field for saving this pointer ?
#define  SAVE_CURR_READ_PTR(_hls_proc, _ptr)  _hls_proc->asa_local.super.op.read.dst = _ptr;
#define  LOAD_CURR_READ_PTR(_hls_proc)        _hls_proc->asa_local.super.op.read.dst;
#define  SAVE_LAST_NUM_UNREAD(_hls_proc, _val)   _hls_proc->asa_local.super.op.read.dst_sz = _val;
#define  LOAD_LAST_NUM_UNREAD(_hls_proc)         _hls_proc->asa_local.super.op.read.dst_sz 
#define  SAVE_CURR_SEGMENT_INDEX(_hls_proc, _val)  _hls_proc->asa_local.super.op.read.dst_max_nbytes = _val;
#define  LOAD_CURR_SEGMENT_INDEX(_hls_proc)        _hls_proc->asa_local.super.op.read.dst_max_nbytes

char * atfp_hls_lvl2pl__load_curr_rd_ptr(atfp_hls_t* _h)
{ return LOAD_CURR_READ_PTR(_h); }

void  atfp_hls_lvl2pl__save_curr_rd_ptr(atfp_hls_t* _h, char *p)
{  SAVE_CURR_READ_PTR(_h, p); }

size_t  atfp_hls_lvl2pl__load_num_unread(atfp_hls_t* _h)
{ return LOAD_LAST_NUM_UNREAD(_h); }

size_t  atfp_hls_lvl2pl__load_segment_idx(atfp_hls_t* _h)
{ return LOAD_CURR_SEGMENT_INDEX(_h); }

// TODO, since ffmpeg doesn't expose the functions for parsing HLS playlist to its public API
// , this function re-invent the wheels and  parses the playlist in asynchronous operation
// , find out better implementation option for this.


static __attribute__((optimize("O0")))  void _atfp_hls_stream__lvl2_plist__parse_extint (atfp_hls_t *hlsproc)
{
    atfp_t *processor = &hlsproc->super;
    json_t *spec = processor->data.spec;
    size_t  _wrbuf_max_sz = json_integer_value(json_object_get(spec, "wrbuf_max_sz"));
    size_t  curr_wr_sz = 0, url_placeholder_sz = 2 * 6;
#define  URL_PATTERN   "\nhttps://%s%s?%s=%s&%s=%s/" HLS_SEGMENT_FILENAME_PREFIX  HLS_SEGMENT_FILENAME_NUM_FORMAT
    const char *_host_domain = json_string_value(json_object_get(spec, "host_domain"));
    const char *_host_path   = json_string_value(json_object_get(spec, "host_path"));
    const char *_doc_id      = json_string_value(json_object_get(spec, API_QUERYPARAM_LABEL__RESOURCE_ID));
    json_t *qp_labels = json_object_get(spec, "query_param_label");
    const char *doc_id_label = json_string_value(json_object_get(qp_labels, "doc_id"));
    const char *detail_label = json_string_value(json_object_get(qp_labels, "detail"));
    size_t  extinf_predicted_sz =  sizeof("\n#EXTINF:,") + HLS_PLIST_TARGET_DURATION_MAX_BYTES +
        sizeof(URL_PATTERN) - url_placeholder_sz + strlen(_host_domain) + strlen(_host_path) +
        strlen(doc_id_label) + strlen(_doc_id) +  strlen(detail_label) + APP_TRANSCODED_VERSION_SIZE -
        sizeof(HLS_SEGMENT_FILENAME_NUM_FORMAT) + HLS_SEGMENT_FILENAME_FORMAT_MAX_DIGITS + 1 ;
    uint32_t  curr_seg_idx = LOAD_CURR_SEGMENT_INDEX(hlsproc);
    char *rd_ptr = LOAD_CURR_READ_PTR(hlsproc);
    char *wr_ptr = processor->transfer.streaming_dst.block.data;
    uint32_t  max_num_tags = _wrbuf_max_sz / extinf_predicted_sz;
    for(uint32_t idx = 0; idx < max_num_tags; idx++) {
        char *ahead_rd_p[2] = {0};
        ahead_rd_p[0] = strstr(rd_ptr, "\n#EXTINF:");
        if(ahead_rd_p[0])
            ahead_rd_p[1] = strchr(ahead_rd_p[0], ',');
        if(!ahead_rd_p[0] || !ahead_rd_p[1])
            break;
        size_t  cpy_sz = (size_t)ahead_rd_p[1] + 1 - (size_t)ahead_rd_p[0];
        if(_wrbuf_max_sz <= cpy_sz) {
            fprintf(stderr, "[hls][lvl2_plist] line:%d, insufficient stream buffer \r\n", __LINE__);
            break;
        }
        rd_ptr = ahead_rd_p[0];
        strncpy(wr_ptr, rd_ptr, cpy_sz);
        wr_ptr += cpy_sz,  rd_ptr = ahead_rd_p[1] + 1;  curr_wr_sz += cpy_sz, _wrbuf_max_sz -= cpy_sz;
        if(_wrbuf_max_sz <= extinf_predicted_sz) {
            fprintf(stderr, "[hls][lvl2_plist] line:%d, insufficient stream buffer \r\n", __LINE__);
            break;
        }
        cpy_sz = snprintf(wr_ptr, _wrbuf_max_sz, URL_PATTERN, _host_domain, _host_path, doc_id_label,
                _doc_id, detail_label, processor->data.version,  curr_seg_idx++);
        assert(extinf_predicted_sz > cpy_sz);
        wr_ptr += cpy_sz;  curr_wr_sz += cpy_sz, _wrbuf_max_sz -= cpy_sz;
    } // end of loop
    if (curr_wr_sz > 0) {
        processor->transfer.streaming_dst.block.len  = curr_wr_sz;
        SAVE_CURR_READ_PTR(hlsproc, rd_ptr);
    } else { // TODO, avoid endless loop in case that write buffer is not enough to cover a extinf tag
        processor->transfer.streaming_dst.flags.is_final = processor->transfer.streaming_dst.flags.eof_reached;
    }
    SAVE_CURR_SEGMENT_INDEX(hlsproc, curr_seg_idx);
    processor->data.callback(processor);
#undef   URL_PATTERN
} // end of  _atfp_hls_stream__lvl2_plist__parse_extint


static void  _atfp_hls__l2_plist_read_extinf_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    if(result == ASTORAGE_RESULT_COMPLETE) {
        processor->transfer.streaming_dst.flags.eof_reached = nread < asa_src->op.read.dst_sz;
        assert(asa_src->op.read.dst_sz >= nread);
        asa_src->op.read.dst[nread] = 0x0;
        size_t  nb_unread = LOAD_LAST_NUM_UNREAD(hlsproc);
        asa_src->op.read.dst -= nb_unread;
        SAVE_CURR_READ_PTR(hlsproc, asa_src->op.read.dst);
        _atfp_hls_stream__lvl2_plist__parse_extint (hlsproc);
    } else {
        fprintf(stderr, "[hls][lvl2_plist] line:%d, failed to read src playlist \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        processor->data.callback(processor);
    }
} // end of  _atfp_hls__l2_plist_read_extinf_cb


void atfp_hls_stream__lvl2_plist__parse_extinf (atfp_hls_t *hlsproc) {
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    char *rd_ptr = LOAD_CURR_READ_PTR(hlsproc);
    char *ahead_rd_p[2] = {0};
    ahead_rd_p[0] = strstr(rd_ptr, "\n#EXTINF:");
    if(ahead_rd_p[0])
        ahead_rd_p[1] = strchr(ahead_rd_p[0], ',');
    processor->transfer.streaming_dst.block.len  = 0;
    if(ahead_rd_p[0] && ahead_rd_p[1]) { // ensure at least one tag will be extracted
        _atfp_hls_stream__lvl2_plist__parse_extint (hlsproc);
    } else {
        if(processor->transfer.streaming_dst.flags.eof_reached) {
            processor->transfer.streaming_dst.flags.is_final = 1;
            processor->data.callback(processor);
        } else {
            asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
            size_t  _rdbuf_max_sz = asa_src->op.read.dst_max_nbytes - 1;
            size_t  nb_unread = (size_t)&asa_src->op.read.dst[_rdbuf_max_sz] - (size_t)rd_ptr;
            if(nb_unread > 0)
                memmove(asa_src->op.read.dst, rd_ptr, nb_unread);
            asa_src->op.read.dst += nb_unread;
            SAVE_LAST_NUM_UNREAD(hlsproc, nb_unread);
            asa_src->op.read.offset = asa_src->op.seek.pos;
            asa_src->op.read.dst_sz = asa_src->op.read.dst_max_nbytes - nb_unread - 1; // reserve last byte for NULL-terminating char
            asa_src->op.read.cb = _atfp_hls__l2_plist_read_extinf_cb;
            ASA_RES_CODE result = asa_src->storage->ops.fn_read(asa_src);
            if(result != ASTORAGE_RESULT_ACCEPT) {
                fprintf(stderr, "[hls][lvl2_plist] line:%d, error on reading src playlist \r\n", __LINE__);
                json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
                processor->data.callback(processor);
            }
        }
    }
} // end of  atfp_hls_stream__lvl2_plist__parse_extinf


static size_t  _atfp_hls__build_lvl2_plist__add_key_tag (json_t *spec, char **wr_ptr,
        size_t *curr_wr_sz,  size_t  _buf_max_sz)
{
    const char *_host_domain = json_string_value(json_object_get(spec, "host_domain"));
    const char *_host_path   = json_string_value(json_object_get(spec, "host_path"));
    const char *_doc_id      = json_string_value(json_object_get(spec, API_QUERYPARAM_LABEL__RESOURCE_ID));
    json_t *qp_labels = json_object_get(spec, "query_param_label");
    const char *doc_id_label = json_string_value(json_object_get(qp_labels, "doc_id"));
    const char *detail_label = json_string_value(json_object_get(qp_labels, "detail"));
#define  URL_PATTERN   "https://%s%s?%s=%s&%s=" HLS_REQ_KEYFILE_LABEL
#define  TAG_PATTERN   "\n#EXT-X-KEY:METHOD=AES-%hu,URI=\"" URL_PATTERN "\",IV=0x%s"
    json_t *iv_obj = json_object_get(json_object_get(spec, "_crypto_key"), "iv");
    const char *iv_raw = json_string_value(json_object_get(iv_obj, "data"));
    uint16_t  iv_nbytes = (uint16_t) json_integer_value(json_object_get(iv_obj, "nbytes"));
    size_t  xtag_key_sz = sizeof(TAG_PATTERN) + 3 + strlen(_host_domain) + strlen(_host_path) +
        strlen(doc_id_label) + strlen(_doc_id) + strlen(detail_label) + strlen(iv_raw) + 1;
    size_t max_wr_avail = _buf_max_sz - *curr_wr_sz;
    size_t nwrite = 0;
    if(xtag_key_sz < max_wr_avail) {
        nwrite = snprintf(*wr_ptr, max_wr_avail, TAG_PATTERN, (iv_nbytes << 3), _host_domain,
                _host_path, doc_id_label, _doc_id, detail_label, iv_raw);
        *curr_wr_sz += nwrite, *wr_ptr += nwrite;
    } else {
        fprintf(stderr, "[hls][lvl2_plist] line:%d, insufficient stream buffer \r\n", __LINE__);
    }
    return nwrite;
#undef   TAG_PATTERN
#undef   URL_PATTERN
} // end of  _atfp_hls__build_lvl2_plist__add_key_tag


static  __attribute__((optimize("O0"))) size_t  _atfp_hls__build_lvl2_plist__modify_map_tag (json_t *spec, const char *_version,
        char **wr_ptr, char **rd_ptr,   size_t *curr_wr_sz,  size_t  _buf_max_sz)
{
    size_t nwrite = 0, cpy_sz = 0;
    char *ahead_rd_p = strstr(*rd_ptr, HLS_FMP4_FILENAME);
    if(!ahead_rd_p) {
        fprintf(stderr, "[hls][lvl2_plist] line:%d, missing expected uri, playlist corrupted \r\n", __LINE__);
        goto done;
    }
#define  CONDITIONAL_WRITE(_cmd) { \
    size_t max_wr_avail = _buf_max_sz - *curr_wr_sz; \
    if(cpy_sz >= max_wr_avail) { \
        fprintf(stderr, "[hls][lvl2_plist] line:%d, insufficient stream buffer \r\n", __LINE__); \
        nwrite = 0; \
        goto done; \
    } \
    { _cmd } \
    *curr_wr_sz += cpy_sz, nwrite += cpy_sz; \
    *wr_ptr += cpy_sz; \
}
    cpy_sz = (size_t)ahead_rd_p - (size_t)*rd_ptr;
    CONDITIONAL_WRITE( strncpy(*wr_ptr, *rd_ptr, cpy_sz); )
#define  URL_PATTERN   "https://%s%s?%s=%s&%s=%s/"
    {
        const char *_host_domain = json_string_value(json_object_get(spec, "host_domain"));
        const char *_host_path   = json_string_value(json_object_get(spec, "host_path"));
        const char *_doc_id      = json_string_value(json_object_get(spec, API_QUERYPARAM_LABEL__RESOURCE_ID));
        json_t *qp_labels = json_object_get(spec, "query_param_label");
        const char *doc_id_label = json_string_value(json_object_get(qp_labels, "doc_id"));
        const char *detail_label = json_string_value(json_object_get(qp_labels, "detail"));
        cpy_sz = sizeof(URL_PATTERN) + strlen(_host_domain) + strlen(_host_path) + strlen(doc_id_label)
            + strlen(_doc_id) + strlen(detail_label) + APP_TRANSCODED_VERSION_SIZE + 1;
        CONDITIONAL_WRITE(
            cpy_sz = snprintf(*wr_ptr, cpy_sz, URL_PATTERN, _host_domain, _host_path, doc_id_label,
                _doc_id, detail_label, _version);   )
    }
#undef   URL_PATTERN
    *rd_ptr = ahead_rd_p,  ahead_rd_p = strchr(*rd_ptr, '\n');
    cpy_sz = (size_t)ahead_rd_p - (size_t)*rd_ptr + 1;
    CONDITIONAL_WRITE(
           strncpy(*wr_ptr, *rd_ptr, cpy_sz - 1);
           (*wr_ptr)[cpy_sz - 1] = 0x0;
           cpy_sz--;
        ) // append NULL char to the end, but the octet should NOT be sent out
    *rd_ptr = ahead_rd_p;
done:
    return nwrite;
#undef   CHECK_WRITE_BUFFER
} // end of  _atfp_hls__build_lvl2_plist__modify_map_tag


void  atfp_hls_stream__lvl2_plist__parse_header (atfp_hls_t *hlsproc)
{
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    json_t *spec = processor->data.spec;
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    size_t  _wrbuf_max_sz = json_integer_value(json_object_get(spec, "wrbuf_max_sz"));
    processor->transfer.streaming_dst.block.data = calloc(_wrbuf_max_sz, sizeof(char));
    processor->transfer.streaming_dst.block.len  = 0;
    char *common_start = asa_src->op.read.dst, *map_start = strstr(common_start, "\n#EXT-X-MAP:"),
         *first_seg_start = strstr(common_start, "\n#EXTINF:"),
         *common_end = (map_start ? map_start: first_seg_start);
    char *rd_ptr = common_start, *wr_ptr = processor->transfer.streaming_dst.block.data;
    size_t  curr_wr_sz = (size_t)common_end - (size_t)common_start;
    strncpy(wr_ptr, rd_ptr, curr_wr_sz);
    wr_ptr += curr_wr_sz,   rd_ptr += curr_wr_sz;
    size_t  nwrite = _atfp_hls__build_lvl2_plist__add_key_tag (spec, &wr_ptr, &curr_wr_sz, _wrbuf_max_sz);
    if(nwrite == 0) {
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        goto done;
    }
    if(common_end == map_start) {
        assert(rd_ptr == map_start);
        nwrite = _atfp_hls__build_lvl2_plist__modify_map_tag (spec, processor->data.version,
                &wr_ptr, &rd_ptr, &curr_wr_sz, _wrbuf_max_sz);
        if(nwrite == 0) {
            json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
            goto done;
        }
    }
    SAVE_CURR_READ_PTR(hlsproc, rd_ptr);
    SAVE_CURR_SEGMENT_INDEX(hlsproc, 0);
    processor->transfer.streaming_dst.block.len  = curr_wr_sz;
done:
    if(json_object_size(err_info) == 0)
        hlsproc->internal.op.build_secondary_playlist = atfp_hls_stream__lvl2_plist__parse_extinf;
    processor->data.callback(processor);
} // end of  atfp_hls_stream__lvl2_plist__parse_header


#define   EXT_M3U       0
#define   EXT_VERSION   1
#define   EXT_MAP       2
#define   EXT_INF       3
#define   EXT_TARGETDURATION   4
#define   EXT_MEDIA_SEQUENCE   5
#define   EXT_PLAYLIST_TYPE    6
#define   NUM_TAGS_HEADER      (EXT_PLAYLIST_TYPE + 1)
static int8_t  _atfp_hls_l2_plist_identify_tag(char *buf)
{
#define  _CODE(_tag_str, _tag_idx) \
    if(strncmp(buf, _tag_str, sizeof(_tag_str) - 1) == 0)  return (int8_t)_tag_idx;
    _CODE("#EXTM3U\n", EXT_M3U)
    _CODE("#EXT-X-VERSION:", EXT_VERSION)
    _CODE("#EXT-X-TARGETDURATION:", EXT_TARGETDURATION)
    _CODE("#EXT-X-MEDIA-SEQUENCE:", EXT_MEDIA_SEQUENCE)
    _CODE("#EXT-X-PLAYLIST-TYPE:", EXT_PLAYLIST_TYPE)
    _CODE("#EXT-X-MAP:", EXT_MAP)
    _CODE("#EXTINF:", EXT_INF)
    return -1;
#undef  _CODE
} // end of  _atfp_hls_l2_plist_identify_tag


static  void _atfp_hls__validate_l2_plist_header (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result, size_t nread)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    json_t *err_info = processor->data.error;
    if(result != ASTORAGE_RESULT_COMPLETE) {
        fprintf(stderr, "[hls][lvl2_plist] line:%d, failed to read src playlist \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        goto done;
    }
    processor->transfer.streaming_dst.flags.eof_reached = nread < asa_src->op.read.dst_sz;
    assert(asa_src->op.read.dst_max_nbytes > nread);
    asa_src->op.read.dst[nread] = 0x0;
    processor->transfer.streaming_dst.flags.is_final = 0;
    char *rd_buf = asa_src->op.read.dst, *buf_ptr = rd_buf;
    int8_t  tags_present[NUM_TAGS_HEADER] = {0}, tag_cnt = 1,  tag_idx = 0;
    int idx = 0;
    for(idx = 0, --buf_ptr; idx < NUM_TAGS_HEADER; idx++, buf_ptr = strchr(buf_ptr, '\n'))
    {
        tag_idx = _atfp_hls_l2_plist_identify_tag (++buf_ptr);
        if(tag_idx < 0)  // invalid tag found
            break;
        tags_present[tag_idx] = tag_cnt++;
    } // end of loop
    if(tag_idx < 0) {
        json_object_set_new(err_info, "transcoder", json_string("[hls] invalid tag found in playlist"));
        goto done;
    }
    uint8_t  required_tags_received = (tags_present[EXT_M3U] > 0) && (tags_present[EXT_VERSION] > 0) &&
        (tags_present[EXT_TARGETDURATION] > 0) && (tags_present[EXT_PLAYLIST_TYPE] > 0) &&
        (tags_present[EXT_MAP] > 0) && (tags_present[EXT_INF] > 0);
    if(!required_tags_received) {
        json_object_set_new(err_info, "transcoder", json_string("[hls] missing required tag"));
        goto done;
    } // must include at least one media segment
    if(tags_present[EXT_MAP] > tags_present[EXT_INF]) 
        json_object_set_new(err_info, "transcoder", json_string("[hls] init map has to be loaded before first media segment"));
done:
    if(json_object_size(err_info) == 0)
        hlsproc->internal.op.build_secondary_playlist = atfp_hls_stream__lvl2_plist__parse_header;
    processor->data.callback(processor);
} // end of  _atfp_hls__validate_l2_plist_header
#undef  EXT_M3U
#undef  EXT_VERSION
#undef  EXT_MAP
#undef  EXT_INF
#undef  EXT_TARGETDURATION
#undef  EXT_PLAYLIST_TYPE
#undef  NUM_TAGS_HEADER



static  void _atfp_hls__close_local_keyfile_cb (asa_op_base_cfg_t *_asa_local, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) H2O_STRUCT_FROM_MEMBER(atfp_hls_t, asa_local, _asa_local);
    atfp_t *processor = & hlsproc->super;
    json_t *err_info = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
        asa_src->op.read.dst_sz = asa_src->op.read.dst_max_nbytes - 1; // reserve last byte for NULL-terminating char
        asa_src->op.read.cb = _atfp_hls__validate_l2_plist_header;
        result = asa_src->storage->ops.fn_read(asa_src);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            fprintf(stderr, "[hls][lvl2_plist] line:%d, error on reading src playlist \r\n", __LINE__);
            json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        }
    } else {
        fprintf(stderr, "[hls][lvl2_plist] line:%d, error on closing crypto key file \r\n", __LINE__);
        json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
    }
    if(result != ASTORAGE_RESULT_ACCEPT)
        processor->data.callback(processor);
} // end of _atfp_hls__close_local_keyfile_cb


static  void _atfp_hls__open_src_l2_plist_cb (asa_op_base_cfg_t *asa_src, ASA_RES_CODE result)
{
    atfp_hls_t *hlsproc = (atfp_hls_t *) asa_src->cb_args.entries[ATFP_INDEX__IN_ASA_USRARG];
    atfp_t *processor = &hlsproc->super;
    json_t *err_info  = processor->data.error;
    if (result == ASTORAGE_RESULT_COMPLETE) {
        result = atfp_hls_stream__load_crypto_key__async (hlsproc, _atfp_hls__close_local_keyfile_cb);
        if(result != ASTORAGE_RESULT_ACCEPT) {
            fprintf(stderr, "[hls][lvl2_plist] line:%d, error on opening crypto key file \r\n", __LINE__);
            json_object_set_new(err_info, "storage", json_string("[hls] internal error"));
        }
    } else { // it is possible to have other video quality encoded with non-HLS format
        fprintf(stderr, "[hls][lvl2_plist] line:%d, error on opening src secondary playlist \r\n", __LINE__);
        json_object_set_new(err_info, "_http_resp_code", json_integer(404));
        json_object_set_new(err_info, "storage", json_string("[hls] error on opening secondary playlist"));
    } // TODO, more advanced error handling, separate errors to client side 4xx or server side 5xx
    if(json_object_size(err_info) > 0)
        processor->data.callback(processor);
} // end of  _atfp_hls__open_src_l2_plist_cb


static ASA_RES_CODE  atfp_hls_stream__lvl2_plist__start (asa_op_base_cfg_t *asa_src, atfp_t *processor)
{
    uint32_t  _usr_id = processor->data.usr_id;
    uint32_t  _upld_req_id = processor->data.upld_req_id;
#define  PATH_PATTERN  "%s/%d/%08x/%s/%s/%s"
    size_t filepath_sz = sizeof(PATH_PATTERN) + strlen(asa_src->storage->base_path) + USR_ID_STR_SIZE +
          UPLOAD_INT2HEX_SIZE(_upld_req_id) + sizeof(ATFP__COMMITTED_FOLDER_NAME) +
          APP_TRANSCODED_VERSION_SIZE + sizeof(HLS_PLAYLIST_FILENAME);
    char filepath[filepath_sz];
    size_t nwrite = snprintf ( &filepath[0], filepath_sz, PATH_PATTERN, asa_src->storage->base_path, _usr_id,
             _upld_req_id, ATFP__COMMITTED_FOLDER_NAME, processor->data.version, HLS_PLAYLIST_FILENAME );
#undef  PATH_PATTERN 
    assert(filepath_sz >= nwrite);
    asa_src->op.open.dst_path = &filepath[0];
    asa_src->op.open.mode  = S_IRUSR;
    asa_src->op.open.flags = O_RDONLY;
    asa_src->op.open.cb  = _atfp_hls__open_src_l2_plist_cb;
    ASA_RES_CODE result = asa_src->storage->ops.fn_open(asa_src);
    asa_src->op.open.dst_path = NULL;
    return result;
} // end of  atfp_hls_stream__lvl2_plist__start


void  atfp_hls_stream__build_lvl2_plist(atfp_hls_t *hlsproc)
{
    atfp_t *processor = &hlsproc->super;
    json_t *spec = processor->data.spec;
    size_t  _rdbuf_max_sz = json_integer_value(json_object_get(spec, "buf_max_sz"));
    json_object_set_new(spec, "wrbuf_max_sz", json_integer(_rdbuf_max_sz)); // TODO, parameterize
    json_object_set_new(spec, "num_usrargs_asa_src", json_integer(NUM_USRARGS_ASA_SRC));
    atfp_hls_stream_seeker__init_common (hlsproc, atfp_hls_stream__lvl2_plist__start);
    json_object_del(spec, "num_usrargs_asa_src");
    asa_op_base_cfg_t  *asa_src = processor->data.storage.handle;
    assert(asa_src);
    assert(asa_src->deinit);
    assert(asa_src->op.read.dst);
    assert(asa_src->op.read.dst_max_nbytes > 0);
} // end of atfp_hls_stream__build_lvl2_plist
