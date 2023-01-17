#ifndef MEDIA__TRANSCODER__FILE_PROCESSOR_H
#define MEDIA__TRANSCODER__FILE_PROCESSOR_H
#ifdef __cplusplus
extern "C" {
#endif

#include <openssl/sha.h>
#include <openssl/evp.h>
#include <jansson.h>
#include <h2o.h>

#include "models/datatypes.h"
#include "storage/datatypes.h"
#include "storage/localfs.h"
#include "rpc/datatypes.h"


// identification of backend library for transcoders
typedef enum {
    ATFP_BACKEND_LIB__UNKNOWN = 0,
    ATFP_BACKEND_LIB__FFMPEG,
    ATFP_BACKEND_LIB__LIBVLC
} ATFP_BACKEND_LIB_TYPE;

typedef enum {
    ATFP_AVCTX_RET__OK = 0,
    ATFP_AVCTX_RET__NEED_MORE_DATA = 1,
    ATFP_AVCTX_RET__END_OF_FLUSH_ENCODER = 2,
} avctx_fn_ret_code;

struct atfp_s;

typedef  struct atfp_av_ctx_s   atfp_av_ctx_t;

typedef struct {
    json_t *error;
    json_t *spec;
    arpc_receipt_t  *rpc_receipt;
    const char *version; // point to version label string
    uint32_t  usr_id;
    uint32_t  upld_req_id;
    void (*callback)(struct atfp_s *);
    struct {
        const char *basepath;
        asa_op_base_cfg_t *handle;
    } storage;
} atfp_data_t;

typedef struct atfp_ops_s {
    // TODO, rename `init` and `deinit` to `init_transcode` and `deinit_transcode`
    void     (*init)(struct atfp_s *);
    uint8_t  (*deinit)(struct atfp_s *); // return value indicates whether de-init is still ongoing
    void     (*processing)(struct atfp_s *);
    uint8_t  (*has_done_processing)(struct atfp_s *);
    uint8_t  (*label_match)(const char *label);
    struct atfp_s *(*instantiate)(void);
} atfp_ops_t;

typedef struct {
    ATFP_BACKEND_LIB_TYPE  backend_id;
    // TODO, look for better implementation option, this member cannot be read-only for few unit-test cases
    //const
    atfp_ops_t  ops;
} atfp_ops_entry_t;

typedef struct atfp_s {
    atfp_data_t  data;
    const atfp_ops_t  *ops;
    ATFP_BACKEND_LIB_TYPE  backend_id;
    struct { // store current index of source file chunks
        uint32_t  curr;  // currently opened file chunk of the source file
        uint32_t  next;
        asa_open_cb_t  usr_cb;
        uint8_t   eof_reached;
        // TODO, move into  `transfer`.`transcoded_dst` field below, it is used only
        // for source file processor during transcoding
    } filechunk_seq;
    struct {
        // indicate the operation can complete its task (regardless of errors happening) in
        //  current loop cycle (value = 0), or it requires more event-loop cycles (value = 1)
        // to get task done
        uint8_t  init:1;
        uint8_t  processing:1;
    } op_async_done;
    union {
        struct {
            struct {
                char   *data;
                size_t  len;
            } block;
            struct {
                uint8_t  is_final:1;
                uint8_t  eof_reached:1;
            }  flags;
        } streaming_dst;
        struct {
            void   (*update_metadata)(struct atfp_s *, void *loop);
            void   (*remove_file)(struct atfp_s *, const char *status);
            uint32_t  tot_nbytes_file;
            json_t   *info;
            struct { // for transfer transcoded files from local to destination storage
                uint8_t  asalocal_open:1;
                uint8_t  asaremote_open:1;
                uint8_t  version_exists:1; // TODO:rename to `version_metadata_exists`
                uint8_t  version_created:1;
            }  flags;
        } transcoded_dst;
    } transfer;
} atfp_t;

typedef struct {
    // indicate list of numbers for transcoded segment files
    H2O_VECTOR(int) rdy_list;
    struct {
        struct {
            char *data;
            size_t sz;
        } prefix;
        struct {
            char *data;
            size_t sz;
            uint8_t max_num_digits;
        } pattern;
    } filename;
    struct {
        struct {
            char *data;
            size_t sz;
        } _asa_local;
        struct {
            char *data;
            size_t sz;
        } _asa_dst;
    } fullpath;
    SHA_CTX   checksum; // currently SHA1 is used to calculate checksum of each file
    struct {
        size_t    nbytes;   // nbytes per file after transcoded, including segment, metadata file, etc.
        uint32_t  curr_idx; // the index to the item in `rdy_list` field above
        uint8_t   eof_reached:1;
    } transfer;
} atfp_segment_t; // used for moving transcoded files from dedicated server to remote storage

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

// for caching / processing streaming file
typedef  void (*asa_cch_proceed_cb_t) (asa_op_base_cfg_t *, ASA_RES_CODE, h2o_iovec_t *, uint8_t is_final);

typedef struct {
    struct {
        asa_open_cb_t    init;
        asa_close_cb_t   deinit;
        asa_cch_proceed_cb_t    proceed;
    } callback;
    struct {
        uint8_t  locked:1;
    } flags;
} asa_cch_usrdata_t;

#define   ATFP__TEMP_TRANSCODING_FOLDER_NAME  "transcoding"
#define   ATFP__COMMITTED_FOLDER_NAME         "committed"
#define   ATFP__DISCARDING_FOLDER_NAME        "discarding"
#define   ATFP__MAXSZ_STATUS_FOLDER_NAME   MAX(sizeof(ATFP__TEMP_TRANSCODING_FOLDER_NAME),MAX(sizeof(ATFP__COMMITTED_FOLDER_NAME),sizeof(ATFP__DISCARDING_FOLDER_NAME)))

#define  ATFP_CACHED_FILE_FOLDERNAME   "cached"
#define  ATFP_ENCRYPT_METADATA_FILENAME   "metadata.json"
#define  ATFP__CRYPTO_KEY_MOST_RECENT   "recent"
// In the transcoder, `atfp_t` object requires that each object of `asa_op_base_cfg_t` type
//  should be able to find back to itself in the callback of `asa_op_base_cfg_t` type.
// For simplicity, the transcoder `atfp_t` reserves the first field of user arguments of
// `asa_op_base_cfg_t` type  as a pointer to the associated file processor
#define  ATFP_INDEX__IN_ASA_USRARG     0
#define  ASAMAP_INDEX__IN_ASA_USRARG   1
#define  SPEC_INDEX__IN_ASA_USRARG     2
#define  ERRINFO_INDEX__IN_ASA_USRARG  3

atfp_t * app_transcoder_file_processor(const char *label);

uint8_t  atfp_common__label_match(const char *label, size_t num, const char **exp_labels);

ASA_RES_CODE  atfp_open_srcfile_chunk ( asa_op_base_cfg_t *,  const char *basepath,
        int chunk_seq, asa_open_cb_t);

ASA_RES_CODE  atfp_switch_to_srcfile_chunk(atfp_t *, int chunk_seq, asa_open_cb_t);

// common callback for opening temp buffer file in local API server
ASA_RES_CODE  atfp_src__open_localbuf(asa_op_base_cfg_t *, asa_open_cb_t);
// common callback for reading bytes from the file in source storage, then perform write to local buffer
int  atfp_src__rd4localbuf_done_cb (asa_op_base_cfg_t *, ASA_RES_CODE, size_t nread, asa_write_cb_t);

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

ASA_RES_CODE  atfp__segment_start_transfer(
        asa_op_base_cfg_t     *asa_dst,
        asa_op_localfs_cfg_t  *asa_local,
        atfp_segment_t        *seg_cfg,
        int chosen_idx );

ASA_RES_CODE  atfp__file_start_transfer(
        asa_op_base_cfg_t     *asa_dst,
        asa_op_localfs_cfg_t  *asa_local,
        atfp_segment_t        *seg_cfg,
        const char *filename_local,
        const char *filename_dst  );


int atfp_segment_init(atfp_segment_t *);
int atfp_segment_final(atfp_segment_t *, json_t *info);

int  atfp_scandir_load_fileinfo (asa_op_base_cfg_t *, json_t *err_info);

void  atfp_storage__commit_new_version(atfp_t *);

int  atfp_check_fileupdate_required(atfp_data_t *, const char *basepath,
        const char *filename, float threshold_secs);
json_t * atfp_image_mask_pattern_index (const char *basepath);

// for crypto / key encryption
size_t  atfp_get_encrypted_file_basepath (const char *basepath, char *out, size_t o_sz,
        const char *doc_id, size_t id_sz);
const char * atfp_get_crypto_key (json_t *keyinfo, const char *key_id, json_t **item_out);
int  atfp_encrypt_document_id (EVP_CIPHER_CTX *, atfp_data_t *, json_t *kitem, unsigned char **out, size_t *out_sz);

// for validating frontend  request at API server
int   atfp_validate_transcode_request (const char *resource_type, json_t *spec, json_t *err_info);
void  atfp_validate_req_dup_version(const char *resource_type, json_t *spec, db_query_row_info_t *existing);
const char * atfp_transcoded_version_sql_pattern(const char *res_typ, size_t *out_sz);

// for cached streaming files at local API server
asa_op_localfs_cfg_t  *atfp_streamcache_init (void *loop, json_t *spec, json_t *err_info, uint8_t num_cb_args,
       uint32_t buf_sz, asa_open_cb_t  _init_cb, asa_close_cb_t  _deinit_cb);
void  atfp_streamcache_proceed_datablock (asa_op_base_cfg_t *, asa_cch_proceed_cb_t);
int  atfp_cache_save_metadata(const char *basepath, const char *mimetype, atfp_data_t *);

void  atfp__close_local_seg__cb  (asa_op_base_cfg_t *, atfp_segment_t *, ASA_RES_CODE);
void  atfp__unlink_local_seg__cb (asa_op_base_cfg_t *, ASA_RES_CODE);
void  atfp__open_local_seg__cb (asa_op_base_cfg_t *, ASA_RES_CODE);
void  atfp__open_dst_seg__cb   (asa_op_base_cfg_t *, atfp_segment_t *, ASA_RES_CODE);
void  atfp__read_local_seg__cb (asa_op_base_cfg_t *, atfp_segment_t *, ASA_RES_CODE, size_t nread);
void  atfp__write_dst_seg__cb  (asa_op_base_cfg_t *, atfp_segment_t *, ASA_RES_CODE, size_t nwrite);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__FILE_PROCESSOR_H
