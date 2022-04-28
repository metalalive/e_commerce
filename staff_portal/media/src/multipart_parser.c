/* Based on node-formidable by Felix Geisendörfer 
 * Igor Afonov - afonov@gmail.com - 2012
 * MIT License - http://www.opensource.org/licenses/mit-license.php
 */
#include "multipart_parser.h"

#include <stdio.h>
#include <stdarg.h>
#include <string.h>

static void multipart_log(const char * format, ...)
{
#ifdef DEBUG_MULTIPART_PARSER
    va_list args;
    va_start(args, format);

    fprintf(stderr, "[HTTP_MULTIPART_PARSER] %s:%d: ", __FILE__, __LINE__);
    vfprintf(stderr, format, args);
    fprintf(stderr, "\n");
#endif
}

#define NOTIFY_CB(FOR)                      \
do {                                        \
  if (p->settings.cbs.on_##FOR) {           \
    if (p->settings.cbs.on_##FOR(p) != 0) { \
      return i;                             \
    }                                       \
  }                                         \
} while (0)

#define EMIT_DATA_CB(FOR, ptr, len)                \
do {                                               \
  if (p->settings.cbs.on_##FOR) {                   \
    if (p->settings.cbs.on_##FOR(p, ptr, len) != 0) \
    {                                               \
      return i;                                    \
    }                                              \
  }                                                \
} while (0)


#define LF 10
#define CR 13
#define HYPHEN '-'

static const char mpfd_boundary_transition_pattern[] = {CR, LF, HYPHEN, HYPHEN};

multipart_parser *multipart_parser_init(const char *boundary, const multipart_parser_settings *settings) {
    if(!boundary || !settings || (settings->usr_args.sz > 0 && !settings->usr_args.entry))
    {
        return NULL;
    }
    size_t boundary_len = strlen(boundary);
    size_t mp_obj_sz = sizeof(multipart_parser) + settings->usr_args.sz + (boundary_len + 1) + (boundary_len + 8);
    multipart_parser *p = malloc(mp_obj_sz);
    p->boundary.len = boundary_len; // exclude NULL-terminating char
    p->index = 0;
    p->state = MULTIPART_STATE_ENTITY_START;
    memcpy(&p->settings, settings, sizeof(multipart_parser_settings));
    { // update internal pointers
        char *ptr = (char *)p + sizeof(multipart_parser);
        p->settings.usr_args.entry = (settings->usr_args.sz > 0 && settings->usr_args.entry) ? (void *)ptr: NULL;
        if(p->settings.usr_args.entry) {
            memcpy(p->settings.usr_args.entry, settings->usr_args.entry, settings->usr_args.sz);
        }
        ptr += settings->usr_args.sz;
        p->boundary.data = ptr;
        strcpy(&p->boundary.data[0], boundary);
        ptr += p->boundary.len + 1;
        p->lookbehind = ptr;
        // p->lookbehind = (&p->boundary.data[0] + p->boundary.len + 1);
    }
    return p;
} // end of multipart_parser_init

void multipart_parser_free(multipart_parser* p) {
  free(p);
}

size_t multipart_parser_execute(multipart_parser *p, const char *buf, size_t len) {
  // Note that the argument `buf` can be chunk of stream bytes, and this
  // function can be invoked several times with the same `parser` argument
  size_t i = 0;
  size_t mark = 0;
  char c, cl;
  int is_last = 0;
  int continue_parsing = 1;

  for(i = 0; (i < len) && continue_parsing; ++i) {
      c = buf[i];
      is_last = (i == (len - 1));
      switch (p->state) {
        case MULTIPART_STATE_ENTITY_START:
          multipart_log("MULTIPART_STATE_ENTITY_START");
          if(buf[0] != HYPHEN || buf[1] != HYPHEN) {
              continue_parsing = 0;
              break;
          }
          i = 2; // simply skip first 2 hyphens followed by the very-first boundary
          c = buf[i];
          p->index = 0;
          p->state = MULTIPART_STATE_INITIAL_BOUNDARY;

        /* fallthrough */
        case MULTIPART_STATE_INITIAL_BOUNDARY:
          multipart_log("MULTIPART_STATE_INITIAL_BOUNDARY");
          if (p->index == p->boundary.len) {
              if (c != CR) { // report error immediately
                  continue_parsing = 0;
              } else {
                  p->index++;
              }
          } else if (p->index == (p->boundary.len + 1)) {
              if (c != LF) { // report error immediately
                  continue_parsing = 0;
              } else {
                  p->index = 0;
                  p->state = MULTIPART_STATE_HEADER_FIELD_START;
                  NOTIFY_CB(part_data_begin);
              }
          } else {
              if (c != p->boundary.data[p->index]) {
                  continue_parsing = 0;
              } else {
                  p->index++;
              }
          }
          break;

        case MULTIPART_STATE_HEADER_FIELD_START:
          multipart_log("MULTIPART_STATE_HEADER_FIELD_START");
          mark = i;
          p->state = MULTIPART_STATE_HEADER_FIELD_PROCEED;

        /* fallthrough */
        case MULTIPART_STATE_HEADER_FIELD_PROCEED:
            multipart_log("MULTIPART_STATE_HEADER_FIELD_PROCEED");
            if (c == CR) {
                p->state = MULTIPART_STATE_HEADERS_POSSIBLE_END;
            } else if (c == ':') {
                EMIT_DATA_CB(header_field, buf + mark, i - mark);
                p->state = MULTIPART_STATE_HEADER_VALUE_START;
            } else {
                cl = tolower(c);
                if (c == HYPHEN) {
                    // skip
                } else  if (cl < 'a' || cl > 'z') {
                    multipart_log("invalid character in header name");
                    continue_parsing = 0;
                }
                if (is_last) {
                    EMIT_DATA_CB(header_field, buf + mark, (i - mark) + 1);
                }
            }
            break;

        case MULTIPART_STATE_HEADERS_POSSIBLE_END:
            multipart_log("MULTIPART_STATE_HEADERS_POSSIBLE_END");
            if (c == LF) {
                p->state = MULTIPART_STATE_PART_DATA_START;
            } else {
                continue_parsing = 0;
            }
            break;

        case MULTIPART_STATE_HEADER_VALUE_START:
            multipart_log("MULTIPART_STATE_HEADER_VALUE_START");
            if (c == ' ') {
                break;
            }
            mark = i;
            p->state = MULTIPART_STATE_HEADER_VALUE_PROCEED;

        /* fallthrough */
        case MULTIPART_STATE_HEADER_VALUE_PROCEED:
          multipart_log("MULTIPART_STATE_HEADER_VALUE_PROCEED");
          if (c == CR) {
             EMIT_DATA_CB(header_value, buf + mark, i - mark);
             p->state = MULTIPART_STATE_HEADER_VALUE_POSSIBLE_END;
          } else {
              if (is_last) {
                  EMIT_DATA_CB(header_value, buf + mark, (i - mark) + 1);
              }
          }
          break;

        case MULTIPART_STATE_HEADER_VALUE_POSSIBLE_END:
          multipart_log("MULTIPART_STATE_HEADER_VALUE_POSSIBLE_END");
          if (c == LF) {
              p->state = MULTIPART_STATE_HEADER_FIELD_START;
          } else {
              continue_parsing = 0;
          }
          break;

        case MULTIPART_STATE_PART_DATA_START:
          multipart_log("MULTIPART_STATE_PART_DATA_START");
          NOTIFY_CB(headers_complete);
          mark = i;
          p->state = MULTIPART_STATE_PART_DATA_PROCEED;

        /* fallthrough */
        case MULTIPART_STATE_PART_DATA_PROCEED:
          multipart_log("MULTIPART_STATE_PART_DATA_PROCEED");
          // probably reach next boundary, advance for further check
          if (c == mpfd_boundary_transition_pattern[0]) {
              if(i > mark) {
                  EMIT_DATA_CB(part_data, buf + mark, i - mark);
                  mark = i;
              } // omit if no data has been visited
              p->state = MULTIPART_STATE_PART_DATA_CR;
              p->lookbehind[0] = mpfd_boundary_transition_pattern[0];
          } else if (is_last) {
              EMIT_DATA_CB(part_data, buf + mark, (i - mark) + 1);
          }
          break;

        case MULTIPART_STATE_PART_DATA_CR:
        case MULTIPART_STATE_PART_DATA_LF:
        case MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN:
          if(p->state == MULTIPART_STATE_PART_DATA_CR) {
              multipart_log("MULTIPART_STATE_PART_DATA_CR");
          } else if(p->state == MULTIPART_STATE_PART_DATA_LF) {
              multipart_log("MULTIPART_STATE_PART_DATA_LF");
          } else if(p->state == MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN) {
              multipart_log("MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN");
          }
          { // probably reach next boundary, advance again for further check
              int delta = (int)p->state - (int)MULTIPART_STATE_PART_DATA_CR;
              char patt = mpfd_boundary_transition_pattern[1 + delta];
              if (c == patt) { // still not confirm it is the next boundary yet
                  p->state += 1; // MULTIPART_STATE_NEXT_BOUNDARY;
                  p->lookbehind[1 + delta] = c;
                  if(p->state == MULTIPART_STATE_NEXT_BOUNDARY) {
                      p->index = 0;
                  }
              } else {
                  // otherwise the parser has NOT reached next boundary yet,
                  // emit callback cuz the byte(s) preserved previously still belongs to current part data.
                  EMIT_DATA_CB(part_data, p->lookbehind, 1 + delta);
                  p->state = MULTIPART_STATE_PART_DATA_PROCEED;
                  mark = i --;
              }
          }
          break;

        case MULTIPART_STATE_NEXT_BOUNDARY:
            multipart_log("MULTIPART_STATE_NEXT_BOUNDARY");
            if (p->boundary.data[p->index] != c) {
                // the parser has NOT reached next boundary yet,
                // emit callback cuz the byte(s) preserved previously still belongs to current part data.
                EMIT_DATA_CB(part_data, p->lookbehind, sizeof(mpfd_boundary_transition_pattern) + p->index);
                p->state = MULTIPART_STATE_PART_DATA_PROCEED;
                mark = i --;
            } else { //  the parser has NOT reached next boundary yet, currently it is still part data
                p->lookbehind[sizeof(mpfd_boundary_transition_pattern) + p->index] = c;
                if ((++ p->index) == p->boundary.len) {
                    NOTIFY_CB(part_data_end);
                    p->state = MULTIPART_STATE_PART_DATA_POSSIBLE_END;
                }
            }
            break;

        case MULTIPART_STATE_PART_DATA_POSSIBLE_END:
            multipart_log("MULTIPART_STATE_PART_DATA_POSSIBLE_END");
            if (c == HYPHEN) {
                p->state = MULTIPART_STATE_ENTITY_POSSIBLE_END;
            } else if (c == CR) {
                p->state = MULTIPART_STATE_PART_DATA_END;
            } else {
                continue_parsing = 0;
            }
            break;

        case MULTIPART_STATE_PART_DATA_END:
            multipart_log("MULTIPART_STATE_PART_DATA_END");
            if (c == LF) {
                p->state = MULTIPART_STATE_HEADER_FIELD_START;
                NOTIFY_CB(part_data_begin);
            } else {
                continue_parsing = 0;
            }
            break;
   
        case MULTIPART_STATE_ENTITY_POSSIBLE_END:
            multipart_log("MULTIPART_STATE_ENTITY_POSSIBLE_END");
            if (c == HYPHEN) {
                NOTIFY_CB(body_end);
                p->state = MULTIPART_STATE_ENTITY_END; // fall through
            } else {
                continue_parsing = 0;
                break;
            }

        case MULTIPART_STATE_ENTITY_END:
            multipart_log("MULTIPART_STATE_ENTITY_END: %02X", (int) c);
            continue_parsing = 0;
            break;

        default:
            multipart_log("Multipart parser unrecoverable error");
            return 0;
      } // end of switch statement
  } // end of  loop
  return i;
} // end of multipart_parser_execute
