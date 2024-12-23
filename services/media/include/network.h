#ifndef MEIDA__NETWORK_H
#define MEIDA__NETWORK_H
#ifdef __cplusplus
extern "C" {
#endif

#include "datatypes.h"

struct addrinfo *resolve_net_addr(int socktype, int protocol, const char *host, uint16_t port);

app_cfg_listener_t *find_existing_listener(app_cfg_listener_t **list, struct addrinfo *addr);

void destroy_network_handle(uv_handle_t *handle, uv_close_cb close_cb);

uv_tcp_t *create_network_handle( uv_loop_t *loop, struct addrinfo *addr,
        uv_connection_cb  cb_on_accept,  unsigned int tfo_q_len);

h2o_socket_t *init_client_tcp_socket(uv_stream_t *server, uv_close_cb on_close);

app_cfg_listener_t *create_new_listener(uv_handle_t *handle);

void free_listener(app_cfg_listener_t *listener);

int atomic_num_connections(app_cfg_t *app_cfg, int delta);

#ifdef __cplusplus
} // end of extern C clause
#endif 
#endif // end of MEIDA__NETWORK_H
