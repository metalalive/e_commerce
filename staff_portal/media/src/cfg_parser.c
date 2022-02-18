#include <sys/resource.h>
#include <h2o.h>
#include <h2o/serverutil.h>

#include "cfg_parser.h"
#include "network.h"
#include "routes.h"

// side effect of this function is that it left pid file opened, be sure to close the file on program exit
int parse_cfg_pid_file(json_t *obj, app_cfg_t *_app_cfg) {
    FILE *_file = NULL;
    if (json_is_string(obj)) {
        const char *pid_file_str = json_string_value(obj);
        _file = fopen(pid_file_str, "w+");
        if (_file) {
            fprintf(_file, "%d\n", (int)getpid());
            fflush(_file);
            _app_cfg->pid_file = _file;
        } else {
            // TODO check errno from <errno.h>
        }
    }
    return (_file ? 0: EX_CONFIG);
} // TODO, remove pid file on program exit


static int parse_cfg_error_log(json_t *obj, app_cfg_t *_app_cfg) {
    int fd = -1;
    if (json_is_string(obj)) {
        const char *err_log_path = json_string_value(obj);
        fd = h2o_access_log_open_log(err_log_path);
        if (fd != -1) { // redirect stdout and stderr to error log
            int fd_stdout = 1;
            int fd_stderr = 2;
            if (dup2(fd, fd_stdout) == -1 || dup2(fd, fd_stderr) == -1) {
                close(fd);
                fd = -1;
            } else {
                _app_cfg->error_log_fd = fd;
            } // TODO, close error log fd later at some point
        }
    }
    return ((fd != -1) ? 0: EX_CONFIG);
}


static int parse_cfg_acs_log(json_t *obj, app_cfg_t *_app_cfg) {
    int err = EX_CONFIG;
    if (json_is_object(obj)) {
        json_t *path_obj   = json_object_get((const json_t *)obj, "path");
        json_t *format_obj = json_object_get((const json_t *)obj, "format");
        const char *path   = json_string_value(path_obj);
        const char *format = json_string_value(format_obj);
        if (path) {
            _app_cfg->access_logger = h2o_access_log_open_handle(path, format, H2O_LOGCONF_ESCAPE_JSON);
            err = 0;
        }
    } // end of optional parameter for access logger
    return err;
}


int parse_cfg_max_conns(json_t *obj, app_cfg_t *_app_cfg) {
    if (!json_is_integer(obj)) {
        goto error;
    }
    json_int_t max_conns_val = json_integer_value(obj);
    if (max_conns_val <= 0) {
        h2o_error_printf("[parsing] max_connections has to be positive integer \n");
        goto error;
    }
    struct rlimit curr_setting = {.rlim_cur=0 , .rlim_max=0};
    if(getrlimit(RLIMIT_NOFILE, &curr_setting) != 0) {
        h2o_error_printf("[parsing] failed to run getrlimit() \n");
        goto error;
    }
    if(max_conns_val > curr_setting.rlim_max) {
        h2o_error_printf("[parsing] rate-limit setting error, config parameter: %lld, must not be greater than default value set by OS kernel: %lu \n",
                max_conns_val, curr_setting.rlim_max);
        goto error;
    }  // MUST NOT exceeds default value set by OS kernel
    curr_setting.rlim_cur = (rlim_t)max_conns_val;
    if(setrlimit(RLIMIT_NOFILE, &curr_setting) != 0) {
        h2o_error_printf("[parsing] failed to run setrlimit() \n");
        goto error;
    }
    _app_cfg->max_connections = (unsigned int) max_conns_val;
    return 0;
error:
    return EX_CONFIG;
} // end of parse_cfg_max_conns


static int parse_cfg_limit_req_body(json_t *obj, app_cfg_t *_app_cfg) {
    int err = EX_CONFIG;
    json_int_t  value = json_integer_value(obj);
    if (value > 0) {
        _app_cfg->server_glb_cfg.max_request_entity_size = (size_t)value;
        err = 0;
    }
    return err;
}


