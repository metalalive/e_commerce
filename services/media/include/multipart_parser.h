/* Based on node-formidable by Felix Geisendörfer
 * Igor Afonov - afonov@gmail.com - 2012
 * MIT License - http://www.opensource.org/licenses/mit-license.php
 *
 */
#ifndef MEDIA__MULTIPART_PARSER_H
#define MEDIA__MULTIPART_PARSER_H
#ifdef __cplusplus
extern "C" {
#endif

#include <stdlib.h>
#include <ctype.h>

typedef struct _multipart_parser multipart_parser;

typedef int (*multipart_data_cb)(multipart_parser *, const char *at, size_t length);
typedef int (*multipart_notify_cb)(multipart_parser *);

typedef struct {
    struct {
        multipart_data_cb   on_header_field;
        multipart_data_cb   on_header_value;
        multipart_data_cb   on_part_data;
        multipart_notify_cb on_part_data_begin;
        multipart_notify_cb on_headers_complete;
        multipart_notify_cb on_part_data_end;
        multipart_notify_cb on_body_end;
    } cbs;
    struct {
        size_t sz;
        void  *entry;
    } usr_args;
} multipart_parser_settings;

typedef enum {
    MULTIPART_STATE_UNINITIALIZED = 1,
    MULTIPART_STATE_ENTITY_START,
    MULTIPART_STATE_INITIAL_BOUNDARY,
    MULTIPART_STATE_HEADER_FIELD_START,
    MULTIPART_STATE_HEADER_FIELD_PROCEED,
    MULTIPART_STATE_HEADERS_POSSIBLE_END,
    MULTIPART_STATE_HEADER_VALUE_START,
    MULTIPART_STATE_HEADER_VALUE_PROCEED,
    MULTIPART_STATE_HEADER_VALUE_POSSIBLE_END,
    MULTIPART_STATE_PART_DATA_START,
    MULTIPART_STATE_PART_DATA_PROCEED,
    // individual states when detecting any character pattern starting with carriage-return (CR),
    // line-feed (LF), followed by reserved hyphen `-` in data portion of each encapsulated part
    MULTIPART_STATE_PART_DATA_CR,
    MULTIPART_STATE_PART_DATA_LF,
    MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN,
    MULTIPART_STATE_NEXT_BOUNDARY,
    MULTIPART_STATE_PART_DATA_POSSIBLE_END,
    MULTIPART_STATE_PART_DATA_END,
    MULTIPART_STATE_ENTITY_POSSIBLE_END,
    MULTIPART_STATE_ENTITY_END
} multipart_parser_state;

struct _multipart_parser {
    size_t                    index;
    multipart_parser_state    state;
    multipart_parser_settings settings;
    char                     *lookbehind;
    struct {
        size_t len;
        char  *data;
    } boundary;
};

multipart_parser *multipart_parser_init(const char *boundary, const multipart_parser_settings *settings);

void multipart_parser_free(multipart_parser *p);

size_t multipart_parser_execute(multipart_parser *p, const char *buf, size_t len);

#ifdef __cplusplus
} // end of  extern "C"
#endif
#endif // end of MEDIA__MULTIPART_PARSER_H
