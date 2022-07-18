#ifndef MEDIA__TRANSCODER__FILE_PROCESSOR_H
#define MEDIA__TRANSCODER__FILE_PROCESSOR_H
#ifdef __cplusplus
extern "C" {
#endif

#include <jansson.h>
#include "storage/datatypes.h"

struct atfp_s;

typedef struct {
    json_t *error;
    json_t *spec;
    void   *loop;
    void (*callback)(struct atfp_s *);
    const char *local_tmpbuf_basepath;
    struct {
        const char *basepath;
        struct {
            asa_op_base_cfg_t *handle;
            asa_cfg_t         *config;
        } storage;
    } src;
    struct {
        const char *basepath;
        struct {
            asa_op_base_cfg_t *handle;
            asa_cfg_t         *config;
        } storage;
    } dst;
} atfp_data_t;

typedef struct {
    void (*init)(struct atfp_s *);
    void (*deinit)(struct atfp_s *);
    void (*processing)(struct atfp_s *);
    size_t  (*get_obj_size)(void);
} atfp_ops_t;

typedef struct {
    const char *mimetype;
    const atfp_ops_t ops;
} atfp_ops_entry_t;

typedef struct atfp_s {
    atfp_data_t  data;
    const atfp_ops_t  *ops;
    json_t   *transcoded_info;
    struct { // store current index of source file chunks
        uint32_t  curr;  // currently opened file chunk of the source file
        uint32_t  next;
        asa_open_cb_t  usr_cb;
        uint8_t   eof_reached;
    } filechunk_seq;
} atfp_t;

// In the transcoder, the object of `asa_op_base_cfg_t` type needs to find back object of 
// the type `atfp_t` in the callback of each operations of `atfp_ops_t` type.
// For simplicity, the transcoder assumes that the first entry of the user argument fields
// of`asa_op_base_cfg_t` type  always store a pointer to object of `atfp_ops_t` type.
#define  ATFP_INDEX_IN_ASA_OP_USRARG  0

atfp_t * app_transcoder_file_processor(const char *mimetype);

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

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__FILE_PROCESSOR_H
