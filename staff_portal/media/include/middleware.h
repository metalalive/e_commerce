#ifndef MEIDA__MIDDLEWARE_H
#define MEIDA__MIDDLEWARE_H
#ifdef __cplusplus
extern "C" {
#endif

#include <search.h>
#include "datatypes.h"

#define  NUM_ENTRIES_APP_HASHMAP 10

typedef struct _app_middleware_node_s
{
    struct _app_middleware_node_s *next;
    struct hsearch_data  *data;
    int (*fn)(RESTAPI_HANDLER_ARGS(self, req), struct _app_middleware_node_s *node);
} app_middleware_node_t;

typedef int (*app_middleware_fn)(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);


void app_cleanup_middlewares(app_middleware_node_t *head);

void app_run_next_middleware(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

app_middleware_node_t *app_gen_middleware_chain(size_t num_hdlrs, ...);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__AUTH_H