static int parse_cfg_num_workers(json_t *obj, app_cfg_t *_app_cfg) {
    // In this application, number of worker threads excludes the main thread
    int new_capacity = (int) json_integer_value(obj);
    if (new_capacity < 0) {
        goto error;
    }
    // TODO, free some of memory if new capacity is smaller than current one
    h2o_vector_reserve(NULL, &_app_cfg->workers, (size_t)new_capacity);
    // preserve space first, update thread ID later
    _app_cfg->workers.size = new_capacity;
    return 0;
error:
    return EX_CONFIG;
}

static int parse_cfg_tfo_q_len(json_t *obj, app_cfg_t *_app_cfg) {
    int err = EX_CONFIG;
    json_int_t  value = json_integer_value(obj);
    if (value > 0) {
        _app_cfg->tfo_q_len = (unsigned int)value;
        err = 0;
    }
    return err;
}


static int parse_cfg_tmp_buf(json_t *obj, app_cfg_t *_app_cfg) {
    if (!json_is_object(obj)) { goto error; }
    json_t *path_obj = json_object_get((const json_t *)obj, "path");
    json_t *threshold_obj = json_object_get((const json_t *)obj, "threshold_in_bytes");
    const char *path = json_string_value(path_obj);
    int  threshold = (int) json_integer_value(threshold_obj);
    if (!path || threshold <= 0) {
        h2o_error_printf("[parsing] invalid tmp_buf settings, path: %s , threshold: %d bytes\n",
                path, threshold);
        goto error;
    }
    // access check to the path
    if(access(path, F_OK | R_OK | W_OK) != 0) {
        h2o_error_printf("[parsing] not all requested permissions granted, path: %s\n", path);
        goto error;
    }
    _app_cfg->tmp_buf.threshold_bytes = (unsigned int)threshold;
    _app_cfg->tmp_buf.path = strdup(path);
    return 0;
error:
    return EX_CONFIG;
} // end of parse_cfg_tmp_buf


static int parse_cfg_auth_keystore(json_t *obj, app_cfg_t *app_cfg) {
    const char *url = json_string_value(obj);
    if(!url) {
        goto error;
    }
    if(!app_cfg->jwks.handle) {
        r_jwks_init(&app_cfg->jwks.handle);
        assert(app_cfg->jwks.handle);
    }
    if(r_jwks_import_from_uri(app_cfg->jwks.handle, url, 0) != RHN_OK)
    {
        h2o_error_printf("[parsing] failed to preload JWKS from given URI: %s \n", url);
        goto error;
    }
    if(r_jwks_is_valid(app_cfg->jwks.handle) != RHN_OK)
    {
        h2o_error_printf("[parsing] failed to decode to JWKS format, URI: %s \n", url);
        goto error;
    }
    app_cfg->jwks.src_url = strdup(url);
    time(&app_cfg->jwks.last_update);
    return 0;
error:
    return EX_CONFIG;
} // end of parse_cfg_auth_keystore


