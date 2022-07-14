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
            asa_op_base_cfg_t *_handles; // for internal use, application must not modify this
            asa_cfg_t         *config;
        } storage;
    } dst;
} atfp_data_t;

typedef struct {
    void (*init)(struct atfp_s *);
    void (*deinit)(struct atfp_s *);
    void (*processing)(struct atfp_s *);
} atfp_ops_t;

typedef struct {
    const char *mimetype;
    const atfp_ops_t ops;
} atfp_ops_entry_t;

typedef struct atfp_s {
    atfp_data_t  data;
    const atfp_ops_t  *ops;
    json_t  *transcoded_info;
} atfp_t;

atfp_t * app_transcoder_file_processor(const char *mimetype);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__TRANSCODER__FILE_PROCESSOR_H
