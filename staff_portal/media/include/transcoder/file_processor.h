#ifndef MEDIA__TRANSCODER__FILE_PROCESSOR_H
#define MEDIA__TRANSCODER__FILE_PROCESSOR_H
#ifdef __cplusplus
extern "C" {
#endif

#include <jansson.h>
#include "storage/datatypes.h"
#include "storage/localfs.h"

// identification of backend library for transcoders
typedef enum {
    ATFP_BACKEND_LIB__UNKNOWN = 0,
    ATFP_BACKEND_LIB__FFMPEG,
    ATFP_BACKEND_LIB__LIBVLC
} ATFP_BACKEND_LIB_TYPE;

struct atfp_s;

typedef  struct atfp_av_ctx_s   atfp_av_ctx_t;

typedef struct {
    json_t *error;
    json_t *spec;
    void (*callback)(struct atfp_s *);
    struct {
        const char *basepath;
        asa_op_base_cfg_t *handle;
        asa_cfg_t         *config;
    } storage;
} atfp_data_t;

typedef struct atfp_ops_s {
    void  (*init)(struct atfp_s *);
    void  (*deinit)(struct atfp_s *);
    void  (*processing)(struct atfp_s *);
    uint8_t  (*has_done_processing)(struct atfp_s *);
    uint8_t  (*label_match)(const char *label);
    struct atfp_s *(*instantiate)(void);
} atfp_ops_t;

typedef struct {
    ATFP_BACKEND_LIB_TYPE  backend_id;
    const atfp_ops_t ops;
} atfp_ops_entry_t;

typedef struct atfp_s {
    atfp_data_t  data;
    const atfp_ops_t  *ops;
    ATFP_BACKEND_LIB_TYPE  backend_id;
    json_t   *transcoded_info;
    struct { // store current index of source file chunks
        uint32_t  curr;  // currently opened file chunk of the source file
        uint32_t  next;
        asa_open_cb_t  usr_cb;
        uint8_t   eof_reached;
    } filechunk_seq;
} atfp_t;

typedef struct {
    asa_op_base_cfg_t  *handle;
    struct {
        uint8_t  working:1;
    } flags; // for internal use
} _asamap_dst_entry_t;

// structure one-to-many relationship bwtween one source file-processor and
// multiple transcoded destination file-processors
typedef struct {
    asa_op_localfs_cfg_t  *local_tmp;
    asa_op_base_cfg_t     *src;
    struct {
        _asamap_dst_entry_t  *entries;
        uint8_t  capacity;
        uint8_t  size;
        uint8_t  iter_idx;
    } dst;
    int   app_sync_cnt;
} atfp_asa_map_t;

#define   ATFP_TEMP_TRANSCODING_FOLDER_NAME  "transcoding"
// In the transcoder, `atfp_t` object requires that each object of `asa_op_base_cfg_t` type
//  should be able to find back to itself in the callback of `asa_op_base_cfg_t` type.
// For simplicity, the transcoder `atfp_t` reserves the first field of user arguments of
// `asa_op_base_cfg_t` type  as a pointer to the associated file processor
#define  ATFP_INDEX__IN_ASA_USRARG    0
#define  ASAMAP_INDEX__IN_ASA_USRARG  1

atfp_t * app_transcoder_file_processor(const char *label);

uint8_t  atfp_common__label_match(const char *label, size_t num, const char **exp_labels);

ASA_RES_CODE  atfp_open_srcfile_chunk(
        asa_op_base_cfg_t *cfg,
        asa_cfg_t  *storage,
        const char *basepath,
        int         chunk_seq,
        asa_open_cb_t  cb );

ASA_RES_CODE  atfp_switch_to_srcfile_chunk(atfp_t *processor, int chunk_seq, asa_open_cb_t cb);

// given a position in `pos` starting from the file chunk where index is specified in `chunk_idx_start`,
// estimate index number of destination file chunk, and then update `pos` with read offset of the
// destination file chunk.
int  atfp_estimate_src_filechunk_idx(json_t *spec, int chunk_idx_start, size_t *pos);

atfp_asa_map_t  *atfp_asa_map_init(uint8_t num_dst);
void             atfp_asa_map_deinit(atfp_asa_map_t *);

void     atfp_asa_map_set_source(atfp_asa_map_t *, asa_op_base_cfg_t *);
void     atfp_asa_map_set_localtmp(atfp_asa_map_t *, asa_op_localfs_cfg_t *);
uint8_t  atfp_asa_map_add_destination(atfp_asa_map_t *, asa_op_base_cfg_t *);
uint8_t  atfp_asa_map_remove_destination(atfp_asa_map_t *, asa_op_base_cfg_t *);

asa_op_localfs_cfg_t  *atfp_asa_map_get_localtmp(atfp_asa_map_t *);
asa_op_base_cfg_t  *atfp_asa_map_get_source(atfp_asa_map_t *);
asa_op_base_cfg_t  *atfp_asa_map_iterate_destination(atfp_asa_map_t *);
void     atfp_asa_map_reset_dst_iteration(atfp_asa_map_t *);
uint8_t  atfp_asa_map_dst_start_working(atfp_asa_map_t *, asa_op_base_cfg_t *);
uint8_t  atfp_asa_map_dst_stop_working(atfp_asa_map_t *, asa_op_base_cfg_t *);
uint8_t  atfp_asa_map_all_dst_stopped(atfp_asa_map_t *);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__FILE_PROCESSOR_H
