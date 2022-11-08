#include "utils.h"

void * app_fetch_from_hashmap(struct hsearch_data *hmap, const char *keyword) {
    ENTRY *found = NULL;
    ENTRY e = {.key = (char *)keyword, .data = NULL };
    int success = hsearch_r(e, FIND, &found, hmap);
    if(success && found) {
        return found->data;
    } else {
        return NULL;
    }
}

int app_save_int_to_hashmap(struct hsearch_data *hmap, const char *keyword, int value)
{
    int success = 0;
    if(!hmap || !keyword) {
        return success;
    }
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
    ENTRY  e0 = {.key = (char *)keyword, .data = NULL };
    ENTRY  e1 = {.key = (char *)keyword, .data = (void *)value };
#pragma GCC diagnostic pop
    ENTRY *e0_ret = NULL;
    ENTRY *e1_ret = NULL;
    // firstly remove existing entry using the same hash (if exists)
    // then insert new entry
    if(hsearch_r(e0, FIND, &e0_ret, hmap)) {
#pragma GCC diagnostic ignored "-Wint-to-pointer-cast"
        e0_ret->data = (void *)value;
#pragma GCC diagnostic pop
        success = 1;
    } else {
        success = hsearch_r(e1, ENTER, &e1_ret, hmap);
    }
    return success;
} // end of app_save_int_to_hashmap

int app_save_ptr_to_hashmap(struct hsearch_data *hmap, const char *keyword, void *value)
{
    int success = 0;
    if(!hmap || !keyword) {
        return success;
    }
    ENTRY  e0 = {.key = (char *)keyword, .data = NULL };
    ENTRY  e1 = {.key = (char *)keyword, .data = value };
    ENTRY *e0_ret = NULL;
    ENTRY *e1_ret = NULL;
    // firstly remove existing entry using the same hash (if exists)
    // then insert new entry
    if(hsearch_r(e0, FIND, &e0_ret, hmap)) {
        e0_ret->data = value;
        success = 1;
    } else {
        success = hsearch_r(e1, ENTER, &e1_ret, hmap);
    }
    return success;
} // end of app_save_ptr_to_hashmap

int app_url_decode_query_param(char *data, json_t *map) {
    // this function does NOT check inproper characters appeared in name/value field
    // and treat all value as string by default.
    char *tok = data;
    char *ptr_kvpair = NULL;
    size_t num_items = 0;
    for(tok = strtok_r(tok, "&", &ptr_kvpair); tok; tok = strtok_r(NULL, "&", &ptr_kvpair))
    { // strtok_r is thread-safe
        char *ptr = NULL;
        char *name  = strtok_r(tok,  "=", &ptr);
        char *value = ptr; // in case the value contains equal symbol, do not use  strtok_r(NULL, "=", &ptr);
        if(!name)
            continue;
        json_t *obj_val = NULL;
        if(value && strlen(value) > 0) {
            obj_val = json_string(value);
        } else {
            obj_val = json_true();
        }
        json_object_set_new(map, name, obj_val);
        num_items++;
        //fprintf(stdout, "[debug] raw data of query params: %s = %s \n", name, value);
    }
    return num_items;
} // end of app_url_decode_query_param


void app_llnode_link(app_llnode_t *curr, app_llnode_t *prev, app_llnode_t *new)
{
    if(prev) {
        prev->next = new;
    }
    if(new) {
        new->prev = prev;
        new->next = curr;
    }
    if(curr) {
        curr->prev = new;
    }
}

void app_llnode_unlink(app_llnode_t *node)
{
    app_llnode_t *n0 = node->prev;
    app_llnode_t *n1 = node->next;
    if(n0) {
        n0->next = n1;
    }
    if(n1) {
        n1->prev = n0;
    }
    node->next = node->prev = NULL;
}


int app_chararray_to_hexstr(char *out, size_t out_sz, const char *in, size_t in_sz)
{
    if(!out || !in || in_sz == 0 || out_sz == 0) {
        return 1;
    }
    const char *map = "0123456789abcdef";
    for(size_t idx = 0; idx < in_sz; idx++) {
        size_t jdx = idx << 1;
        out[jdx + 0] = map[(in[idx] >> 4) & 0xf];
        out[jdx + 1] = map[ in[idx] & 0xf];
    }
    return 0; // ok
}

int app_hexstr_to_chararray(char *out, size_t out_sz, const char *in, size_t in_sz)
{ // this function assume the input is an octet array of hex char
    int err = 0;
    if(!out || !in || in_sz == 0 || out_sz == 0) {
        err = 1;
    } else if ((out_sz << 1) != in_sz) {
        err = 2;
    }
    if(err)
        goto done;
    for(size_t idx = 0; idx < in_sz; idx++) {
        char c = in[idx];
        int8_t  n = -1;
        if('0' <= c && c <= '9') {
            n = c - '0';
        } else if('a' <= c && c <= 'f') {
            n = c - 'a' + 0xa;
        } else if('A' <= c && c <= 'F') {
            n = c - 'A' + 0xa;
        } else { // invalid char in hex string input
            err = 3;
            break;
        }
        size_t octet_pos = idx >> 1,  bit_pos = (~idx & 1) << 2;
        if(bit_pos != 0)
            out[octet_pos] = 0;
        out[octet_pos] |= n << bit_pos;
    } // end of loop
done:
    return  err;
}
