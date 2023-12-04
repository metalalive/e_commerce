#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <string.h>
#include <libelf.h>
#include <h2o.h>
#include <h2o/serverutil.h>

#include "datatypes.h"
#include "routes.h"

typedef struct {
    h2o_hostconf_t *host;
    json_t   *routes_cfg;
} app_apiview_cb_arg_t;

typedef int (*restapi_endpoint_handle_fn)(RESTAPI_HANDLER_ARGS(self, req));


static int _app_elf_parse_from_symbol_table(Elf *elf, app_elf_fn_traverse_cb cb, void *cb_args)
{
    Elf64_Ehdr *ehdr = NULL;
    Elf64_Shdr *shdr = NULL;
    Elf_Scn    *section = NULL;
    Elf_Data   *data = NULL;
    // check ELF executable header
    if((ehdr = elf64_getehdr(elf)) == NULL) {
        h2o_error_printf("[ELF parsing] data corruption on ELF executable header \n");
        goto error;
    }
    // point to section of ELF executable header
    if((section = elf_getscn(elf, ehdr->e_shstrndx)) == NULL) {
        h2o_error_printf("[ELF parsing] section not found ELF executable header \n");
        goto error;
    }
    if((data = elf_getdata(section, NULL)) == NULL) {
        h2o_error_printf("[ELF parsing] first data chunk not found in the section of ELF executable header \n");
        goto error;
    }
    section = NULL;
    while((section = elf_nextscn(elf, section)) != NULL) {
        if((shdr = elf64_getshdr(section)) == NULL) {
            h2o_error_printf("[ELF parsing] section header not found \n");
            goto error;
        }
        if(shdr->sh_type != SHT_SYMTAB) {
            continue;
        }
        data = NULL;
        while((data = elf_getdata(section ,data)) != NULL) {
            if(data->d_size == 0) {
                continue;
            }
            Elf64_Sym *esym  = NULL;
            Elf64_Sym *data_end  = (Elf64_Sym *) ((char *)data->d_buf + data->d_size);
            for(esym = (Elf64_Sym *)data->d_buf; esym < data_end; esym++) {
                if((esym->st_value == 0) || (ELF64_ST_BIND(esym->st_info) == STB_WEAK)
                        || (ELF64_ST_BIND(esym->st_info) == STB_NUM)
                        || (ELF64_ST_TYPE(esym->st_info) != STT_FUNC))  {
                    continue;
                }
                char *fn_name = elf_strptr(elf, (size_t)shdr->sh_link, (size_t)esym->st_name);
                if(fn_name) {
                    uint8_t immediate_stop = cb(fn_name, (void *)esym->st_value, cb_args);
                    if(immediate_stop) {
                        goto done;
                    }
                } else {
                    h2o_error_printf("[ELF parsing] function name not found in symbol table \n");
                    goto error;
                }
            } // end of inner for-loop
            assert(esym == data_end);
        } // end of data chunk iteration for each section
    } // end of sections iteration
done:
    return 0;
error:
    return EX_CONFIG;
} // end of _app_elf_parse_from_symbol_table


static int default_handler_method_not_allowed(RESTAPI_HANDLER_ARGS(self, req))
{
    h2o_generator_t generator = {NULL, NULL};
    h2o_iovec_t body = {.base = NULL, .len = 0};
    size_t bufcnt = 0;
    req->res.status = 405;
    req->res.reason = "method not allowed";
    h2o_start_response(req, &generator);
    h2o_send(req, &body, bufcnt, H2O_SEND_STATE_FINAL);
    return 0;
}


static void setup_default_handlers(h2o_hostconf_t *host)
{
    // register default handler function to all pathconf objects, and append it to the end of
    // list of handler function pointers, for responding status 405 "method not allowed"
    int idx = 0;
    for(idx = 0; idx < host->paths.size; idx++) {
        h2o_pathconf_t *pathcfg = host->paths.entries[idx];
        h2o_handler_t *handler = NULL;
        size_t num_handlers = pathcfg->handlers.size;
        if(num_handlers > 0) {
            handler = pathcfg->handlers.entries[num_handlers - 1];
        }
        if(!handler || handler->on_req != default_handler_method_not_allowed) {
            handler = h2o_create_handler(pathcfg, sizeof(h2o_handler_t));
            handler->on_req = default_handler_method_not_allowed;
        }
    }
} // end of setup_default_handlers()


int app_elf_traverse_functions(const char *exe_path, app_elf_fn_traverse_cb cb, void *cb_args)
{
    int exe_fd = -1;
    Elf *elf = NULL;
    if(elf_version(EV_CURRENT) == EV_NONE) {
        h2o_error_printf("[routing] setup error, failure on libelf version\n");
        goto error;
    }
    if((exe_fd = open(exe_path, O_RDONLY)) == -1) {
        h2o_error_printf("[routing] failed to load executable object for symbol table check \n");
        goto error;
    }
    if((elf = elf_begin(exe_fd, ELF_C_READ, NULL)) == NULL) {
        h2o_error_printf("[routing] the executable object is NOT ELF binary file \n");
        goto error;
    }
    if(_app_elf_parse_from_symbol_table(elf, cb, cb_args) != 0) {
        goto error;
    }
    elf_end(elf);
    close(exe_fd);
    return 0;
error:
    if(elf) {
        elf_end(elf);
        elf = NULL;
    }
    if(exe_fd != -1) {
        close(exe_fd);
    }
    return EX_CONFIG;
} // end of app_elf_traverse_functions


