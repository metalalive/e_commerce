#include <stdarg.h>
#include "middleware.h"

void app_run_next_middleware(RESTAPI_HANDLER_ARGS(self, req), app_middleware_node_t *node)
{
    if(!self || !req || !node) {
        h2o_error_printf("[middleware] argument missing \n");
    } else if(node->next) {
        node = node->next;
        if((req->res.status == 0) || node->flags.mandatory) {
            node->fn(self, req, node);
        } else { // skip
            app_run_next_middleware(self, req, node);
        }
    } else if(!node->next && node->data) { // reach the end of middleware chain
        ENTRY  e = {.key = "middleware_chain_head", .data = NULL};
        ENTRY *e_ret = NULL;
        hsearch_r(e, FIND, &e_ret, node->data);
        if(e_ret && e_ret->data) {
            app_middleware_node_t *head = (app_middleware_node_t *)e_ret->data;
            app_cleanup_middlewares(head);
            // must NOT access the middleware chain after de-initialization
        }
    }
} // end of app_run_next_middleware


app_middleware_node_t *app_gen_middleware_chain(size_t num_args, ...)
{
    // return a list of nodes for running middleware and pointer to hash map used
    //  to store data which can be accessed among the given middlewares.
    int result = 0;
    app_middleware_node_t *prev_node = NULL;
    app_middleware_node_t *curr_node = NULL;
    size_t  num_hdlrs  = num_args >> 1;
    va_list ap;
    size_t chunk_sz = sizeof(app_middleware_node_t) * num_hdlrs + sizeof(struct hsearch_data);
    app_middleware_node_t *nodes = calloc(chunk_sz, sizeof(char));
    struct hsearch_data   *htab  = (struct hsearch_data *)&nodes[num_hdlrs];
    va_start(ap, num_args);
    for(size_t idx = 0; idx < num_hdlrs; idx++) {
        curr_node = &nodes[idx];
        curr_node->data = htab;
        curr_node->fn   = va_arg(ap, app_middleware_fn);
        int _mandatory = va_arg(ap, int);
        curr_node->flags.mandatory = (uint8_t)_mandatory;
        if(prev_node)
            prev_node->next = curr_node;
        prev_node = curr_node;
    }
    va_end(ap);
    result = hcreate_r(1 + NUM_ENTRIES_APP_HASHMAP, htab);
    if(result == 0) {
        app_cleanup_middlewares(&nodes[0]);
        return  NULL;
    } else {
        ENTRY  e = {.key = "middleware_chain_head", .data = (void*)&nodes[0] };
        ENTRY *e_ret = NULL;
        hsearch_r(e, ENTER, &e_ret, htab);
        return &nodes[0];
    }
} // end of app_gen_middleware_chain


void app_cleanup_middlewares(app_middleware_node_t *head)
{
    if(head) {
        hdestroy_r(head->data);
        free(head);
    }
} // end of app_cleanup_middlewares
