#include "utils.h"
#include "views.h"
#include "views/filefetch_common.h"

RESTAPI_ENDPOINT_HANDLER(initiate_single_file, GET, hdlr, req)
{
#if 0
    json_t *err_info = app_fetch_from_hashmap(node->data, "err_info");
    json_t *spec  = app_fetch_from_hashmap(node->data, "qparams");
    uint32_t  last_upld_seq = (uint32_t) json_integer_value(json_object_get(spec, "last_upld_req"));
    uint32_t  res_owner_id  = (uint32_t) json_integer_value(json_object_get(spec, "resource_owner_id"));
    const char *_res_id_encoded = json_string_value(json_object_get(spec, "res_id_encoded"));
#endif
    return 0;
} // end of  initiate_single_file