int parse_cfg_listener_ssl(struct app_cfg_security_t *security, const json_t *obj)
{
    SSL_CTX *ssl_ctx = NULL;
    if(!json_is_object(obj)) {
        goto error;
    }
    json_t *cert_file_obj    = json_object_get(obj, "cert_file");
    json_t *privkey_file_obj = json_object_get(obj, "privkey_file");
    json_t *min_ver_obj      = json_object_get(obj, "min_version");
    json_t *cipher_suites_obj = json_object_get(obj, "cipher_suites");
    const char *cert_file_path    = json_string_value(cert_file_obj   );
    const char *privkey_file_path = json_string_value(privkey_file_obj);
    json_int_t  min_version       = json_integer_value(min_ver_obj     );
    const char *ciphersuite_labels = json_string_value(cipher_suites_obj);
    if(!cert_file_path || !privkey_file_path || !ciphersuite_labels)
    {
        goto error;
    }
    assert((0x8000 & min_version) == 0); // currently not support DTLS (TODO)
    if(min_version < TLS1_3_VERSION) {
        h2o_error_printf("[parsing] currently this server only supports TLS v1.3 and successive versions, given : 0x%llx \n", min_version);
        goto error;
    }
    ssl_ctx = SSL_CTX_new(TLS_server_method());
    long disabled_ssl_versions = SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 | SSL_OP_NO_TLSv1
        | SSL_OP_NO_TLSv1_1 | SSL_OP_NO_TLSv1_2;
    long ssl_options = SSL_OP_ALL | SSL_OP_CIPHER_SERVER_PREFERENCE | SSL_OP_NO_COMPRESSION
        | SSL_OP_NO_RENEGOTIATION | disabled_ssl_versions;
    SSL_CTX_set_options(ssl_ctx, ssl_options);
    if(SSL_CTX_set_min_proto_version(ssl_ctx, min_version) != 1) {
        goto error;
    }
    SSL_CTX_set_session_id_context(ssl_ctx, (const unsigned char *)APP_LABEL, (unsigned int)APP_LABEL_LEN);
    if (SSL_CTX_use_PrivateKey_file(ssl_ctx, privkey_file_path, SSL_FILETYPE_PEM) != 1) {
        h2o_error_printf("[parsing] failed to load private key for server certificate : %s\n", privkey_file_path);
        goto error;
    }
    if (SSL_CTX_use_certificate_chain_file(ssl_ctx, cert_file_path) != 1) {
        h2o_error_printf("[parsing] failed to load server certificate file : %s\n", cert_file_path);
        goto error;
    }
    X509 *x509 = SSL_CTX_get0_certificate(ssl_ctx);
    if(X509_cmp_current_time(X509_get0_notAfter(x509)) == -1) {
        h2o_error_printf("[parsing] server certificate expired : %s\n", cert_file_path);
        goto error;
    } // TODO, examine Common Name (CN) and Subject Alternative Name (SAN)
    if(SSL_CTX_set_ciphersuites(ssl_ctx, ciphersuite_labels) != 1) {
        h2o_error_printf("[parsing] failed to set cipher suites, the given value : %s\n", ciphersuite_labels);
        goto error;
    }
#ifdef H2O_USE_ALPN
    // some clients may drop NPN support (e.g. google chrome) since it usually works with deprecated SPDY
    h2o_ssl_register_alpn_protocols(ssl_ctx, h2o_alpn_protocols);
#endif // end of H2O_USE_ALPN
    security->ctx = ssl_ctx;
    return 0;
error:
    if(ssl_ctx) {
        SSL_CTX_free(ssl_ctx);
    }
    return EX_CONFIG;
} // end of parse_cfg_listener_ssl


static void _dummy_cb_on_nt_accept(uv_stream_t *server, int status)
{
    // this callback is used only for testing network configuration when
    //  creating network handles, it won't be used in dev / production server
    assert(0);
}

static int maybe_create_new_listener(const char *host, uint16_t port, json_t *ssl_obj,
        json_t *routes_cfg, app_cfg_t *_app_cfg)
{ // TODO, currently only support TCP handle, would support UDP in future
    struct addrinfo *curr_addr = NULL, *res_addr = NULL;
    if(!host || port <= 0) {
        goto error;
    }
    res_addr = resolve_net_addr(SOCK_STREAM, IPPROTO_TCP, host, (uint16_t)port);
    if(!res_addr) { goto error; }
    for (curr_addr = res_addr; curr_addr != NULL; curr_addr = curr_addr->ai_next) {
        app_cfg_listener_t *found = find_existing_listener(_app_cfg->listeners, curr_addr);
        if(found) { continue; }
        // the default loop works with the 1st. thread of this application
        // (main thread in master mode, the 1st. worker thread in daemon mode)
        uv_handle_t *handle = (uv_handle_t *)create_network_handle(uv_default_loop(), curr_addr,
                 _dummy_cb_on_nt_accept, _app_cfg->tfo_q_len);
        if(!handle) { goto error; }
        app_cfg_listener_t *_new = create_new_listener(handle);
        if(parse_cfg_listener_ssl(&_new->security, (const json_t *)ssl_obj) != 0) {
            destroy_network_handle(handle, (uv_close_cb)free);
            free_listener(_new);
            goto error;
        }
        h2o_hostconf_t *hostcfg = h2o_config_register_host(
                &_app_cfg->server_glb_cfg,
                h2o_iovec_init(host, strlen(host)),
                port
            );
        if(setup_routes(hostcfg, routes_cfg, _app_cfg->exe_path) != 0) {
            goto error;
        } // preserve some network attributes which are NOT stored in `struct sockaddr`
        uv_nt_handle_data *nt_attr = h2o_mem_alloc(sizeof(uv_nt_handle_data));
        *nt_attr = (uv_nt_handle_data){
            .ai_flags = curr_addr->ai_flags,         .ai_family = curr_addr->ai_family,
            .ai_socktype = curr_addr->ai_socktype,   .ai_protocol = curr_addr->ai_protocol
        };
        handle->data = (void *)nt_attr;
        _new->hostconf = hostcfg;
        h2o_append_to_null_terminated_list((void ***)&_app_cfg->listeners, (void *)_new);
        _app_cfg->num_listeners += 1;
    } // end of address iteration
    freeaddrinfo(res_addr);
    return 0;
error:
    if(!res_addr) {
        freeaddrinfo(res_addr);
    }
    return EX_CONFIG;
} // end of maybe_create_new_listener


