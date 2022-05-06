#include <amqp.h>
#include <amqp_tcp_socket.h>
#include "rpc/core.h"

void * app_rpc_conn_init(app_cfg_t *app_cfg) {
    amqp_connection_state_t *conn = NULL;
    return (void *)conn;
} // end of app_rpc_conn_init


void app_rpc_conn_deinit(void *conn) {
} // end of app_rpc_conn_deinit

ARPC_STATUS_CODE app_rpc_start(arpc_exe_arg_t *cfg)
{
    if(!cfg || !cfg->job_id || !cfg->msg_body.bytes || !cfg->routing_key) {
        return APPRPC_RESP_ARG_ERROR;
    }
    memcpy(cfg->job_id, "aC1o3k", 6);
    ARPC_STATUS_CODE status = APPRPC_RESP_ACCEPTED;
    return status;
}

ARPC_STATUS_CODE app_rpc_publish(arpc_exe_arg_t *cfg)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_ACCEPTED;
    if(0) {
    }
    // int err = amqp_basic_publish(amqp_connection_state_t state, amqp_channel_t channel,
    //                        amqp_bytes_t exchange, amqp_bytes_t routing_key,
    //                        amqp_boolean_t mandatory, amqp_boolean_t immediate,
    //                        amqp_basic_properties_t const *properties,
    //                        amqp_bytes_t body);
    
    return status;
} // end of app_rpc_publish

ARPC_STATUS_CODE app_rpc_subscribe(arpc_exe_arg_t *cfg)
{
    ARPC_STATUS_CODE status = APPRPC_RESP_ACCEPTED;
    return status;
} // end of app_rpc_subscribe
