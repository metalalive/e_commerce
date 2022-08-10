#ifndef MEDIA__TRANSCODER__VIDEO_MP4_H
#define MEDIA__TRANSCODER__VIDEO_MP4_H
#ifdef __cplusplus
extern "C" {
#endif

#include "transcoder/file_processor.h"

typedef struct {
    uint32_t size;
    uint32_t type;
} mp4_atom;

struct atfp_mp4_s ;

typedef struct atfp_mp4_s {
    atfp_t  super;
    atfp_av_ctx_t  *av;
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
            // `pos` field above means the position in the chunk file indexed with `fchunk_seq`,
            // while `pos_wholefile` means the position in the whole (not segmented) file 
            size_t    pos_wholefile;
            size_t    nb_preloaded; // total num of bytes preloaded in `mdat` atom
        } mdat;
        size_t  nread_prev_chunk;
        struct {
            void (*preload_done)(struct atfp_mp4_s *);
            void (*av_init_done)(struct atfp_mp4_s *);
        } callback;
    } internal;
} atfp_mp4_t;


ASA_RES_CODE  atfp_mp4__preload_stream_info (atfp_mp4_t *, void (*cb)(atfp_mp4_t *));

ASA_RES_CODE  atfp_mp4__preload_packet_sequence (atfp_mp4_t *mp4proc, int chunk_idx_start,
        size_t chunk_offset, size_t nbytes_to_load, void (*cb)(atfp_mp4_t *));

int  atfp_mp4__validate_source_format(atfp_mp4_t *mp4proc);

ASA_RES_CODE  atfp_mp4__av_init (atfp_mp4_t *, void (*cb)(atfp_mp4_t *));

void  atfp_mp4__av_deinit(atfp_mp4_t *);

ASA_RES_CODE  atfp_mp4__av_preload_packets (atfp_mp4_t *, void (*cb)(atfp_mp4_t *));
int  atfp_mp4__av_next_local_packet(atfp_av_ctx_t *, void **packet_p);
int  atfp_mp4__av_decode_packet(atfp_av_ctx_t *, void *packet);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__VIDEO_MP4_H