static int parse_cfg_listeners(const json_t *objs, app_cfg_t *_app_cfg) {
    if (!json_is_array(objs)) {
        goto error;
    }
    const json_t *obj = NULL;
    int num_objs = (int)json_array_size(objs);
    int idx = 0;
    if(!_app_cfg->listeners) {
        _app_cfg->listeners = h2o_mem_alloc(sizeof(app_cfg_listener_t**));
        _app_cfg->listeners[0] = NULL;
    }
    json_array_foreach(objs, idx, obj) {
        if (!json_is_object(obj)) {
            break;
        }
        json_t *port_obj = json_object_get(obj, "port");
        json_t *host_obj = json_object_get(obj, "host");
        json_t *ssl_obj  = json_object_get(obj, "ssl");
        json_t *routes_obj  = json_object_get(obj, "routes");
        const char *host = json_string_value(host_obj);
        uint16_t  port = (uint16_t) json_integer_value(port_obj);
        if(maybe_create_new_listener(host, port, ssl_obj, routes_obj, _app_cfg) != 0) {
            break;
        }
    } // end of iteration
    if(num_objs > idx) {
        goto error;
    }
    return 0;
error:
    return EX_CONFIG;
} // end of parse_cfg_listeners()


int parse_cfg_params(const char *cfg_file_path, app_cfg_t *_app_cfg)
{
    int result_error = 0;
    json_error_t jerror;
    json_t  *root = NULL;
    root = json_load_file(cfg_file_path, (size_t)0, &jerror);
    if (!json_is_object(root)) {
        h2o_error_printf("[parsing] decode error on JSON file %s at line %d, column %d\n",
               &jerror.source[0], jerror.line, jerror.column);
        goto error;
    }
    result_error = parse_cfg_pid_file(json_object_get((const json_t *)root, "pid_file"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_error_log(json_object_get((const json_t *)root, "error_log"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_acs_log(json_object_get((const json_t *)root, "access_log"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_max_conns(json_object_get((const json_t *)root, "max_connections"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_limit_req_body(json_object_get((const json_t *)root, "limit_req_body_in_bytes"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_num_workers(json_object_get((const json_t *)root, "num_workers"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_tfo_q_len(json_object_get((const json_t *)root, "tcp_fastopen_queue_size"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_listeners(json_object_get((const json_t *)root, "listen"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_tmp_buf(json_object_get((const json_t *)root, "tmp_buf"), _app_cfg);
    if (result_error) {  goto error; }
    result_error = parse_cfg_auth_keystore(json_object_get((const json_t *)root, "auth_keystore"), _app_cfg);
    if (result_error) {  goto error; }
    json_decref(root);
    return 0;
error:
    if (!root) {
        json_decref(root);
    }
    return EX_CONFIG;
} // end of parse_cfg_params()
