#ifndef MEDIA__TRANSCODER__VIDEO_MP4_H
#define MEDIA__TRANSCODER__VIDEO_MP4_H
#ifdef __cplusplus
extern "C" {
#endif

#include "storage/localfs.h"
#include "transcoder/file_processor.h"

typedef struct {
    uint32_t size;
    uint32_t type;
} mp4_atom;


typedef  struct atfp_mp4_stream_ctx_s  atfp_mp4_stream_ctx_t;

struct atfp_mp4_s ;

typedef struct atfp_mp4_s {
    atfp_t  super;
    asa_op_localfs_cfg_t  local_tmpbuf_handle;
    struct {
        struct {
            size_t  size;
            size_t  nbytes_copied;
        } curr_atom; // TODO, union with preload_pkts field
        struct {
            size_t  size;
            size_t  nbytes_copied;
        } preload_pkts;
        struct {
            mp4_atom  header;
            size_t    fchunk_seq; 
            size_t    pos;  // position started immediately after first 8-byte header
            size_t    size; // the size without respect to first 8-byte header
        } mdat;
        size_t  nread_prev_chunk;
        struct {
            void (*preload_done)(struct atfp_mp4_s *);
            void (*avinput_init_done)(struct atfp_mp4_s *);
        } callback;
    } internal;
    struct {
        void     *fmt_ctx;
        atfp_mp4_stream_ctx_t *stream_ctx;
    } avinput;
} atfp_mp4_t;


ASA_RES_CODE  atfp_mp4__preload_stream_info (atfp_mp4_t *, void (*cb)(atfp_mp4_t *));

ASA_RES_CODE  atfp_mp4__preload_packet_sequence (atfp_mp4_t *mp4proc, int chunk_idx_start,
        size_t chunk_offset, size_t nbytes_to_load, void (*cb)(atfp_mp4_t *));

ASA_RES_CODE  atfp_mp4__avinput_init (atfp_mp4_t *, size_t num_init_pkts, void (*cb)(atfp_mp4_t *));

int  atfp_mp4__validate_source_format(atfp_mp4_t *mp4proc);

void  atfp_mp4__avinput_deinit(atfp_mp4_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_MP4_H
