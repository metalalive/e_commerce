#ifndef MEIDA__MIDDLEWARE_H
#define MEIDA__MIDDLEWARE_H
#ifdef __cplusplus
extern "C" {
#endif

#include <search.h>
#include "datatypes.h"

#define  NUM_ENTRIES_APP_HASHMAP 15 // TODO, parameterize, for each API endpoint

typedef struct _app_middleware_node_s
{
    struct _app_middleware_node_s *next;
    struct hsearch_data  *data;
    int (*fn)(RESTAPI_HANDLER_ARGS(self, req), struct _app_middleware_node_s *node);
    struct {
        uint8_t  mandatory:1; // the middleware function has to be invoked regardless the response status of a request
    } flags;
} app_middleware_node_t;

typedef int (*app_middleware_fn)(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);


void app_cleanup_middlewares(app_middleware_node_t *head);

void app_run_next_middleware(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node);

app_middleware_node_t *app_gen_middleware_chain(size_t num_args, ...);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__AUTH_H
