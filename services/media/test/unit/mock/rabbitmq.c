#include <string.h>
#include <amqp_tcp_socket.h>
#include <cgreen/mocks.h>

const amqp_bytes_t amqp_empty_bytes = {0, NULL};
const amqp_table_t amqp_empty_table = {0, NULL};

amqp_bytes_t amqp_cstring_bytes(char const *cstr) {
    amqp_bytes_t result;
    result.len = strlen(cstr);
    result.bytes = (void *)cstr;
    return result;
}

const char *amqp_error_string2(int code) { return (char *)mock(code); }

amqp_socket_t *amqp_tcp_socket_new(amqp_connection_state_t conn_state) {
    return (amqp_socket_t *)mock(conn_state);
}

amqp_connection_state_t amqp_new_connection(void) { return (amqp_connection_state_t)mock(); }

amqp_rpc_reply_t amqp_connection_close(amqp_connection_state_t conn_state, int code) {
    mock(conn_state, code);
    return (amqp_rpc_reply_t){0};
}

int amqp_destroy_connection(amqp_connection_state_t conn_state) { return (int)mock(conn_state); }

int amqp_socket_open(amqp_socket_t *self, const char *host, int port) { return (int)mock(self, host, port); }

amqp_rpc_reply_t amqp_login(
    amqp_connection_state_t conn_state, char const *vhost, int channel_max, int frame_max, int heartbeat,
    int sasl_method, ...
) {
    amqp_rpc_reply_t *out = (amqp_rpc_reply_t *)mock(conn_state, vhost, channel_max, frame_max, heartbeat);
    return *out;
}

amqp_channel_open_ok_t *amqp_channel_open(amqp_connection_state_t conn_state, amqp_channel_t channel) {
    return (amqp_channel_open_ok_t *)mock(conn_state, channel);
}

amqp_rpc_reply_t amqp_channel_close(amqp_connection_state_t conn_state, amqp_channel_t channel, int code) {
    amqp_rpc_reply_t reply = {0};
    mock(conn_state, channel, code);
    return reply;
}

amqp_queue_declare_ok_t *amqp_queue_declare(
    amqp_connection_state_t conn_state, amqp_channel_t channel, amqp_bytes_t queue, amqp_boolean_t passive,
    amqp_boolean_t durable, amqp_boolean_t exclusive, amqp_boolean_t auto_delete, amqp_table_t arguments
) {
    char *q_name = queue.bytes;
    return (amqp_queue_declare_ok_t *)mock(
        conn_state, channel, q_name, passive, durable, exclusive, auto_delete
    );
}

amqp_rpc_reply_t amqp_get_rpc_reply(amqp_connection_state_t conn_state) {
    amqp_rpc_reply_t *out = (amqp_rpc_reply_t *)mock(conn_state);
    return *out;
}

amqp_queue_bind_ok_t *AMQP_CALL amqp_queue_bind(
    amqp_connection_state_t conn_state, amqp_channel_t channel, amqp_bytes_t queue, amqp_bytes_t exchange,
    amqp_bytes_t routing_key, amqp_table_t arguments
) {
    char *queue_name = queue.bytes;
    char *exchange_name = exchange.bytes;
    char *route_key_name = routing_key.bytes;
    return (amqp_queue_bind_ok_t *)mock(conn_state, channel, queue_name, exchange_name, route_key_name);
}

int amqp_basic_publish(
    amqp_connection_state_t conn_state, amqp_channel_t channel, amqp_bytes_t exchange,
    amqp_bytes_t routing_key, amqp_boolean_t mandatory, amqp_boolean_t immediate,
    amqp_basic_properties_t const *properties, amqp_bytes_t body
) {
    char               *exchange_name = exchange.bytes;
    char               *route_key_name = routing_key.bytes;
    char               *raw_body = body.bytes;
    size_t              num_hdr_entries = properties->headers.num_entries;
    amqp_table_entry_t *hdr_entries = properties->headers.entries;
    int                 ret =
        (int)mock(conn_state, channel, exchange_name, route_key_name, raw_body, num_hdr_entries, hdr_entries);
    for (int idx = 0; idx < num_hdr_entries; idx++) {
        amqp_table_entry_t *hdr_entry = &hdr_entries[idx];
        const char         *rd_key = hdr_entry->key.bytes;
        const char         *rd_val = hdr_entry->value.value.bytes.bytes;
        mock(rd_key, rd_val);
    }
    return ret;
} // end of  amqp_basic_publish

amqp_basic_consume_ok_t *amqp_basic_consume(
    amqp_connection_state_t conn_state, amqp_channel_t channel, amqp_bytes_t queue, amqp_bytes_t consumer_tag,
    amqp_boolean_t no_local, amqp_boolean_t no_ack, amqp_boolean_t exclusive, amqp_table_t arguments
) {
    char *q_name = queue.bytes;
    return (amqp_basic_consume_ok_t *)mock(conn_state, channel, q_name, no_local, no_ack);
}

amqp_rpc_reply_t amqp_consume_message(
    amqp_connection_state_t conn_state, amqp_envelope_t *envelope, const struct timeval *timeout, int flags
) {
    void            **evp_routekey = &envelope->routing_key.bytes;
    size_t           *evp_routekey_sz = &envelope->routing_key.len;
    amqp_bytes_t     *evp_msg_body = &envelope->message.body;
    amqp_bytes_t     *evp_corr_id = &envelope->message.properties.correlation_id;
    amqp_rpc_reply_t *out = (amqp_rpc_reply_t *)mock(
        conn_state, evp_routekey, evp_routekey_sz, evp_msg_body, evp_corr_id, timeout, flags
    );
    return *out;
} // end of  amqp_consume_message

void amqp_destroy_envelope(amqp_envelope_t *envelope) { mock(envelope); }

int amqp_get_sockfd(amqp_connection_state_t conn_state) { return (int)mock(conn_state); }

void amqp_maybe_release_buffers(amqp_connection_state_t conn_state) { mock(conn_state); }

amqp_basic_cancel_ok_t *
amqp_basic_cancel(amqp_connection_state_t conn_state, amqp_channel_t channel, amqp_bytes_t consumer_tag) {
    char *tag = consumer_tag.bytes;
    return (amqp_basic_cancel_ok_t *)mock(conn_state, tag);
}