static const char *_app_get_restapi_endpoint_path(json_t *routes_cfg, const char *fn_name_x) {
    const char *out = NULL;
    json_t *route_cfg = NULL;
    int idx = 0;
    json_array_foreach(routes_cfg, idx, route_cfg) {
        if (!json_is_object(route_cfg)) {
            continue;
        }
        const char *fn_name_y = json_string_value(json_object_get(route_cfg, "entry_fn"));
        const char *path = json_string_value(json_object_get(route_cfg, "path"));
        if((strcmp(fn_name_x, fn_name_y) == 0) && (path != NULL)) {
            out = path;
            break;
        }
    } // end of json-array iteration
    return out;
} // end of _app_get_restapi_endpoint_path


static h2o_pathconf_t *_app_find_h2o_pathconf(h2o_hostconf_t *host, const char *keyword_path) {
    h2o_pathconf_t *found = NULL;
    int idx = 0;
    for(idx = 0; idx < host->paths.size; idx++) {
        h2o_pathconf_t *curr = host->paths.entries[idx];
        // both of the paths have to be compared as NULL-terminated strings
        // , shouldn't be compared using strncmp()
        if(strcmp(keyword_path, (const char *)curr->path.base) == 0) {
            found = curr;
            break;
        }
    }
    return found;
}

static void _app_register_route_handler(h2o_hostconf_t *host, const char *path, restapi_endpoint_handle_fn  fp)
{
    h2o_pathconf_t *pathcfg = _app_find_h2o_pathconf(host, path);
    if(!pathcfg) {
        pathcfg = h2o_config_register_path(host, path, 0);
    }
    h2o_handler_t *handler = h2o_create_handler(pathcfg, sizeof(h2o_handler_t));
    handler->on_req = fp;
} // end of _app_register_route_handler

static uint8_t _app_elf_traverse_apiviews_cb(char *fn_name, void *entry_point, void *cb_args)
{
    h2o_hostconf_t *host = ((app_apiview_cb_arg_t *)cb_args)->host;
    json_t   *routes_cfg = ((app_apiview_cb_arg_t *)cb_args)->routes_cfg;
    const char *path = NULL;
    if((path = _app_get_restapi_endpoint_path(routes_cfg, fn_name)) != NULL) {
        _app_register_route_handler(host, path, (restapi_endpoint_handle_fn) entry_point);
    }
    return 0; // keep parsing rest of API endpoint functions
} // end of _app_elf_traverse_apiviews_cb


static int _compare_from_longer_paths(const void *x, const void *y) {
    h2o_pathconf_t *pathcfg_x = *(h2o_pathconf_t **)x;
    h2o_pathconf_t *pathcfg_y = *(h2o_pathconf_t **)y;
    // sorted path in descending order (longest path first)
    int ret = pathcfg_y->path.len - pathcfg_x->path.len;
    if(ret == 0) {
        ret = strcmp(pathcfg_x->path.base, pathcfg_y->path.base);
    }
    return ret;
}

int app_setup_apiview_routes(h2o_hostconf_t *host, json_t *routes_cfg, const char *exe_path) {
    int num_routes = 0;
    if(!host || !exe_path) {
        goto error;
    }
    if(routes_cfg) {
        if(!json_is_array(routes_cfg)) {
            h2o_error_printf("[parsing] setup error, routes_cfg should be a list of json objects \n");
            goto error;
        }
        num_routes = (int)json_array_size(routes_cfg);
        if(num_routes < 0) {
            h2o_error_printf("[parsing] setup error, num_routes should be positive integer , but got %d \n", num_routes);
            goto error;
        } else if(num_routes == 0) {
            return 0;
        }
    } else { // skip , no paths is created along with the host conf
        return 0;
    }
    app_apiview_cb_arg_t cb_args = {.host = host, .routes_cfg=routes_cfg };
    if(app_elf_traverse_functions(exe_path, _app_elf_traverse_apiviews_cb, (void *)&cb_args) != 0) {
        goto error;
    }
    // https://h2o.examp1e.net/configure/base_directives.html#paths
    // When more than one matching paths were found, h2o chooses the longest path,
    // internally this is achieved by sorting all the pathconf objects with the path as
    // a key, when libh2o performs path lookup in setup_pathconf(...) it always chooses
    // the first matching path since libh2o expects that the list of pathconf objects in
    // corresponding hostconf object were already sorted before server started.
    qsort((void *)&host->paths.entries[0], host->paths.size, sizeof(h2o_pathconf_t *),
            _compare_from_longer_paths);
    setup_default_handlers(host);
    return 0;
error:
    return EX_CONFIG;
} // end of app_setup_apiview_routes
