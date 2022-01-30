#ifndef MEIDA__ROUTES_H
#define MEIDA__ROUTES_H
#ifdef __cplusplus
extern "C" {
#endif

#include <h2o.h>
#include <jansson.h>

#define RESTAPI_HANDLER_ARGS(hdlr_var, req_var)    h2o_handler_t *hdlr_var, h2o_req_t *req_var

// for any request whose method does NOT match the parameter `http_method`,  the handler function
// actually returns non-zero integer which means it passed the request to next handler function
// (if exists)
#define RESTAPI_ENDPOINT_HANDLER(func_name, http_method, hdlr_var, req_var) \
    static int METHOD_HANDLER_##func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var)); \
    \
    static __attribute__((optimize("O0"))) int func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var)) \
    { \
        h2o_iovec_t *expect = &(req_var)->input.method; \
        int ret = strncmp((const char *)(#http_method), expect->base, expect->len); \
        if(ret == 0) {  \
            return METHOD_HANDLER_##func_name(hdlr_var, req_var);   \
        } else { \
            return -1;   \
        } \
    } \
    static int METHOD_HANDLER_##func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var))

typedef int (*restapi_endpoint_handle_fn)(RESTAPI_HANDLER_ARGS(self, req));

int setup_routes(h2o_hostconf_t *host, json_t *routes_cfg, const char *exe_path);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__ROUTES_H
