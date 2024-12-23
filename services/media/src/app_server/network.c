#include "network.h"

struct addrinfo *resolve_net_addr(int socktype, int protocol, const char *host, uint16_t port) {
    struct addrinfo hints, *res = NULL;
    int error = 0;
    char port_str[6] = {0,0,0,0,0,0};
    snprintf(&port_str[0], 5, "%hu", port);
    memset(&hints, 0, sizeof(hints));
    // expect to retrieve address chain that contains AF_INET (IPv4) or AF_INET6 (IPv6)
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = socktype;
    hints.ai_protocol = protocol;
    hints.ai_flags = AI_PASSIVE | AI_ADDRCONFIG | AI_NUMERICSERV;
    error = getaddrinfo(host, &port_str[0], &hints, &res);
    if(error) {
        h2o_error_printf("[parsing] failed to resolve address, host:%s, port:%d, reason:%s\n",
               host, port, gai_strerror(error));
    }
    if(res == NULL) {
        h2o_error_printf("[parsing] failed to resolve address: getaddrinfo returned an empty list\n");
    }
    return res;
} // end of resolve_net_addr


app_cfg_listener_t *find_existing_listener(app_cfg_listener_t **list, struct addrinfo *addr)
{
    app_cfg_listener_t *found = NULL;
    struct sockaddr  x;
    size_t x_namelen = sizeof(struct sockaddr); // should specify size of struct sockaddr 
    int chk_port = 1;
    for(int idx = 0; list[idx] != NULL; idx++) {
        struct sockaddr *y = addr->ai_addr;
        memset(&x, 0, sizeof(struct sockaddr));
        uv_tcp_t *nt_handle = (uv_tcp_t *) list[idx]->nt_handle;
        uv_tcp_getsockname(nt_handle, &x, (int *)&x_namelen);
        if(h2o_socket_compare_address(&x, y, chk_port) == 0) {
            found = list[idx];
            break;
        }
    }
    return found;
} // end of find_existing_listener


void destroy_network_handle(uv_handle_t *handle, uv_close_cb close_cb) {
    if (handle) {
        if(handle->data) {
            free(handle->data);
            handle->data = NULL;
        }
        if(!uv_is_closing(handle)) {
            uv_close(handle, close_cb);
        }
    }
}

// create network handle for each listener object in each worker thread, will act as server
uv_tcp_t *create_network_handle( uv_loop_t *loop, struct addrinfo *addr,
        uv_connection_cb  cb_on_accept,  unsigned int tfo_q_len)
{ // TODO, seperate network handle function for UDP protocol
    assert(IPPROTO_TCP == addr->ai_protocol);
    uv_os_fd_t fd = -1; // fetch low-level file descriptor
    int ret = 0;
    char ip4log[INET_ADDRSTRLEN] = {0};
    const struct sockaddr *sock_addr = (const struct sockaddr *) addr->ai_addr;
    uint16_t port_hsb = ntohs( ((struct sockaddr_in *)sock_addr)->sin_port );
    uv_tcp_t *handle = h2o_mem_alloc(sizeof(uv_tcp_t));
    if(!handle) { goto error; }
    unsigned int flgs = (0xff & addr->ai_family);
    handle->data = NULL;
    ret = uv_tcp_init_ex(loop, handle, flgs);
    if(ret != 0) {
        h2o_error_printf("[network] failed to initialize network handle, flags:%x , reason:%s \n",
                flgs, uv_strerror(ret));
        goto error;
    }
    ret = uv_fileno((const uv_handle_t *)handle, &fd);
    if(ret < 0 || fd == -1) {
        h2o_error_printf("[network] failed to get sockfd (port=%u) from created network handle, reason:%s \n",
                port_hsb, uv_strerror(ret));
        goto error;
    }
    int optval = 1;
    ret = setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &optval, sizeof(optval));
    if(ret != 0) {
        h2o_error_printf("[network] failed to reuse port %u upon created network handle, reason:%s \n",
                port_hsb, uv_strerror(ret));
        goto error;
    }
    // try binding the address and port, might fail if the address / port is invalid
    // Note SO_REUSEADDR were already set in uv_tcp_bind() to reuse the same addresses among threads
    ret = uv_tcp_bind(handle, sock_addr, 0);
    if(ret != 0) {
        h2o_error_printf("[network] failed to bind the address (port=%u) when creating network handle, reason:%s \n",
                port_hsb, uv_strerror(ret));
        goto error;
    }
    if(tfo_q_len > 0) {
        ret = setsockopt(fd, addr->ai_protocol, TCP_FASTOPEN, (const void *)&tfo_q_len, sizeof(tfo_q_len));
        if(ret != 0) {
            h2o_error_printf("[network] failed to configure TCP fastopen (port=%u) when creating network handle, reason:%s \n",
                    port_hsb, uv_strerror(ret));
            goto error; 
        }
    }
    int backlog_q_sz = 0x80; // max number of pending connections to queue
    // might fail if the address / port is in use by another process
    ret = uv_listen((uv_stream_t *)handle, backlog_q_sz, cb_on_accept);
    if(ret != 0) {
        h2o_error_printf("[network] failed to listen to the port %u when creating network handle, reason:%s \n",
                port_hsb, uv_strerror(ret));
        goto error;
    }
    return handle;
error:
    inet_ntop(AF_INET, &((struct sockaddr_in *)sock_addr)->sin_addr,
        (void *)&ip4log[0], sizeof(ip4log));
    h2o_error_printf("[network][create-handle] curr-ip-addr:%s \n", &ip4log[0]);
    destroy_network_handle((uv_handle_t *)handle, (uv_close_cb)free);
    return NULL;
} // end of create_network_handle


h2o_socket_t *init_client_tcp_socket(uv_stream_t *server, uv_close_cb on_close) {
    int ret = 0;
    uv_tcp_t     *client_conn = NULL;
    client_conn = (uv_tcp_t *)h2o_mem_alloc(sizeof(uv_tcp_t));
    client_conn->data = NULL;
    ret = uv_tcp_init(server->loop, client_conn);
    if(ret != 0) {
        h2o_error_printf("[network] server failed to initialize client connection, reason:%s \n",
                 uv_strerror(ret));
        goto error;
    }
    ret = uv_accept(server, (uv_stream_t *)client_conn);
    if(ret != 0) {
        h2o_error_printf("[network] server failed to accept client connection, reason:%s \n",
                 uv_strerror(ret));
        goto error;
    }
    return  h2o_uv_socket_create((uv_handle_t *)client_conn, on_close);
error:
    destroy_network_handle((uv_handle_t *)client_conn, (uv_close_cb)free);
    return NULL;
} // end of init_client_tcp_socket


app_cfg_listener_t *create_new_listener(uv_handle_t *handle) {
    app_cfg_listener_t *listener = h2o_mem_alloc(sizeof(app_cfg_listener_t));
    memset(listener, 0, sizeof(app_cfg_listener_t));
    listener->nt_handle = handle;
    return listener;
}

void free_listener(app_cfg_listener_t *listener) {
    if(listener) {
        if(listener->security.ctx) {
            SSL_CTX_free(listener->security.ctx);
        }
        memset(listener, 0, sizeof(app_cfg_listener_t));
        free((void *)listener);
    }
}

int atomic_num_connections(app_cfg_t *app_cfg, int delta)
{
    int prev = 0;
    if(delta == 0) {
        prev = app_cfg->state.num_curr_connections;
    } else {
        prev = atomic_fetch_add_explicit( &app_cfg->state.num_curr_connections,
                delta, memory_order_acq_rel);
    }
    return prev;
}

