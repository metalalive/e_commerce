#include "transcoder/image/common.h"

void  atfp_image__dst_update_metadata(atfp_t *processor, void *loop)
{
    json_t *err_info = processor->data.error;
    json_object_set_new(err_info, "dev", json_string("implementation not finished"));
    //processor -> data.callback(processor);
}
