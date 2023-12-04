#ifndef MEDIA__UTILS_H
#define MEDIA__UTILS_H
#ifdef __cplusplus
extern "C" {
#endif

#include <string.h>
#include <search.h>
#include <jansson.h>

typedef struct app_llnode_s {
    char   dummy;
    struct app_llnode_s *next;
    struct app_llnode_s *prev;
    char   data[1]; // may extend the storage space based on given type
} app_llnode_t;

void * app_fetch_from_hashmap(struct hsearch_data *hmap, const char *keyword);

int app_save_int_to_hashmap(struct hsearch_data *hmap, const char *keyword, int value);
int app_save_ptr_to_hashmap(struct hsearch_data *hmap, const char *keyword, void *value);

int app_url_decode_query_param(char *data, json_t *map);

void app_llnode_link(app_llnode_t *curr, app_llnode_t *prev, app_llnode_t *new);

void app_llnode_unlink(app_llnode_t *node);

int app_chararray_to_hexstr(char *out, size_t out_sz, const char *in, size_t in_sz);
int app_hexstr_to_chararray(char *out, size_t out_sz, const char *in, size_t in_sz);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__UTILS_H
