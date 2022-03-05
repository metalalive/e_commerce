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

void * fetch_from_hashmap(struct hsearch_data *hmap, ENTRY keyword);

int app_url_decode_query_param(char *data, json_t *map);

void app_llnode_link(app_llnode_t *curr, app_llnode_t *prev, app_llnode_t *new);

void app_llnode_unlink(app_llnode_t *node);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__UTILS_H
