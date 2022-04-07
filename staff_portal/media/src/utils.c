#include "utils.h"

void * app_fetch_from_hashmap(struct hsearch_data *hmap, const char *keyword) {
    ENTRY *found = NULL;
    ENTRY e = {.key = keyword, .data = NULL };
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
    ENTRY  e = {.key = keyword, .data = (void *)value };
#pragma GCC diagnostic pop
    ENTRY *e_ret = NULL;
    success = hsearch_r(e, ENTER, &e_ret, hmap);
    return success;
} // end of app_save_int_to_hashmap

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
        char *value = strtok_r(NULL, "=", &ptr);
        if(!name) {
            continue;
        }
        json_t *obj_val = NULL;
        if(value) {
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

