#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include <jansson.h>

#include "multipart_parser.h"

// * Due to section 5.1.1 in RFC2046, the boundary parameter has to come with Content-Type header
//   if the HTTP reqeust is multipart MIME-type.
// * The client side should ensure the boundary is absent in any of encapsulated parts in a
//   multipart entity
#define  TEST_MULTIPART_BOUNDARY_HALF  "p841S05"
#define  TEST_MULTIPART_BOUNDARY       TEST_MULTIPART_BOUNDARY_HALF "7eb956X10d3"
#define  TEST_CHAR_NEWLINE "\r\n"
#define  TEST_CHAR_2HYPHENS "--"
#define  TEST_REQ_BODY_IGNORED  "will be omitted"

static int utest_multipart__on_part_data_begin (multipart_parser *mp) {
    return mock(mp);
}
static int utest_multipart__on_headers_complete (multipart_parser *mp) {
    return mock(mp);
}
static int utest_multipart__on_part_data_end (multipart_parser *mp) {
    return mock(mp);
}
static int utest_multipart__on_body_end (multipart_parser *mp) {
    return mock(mp);
}

static int utest_multipart__on_header_kv (multipart_parser *mp, const char *at, size_t len) {
    multipart_parser_state curr_state = mp->state;
    char fetched_value[len + 1];
    memcpy(&fetched_value[0], at, len);
    fetched_value[len] = 0x0;
    return mock(mp, at, len, curr_state, fetched_value);
} // end of utest_multipart__on_header_kv

static int utest_multipart_1__on_part_data (multipart_parser *mp, const char *at, size_t len) {
    if(mp->settings.usr_args.entry) {
        char *readback = *(char **) mp->settings.usr_args.entry;
        memcpy(readback, at, len);
        *(char **) mp->settings.usr_args.entry = readback + len;
    }
    multipart_parser_state curr_state = mp->state;
    char fetched_value[len + 1];
    memcpy(&fetched_value[0], at, len);
    fetched_value[len] = 0x0;
    return mock(mp, at, len, curr_state, fetched_value);
}


#define  TEST_MULTIPART_PART_1  "prepare to parse even more complicated raw bytes"
// assume there's no header in each encapsulated part
#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_1                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS
Ensure(multipart_parsing_test__fragmented_bodypart_1) {
    char  mock_part_readback[sizeof(TEST_MULTIPART_PART_1)] = {0};
    char *mock_part_readback_ptr = &mock_part_readback[0];
    multipart_parser_settings  settings = {
        .usr_args = {.sz = sizeof(char **) , .entry = (void *)&mock_part_readback_ptr},
        .cbs = {
            .on_part_data_begin = utest_multipart__on_part_data_begin,
            .on_headers_complete = utest_multipart__on_headers_complete,
            .on_part_data = utest_multipart_1__on_part_data,
            .on_part_data_end = utest_multipart__on_part_data_end,
            .on_body_end = utest_multipart__on_body_end,
        }
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) {
        return;
    }
    size_t nread = 0;
    size_t mock_rd_buf_sz = 0;
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    const char *req_body_ptr_bak = req_body_ptr;
    // read small chucks of multipart entity
    { // read the first boundary
        mock_rd_buf_sz = sizeof(TEST_CHAR_2HYPHENS  TEST_MULTIPART_BOUNDARY  TEST_CHAR_NEWLINE);
        expect(utest_multipart__on_part_data_begin, will_return(0));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    { // read half of first part in the multipart entity
        size_t  part_data_half_sz = sizeof(TEST_MULTIPART_PART_1) >> 1;
        mock_rd_buf_sz = sizeof(TEST_CHAR_NEWLINE) + part_data_half_sz;
        expect(utest_multipart__on_headers_complete, will_return(0));
        expect(utest_multipart_1__on_part_data, will_return(0));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    { // read rest of first part in the multipart entity
        size_t  part_data_half_sz = sizeof(TEST_MULTIPART_PART_1) >> 1;
        size_t  part_boundary_half_sz = sizeof(TEST_MULTIPART_BOUNDARY) >> 1;
        mock_rd_buf_sz = part_data_half_sz + part_boundary_half_sz +
                sizeof(TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS);
        expect(utest_multipart_1__on_part_data, will_return(0));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
        // read content on both sides should match
        assert_that(mock_part_readback_ptr, is_equal_to_string(TEST_MULTIPART_PART_1));
        *(char **) mp->settings.usr_args.entry = mock_part_readback_ptr;
    }
    { // parse the closing boundary delimiter
        mock_rd_buf_sz = sizeof(TEST_MULTIPART_BOUNDARY  TEST_CHAR_2HYPHENS TEST_CHAR_NEWLINE);
        expect(utest_multipart__on_part_data_end, will_return(0));
        expect(utest_multipart__on_body_end, will_return(0));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(mp->state, is_equal_to(MULTIPART_STATE_ENTITY_END));
        req_body_ptr += nread;
        assert_that(req_body_ptr, is_equal_to(req_body_ptr_bak + sizeof(TEST_MULTIPART_ENTITY)));
    }
    multipart_parser_free(mp);
} // end of multipart_parsing_test__fragmented_bodypart_1
#undef  TEST_MULTIPART_PART_1
#undef  TEST_MULTIPART_ENTITY


#define  TEST_MULTIPART_BODYPART_CHUNK1 "wombat" "\r" 
#define  TEST_MULTIPART_BODYPART_CHUNK2 "penguin." 
#define  TEST_MULTIPART_BODYPART_CHUNK3 "pigeon" TEST_CHAR_NEWLINE
#define  TEST_MULTIPART_BODYPART_CHUNK4 "kangaroo." 
#define  TEST_MULTIPART_BODYPART_CHUNK5 "koala" TEST_CHAR_NEWLINE "-"
#define  TEST_MULTIPART_BODYPART_CHUNK6 "sloths." 
#define  TEST_MULTIPART_BODYPART_CHUNK7 "catfish" TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS
#define  TEST_MULTIPART_BODYPART_CHUNK8 "electric fish." 
#define  TEST_MULTIPART_BODY_PART  TEST_MULTIPART_BODYPART_CHUNK1  TEST_MULTIPART_BODYPART_CHUNK2 \
        TEST_MULTIPART_BODYPART_CHUNK3  TEST_MULTIPART_BODYPART_CHUNK4  TEST_MULTIPART_BODYPART_CHUNK5 \
        TEST_MULTIPART_BODYPART_CHUNK6  TEST_MULTIPART_BODYPART_CHUNK7  TEST_MULTIPART_BODYPART_CHUNK8
#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_BODY_PART                   TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS \
    TEST_REQ_BODY_IGNORED
Ensure(multipart_parsing_test__fragmented_bodypart_2) {
    // this test case shows that you may read more bytes than you specified in `len`
    //  argument of multipart_parser_execute() when certain situations happen
    multipart_parser_settings  settings = {.usr_args = {.sz = 0, .entry = NULL},
        .cbs = {.on_part_data = utest_multipart_1__on_part_data }
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) { return; }
    size_t nread = 0;
    size_t mock_rd_buf_sz = 0;
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    {
        const char *exp_tot_rd_data = TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY  TEST_CHAR_NEWLINE
            TEST_CHAR_NEWLINE  TEST_MULTIPART_BODYPART_CHUNK1;
        size_t exp_rd_sz = sizeof(TEST_MULTIPART_BODYPART_CHUNK1) - 1;
        char exp_rd_data[sizeof(TEST_MULTIPART_BODYPART_CHUNK1) - 1] = {0};
        memcpy(exp_rd_data, TEST_MULTIPART_BODYPART_CHUNK1, exp_rd_sz - 1); // final \r character should be preserved in mp object
        mock_rd_buf_sz = strlen(exp_tot_rd_data);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(exp_rd_data)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK2);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string("\r")));
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_MULTIPART_BODYPART_CHUNK2)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK3);
        char exp_rd_data[sizeof(TEST_MULTIPART_BODYPART_CHUNK3)] = {0};
        memcpy(exp_rd_data, TEST_MULTIPART_BODYPART_CHUNK3, mock_rd_buf_sz - 2);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string( exp_rd_data )));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK4);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_CHAR_NEWLINE)));
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_MULTIPART_BODYPART_CHUNK4)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK5);
        char exp_rd_data[sizeof(TEST_MULTIPART_BODYPART_CHUNK5)] = {0};
        memcpy(exp_rd_data, TEST_MULTIPART_BODYPART_CHUNK5, mock_rd_buf_sz - 3);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string( exp_rd_data )));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK6);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_CHAR_NEWLINE "-")));
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_MULTIPART_BODYPART_CHUNK6)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK7);
        char exp_rd_data[sizeof(TEST_MULTIPART_BODYPART_CHUNK7)] = {0};
        memcpy(exp_rd_data, TEST_MULTIPART_BODYPART_CHUNK7, mock_rd_buf_sz - 4);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string( exp_rd_data )));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    {
        mock_rd_buf_sz = strlen(TEST_MULTIPART_BODYPART_CHUNK8);
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS)));
        expect(utest_multipart_1__on_part_data, will_return(0),  when(fetched_value, is_equal_to_string(TEST_MULTIPART_BODYPART_CHUNK8)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
} // end of multipart_parsing_test__fragmented_bodypart_2
#undef  TEST_MULTIPART_ENTITY
#undef  TEST_MULTIPART_BODY_PART
#undef  TEST_MULTIPART_BODYPART_CHUNK1
#undef  TEST_MULTIPART_BODYPART_CHUNK2
#undef  TEST_MULTIPART_BODYPART_CHUNK3
#undef  TEST_MULTIPART_BODYPART_CHUNK4
#undef  TEST_MULTIPART_BODYPART_CHUNK5
#undef  TEST_MULTIPART_BODYPART_CHUNK6
#undef  TEST_MULTIPART_BODYPART_CHUNK7
#undef  TEST_MULTIPART_BODYPART_CHUNK8



#define  TEST_MULTIPART_PART_1  "SOLID\r" "high-traffic app" TEST_CHAR_NEWLINE \
    "Golang" TEST_CHAR_NEWLINE "-"   "Scalable" TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS \
    "Rust"  TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS  TEST_MULTIPART_BOUNDARY_HALF \
    "still in current part" TEST_CHAR_NEWLINE TEST_CHAR_2HYPHENS \
    "DB indexing" TEST_CHAR_NEWLINE "-"  "B-tree lookup" TEST_CHAR_NEWLINE "people"

#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_1                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS \
    TEST_REQ_BODY_IGNORED
Ensure(multipart_parsing_test__identify_boundary)
{ // parse the content that is pretty similar to (but NOT the same as) the defined boundary
    char  mock_part_readback[sizeof(TEST_MULTIPART_PART_1)] = {0};
    char *mock_part_readback_ptr = &mock_part_readback[0];
    multipart_parser_settings  settings = {
        .usr_args = {.sz = sizeof(char **) , .entry = (void *)&mock_part_readback_ptr},
        .cbs = {.on_part_data = utest_multipart_1__on_part_data}
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) {
        return;
    }
    size_t nread = 0;
    size_t mock_rd_buf_sz = 0;
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    const char *req_body_ptr_bak = req_body_ptr;
    { // parse the first half, data callback will be invoked several times , for handling edge cases
        mock_rd_buf_sz = sizeof(TEST_MULTIPART_ENTITY) >> 1;
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_CR)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_LF)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_NEXT_BOUNDARY)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_NEXT_BOUNDARY)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        assert_that(nread, is_equal_to(mock_rd_buf_sz));
        req_body_ptr += nread;
    }
    { // parse the second half of the multipart entity
        mock_rd_buf_sz = (sizeof(TEST_MULTIPART_ENTITY) >> 1) + 1;
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_NEXT_BOUNDARY)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_NEWLINE_HYPHEN)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_LF)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)));
        nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        req_body_ptr += nread;
        size_t expect_total_nread = sizeof(TEST_MULTIPART_ENTITY) - strlen(TEST_REQ_BODY_IGNORED);
        assert_that(req_body_ptr, is_equal_to(req_body_ptr_bak + expect_total_nread));
    }
    assert_that(mock_part_readback_ptr, is_equal_to_string(TEST_MULTIPART_PART_1));
    multipart_parser_free(mp);
} // end of multipart_parsing_test__identify_boundary
#undef  TEST_MULTIPART_PART_1
#undef  TEST_MULTIPART_ENTITY


#define  TEST_MULTIPART_PART_1  "the company succeed not because they got first."
#define  TEST_MULTIPART_PART_2  "cannot draw explicit line between software architecture and design."
#define  TEST_MULTIPART_PART_3  "any chance to see driving-car on the road in 5 years"
#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_1                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_2                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_3                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS \
    TEST_REQ_BODY_IGNORED
Ensure(multipart_parsing_test__several_parts_ok) {
#define EXPECT_READBACK_CONTENT  TEST_MULTIPART_PART_1  TEST_MULTIPART_PART_2  TEST_MULTIPART_PART_3
    char  mock_part_readback[sizeof(EXPECT_READBACK_CONTENT)] = {0};
    char *mock_part_readback_ptr = &mock_part_readback[0];
    multipart_parser_settings  settings = {
        .usr_args = {.sz = sizeof(char **) , .entry = (void *)&mock_part_readback_ptr},
        .cbs = {.on_part_data = utest_multipart_1__on_part_data}
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) {
        return;
    }
    size_t mock_rd_buf_sz = 0;
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    { // parse the 3 encapsulated parts in one go, the data callback will be invoked 3 times
        mock_rd_buf_sz = sizeof(TEST_MULTIPART_ENTITY); 
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_MULTIPART_PART_1)) );
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_MULTIPART_PART_2)) );
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_MULTIPART_PART_3)) );
        size_t actual_nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        size_t expect_nread = sizeof(TEST_MULTIPART_ENTITY) - strlen(TEST_REQ_BODY_IGNORED);
        assert_that(actual_nread, is_equal_to(expect_nread));
    }
    assert_that(mock_part_readback_ptr, is_equal_to_string(EXPECT_READBACK_CONTENT));
    multipart_parser_free(mp);
#undef  EXPECT_READBACK_CONTENT
} // end of multipart_parsing_test__several_parts_ok
#undef  TEST_MULTIPART_PART_1
#undef  TEST_MULTIPART_PART_2
#undef  TEST_MULTIPART_ENTITY


#define  TEST_PART_1_HEADER_FIELD "Content-ID"// see RFC2045, RFC2046
#define  TEST_PART_1_HEADER_VALUE "<not.necessary@to.be.email>"
#define  TEST_PART_2_HEADER_1_FIELD "Content-Disposition"
#define  TEST_PART_2_HEADER_1_VALUE "form-data; name=\"m190ts8gxL\""
#define  TEST_PART_2_HEADER_2_FIELD "Content-Description"
#define  TEST_PART_2_HEADER_2_VALUE "for unit testing"
#define  TEST_PART_1_HEADER  TEST_PART_1_HEADER_FIELD ":      " TEST_PART_1_HEADER_VALUE
#define  TEST_PART_2_HEADER  TEST_PART_2_HEADER_1_FIELD ": " TEST_PART_2_HEADER_1_VALUE  TEST_CHAR_NEWLINE \
    TEST_PART_2_HEADER_2_FIELD ":   " TEST_PART_2_HEADER_2_VALUE
#define  TEST_MULTIPART_PART_1  "overdrive vs. distortion vs. Boogie"
#define  TEST_MULTIPART_PART_2  "reverb, delay, wah, equalizer"
#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
    TEST_PART_1_HEADER                         TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_1                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
    TEST_PART_2_HEADER                         TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_2                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS
Ensure(multipart_parsing_test__headers_ok) {
    multipart_parser_settings  settings = { .usr_args = {.sz = 0 , .entry = NULL},
        .cbs = { .on_part_data = utest_multipart_1__on_part_data,
            .on_header_field = utest_multipart__on_header_kv,
            .on_header_value = utest_multipart__on_header_kv }
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) {
        return;
    }
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    {
        size_t mock_rd_buf_sz = sizeof(TEST_MULTIPART_ENTITY); 
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_FIELD_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_1_HEADER_FIELD)));
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_VALUE_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_1_HEADER_VALUE)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_MULTIPART_PART_1)) );
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_FIELD_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_2_HEADER_1_FIELD)));
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_VALUE_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_2_HEADER_1_VALUE)));
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_FIELD_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_2_HEADER_2_FIELD)));
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_VALUE_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_2_HEADER_2_VALUE)));
        expect(utest_multipart_1__on_part_data, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_PART_DATA_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_MULTIPART_PART_2)) );
        size_t actual_nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        size_t expect_nread = sizeof(TEST_MULTIPART_ENTITY);
        assert_that(actual_nread, is_equal_to(expect_nread));
    }
    multipart_parser_free(mp);
} // end of multipart_parsing_test__headers_ok
#undef  TEST_MULTIPART_ENTITY
#undef  TEST_MULTIPART_PART_1
#undef  TEST_MULTIPART_PART_2
#undef  TEST_PART_1_HEADER_FIELD
#undef  TEST_PART_1_HEADER_VALUE
#undef  TEST_PART_2_HEADER_1_FIELD
#undef  TEST_PART_2_HEADER_1_VALUE
#undef  TEST_PART_2_HEADER_2_FIELD
#undef  TEST_PART_2_HEADER_2_VALUE
#undef  TEST_PART_1_HEADER
#undef  TEST_PART_2_HEADER



#define  TEST_PART_1_HEADER_FIELD "Content-Description"
#define  TEST_PART_1_HEADER_VALUE__FRAGMENT_1  "does Test-Driven Development"
#define  TEST_PART_1_HEADER_VALUE__FRAGMENT_2  " allow you to apply appropriate architecture"
#define  TEST_PART_1_HEADER  TEST_PART_1_HEADER_FIELD  ":"  TEST_PART_1_HEADER_VALUE__FRAGMENT_1 \
    TEST_PART_1_HEADER_VALUE__FRAGMENT_2
#define  TEST_MULTIPART_PART_1  "Agile vs. Scrum"
#define  TEST_MULTIPART_ENTITY \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE \
    TEST_PART_1_HEADER                         TEST_CHAR_NEWLINE \
                                               TEST_CHAR_NEWLINE \
    TEST_MULTIPART_PART_1                      TEST_CHAR_NEWLINE \
    TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_2HYPHENS
Ensure(multipart_parsing_test__fragmented_header) {
    multipart_parser_settings  settings = { .usr_args = {.sz = 0 , .entry = NULL},
        .cbs = { .on_header_field = utest_multipart__on_header_kv,
            .on_header_value = utest_multipart__on_header_kv }
    };
    multipart_parser *mp = multipart_parser_init(TEST_MULTIPART_BOUNDARY, &settings);
    assert_that(mp, is_not_null);
    if (!mp) {
        return;
    }
    size_t mock_rd_buf_sz = 0;
    const char *req_body_ptr = TEST_MULTIPART_ENTITY;
    {
        const char *expect_rd_chunk = TEST_CHAR_2HYPHENS TEST_MULTIPART_BOUNDARY TEST_CHAR_NEWLINE 
                                      TEST_PART_1_HEADER_FIELD  ":"  TEST_PART_1_HEADER_VALUE__FRAGMENT_1;
        mock_rd_buf_sz = strlen(expect_rd_chunk); 
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_FIELD_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_1_HEADER_FIELD)));
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_VALUE_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_1_HEADER_VALUE__FRAGMENT_1)));
        size_t nread = multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
        req_body_ptr += nread;
    }
    {
        const char *expect_rd_chunk = TEST_PART_1_HEADER_VALUE__FRAGMENT_2 TEST_CHAR_NEWLINE
            TEST_CHAR_NEWLINE  TEST_MULTIPART_PART_1;
        mock_rd_buf_sz = strlen(expect_rd_chunk); 
        expect(utest_multipart__on_header_kv, will_return(0), when(curr_state, is_equal_to(MULTIPART_STATE_HEADER_VALUE_PROCEED)),
                when(fetched_value, is_equal_to_string(TEST_PART_1_HEADER_VALUE__FRAGMENT_2)));
        multipart_parser_execute(mp, req_body_ptr, mock_rd_buf_sz);
    }
    multipart_parser_free(mp);
} // end of multipart_parsing_test__fragmented_header
#undef TEST_PART_1_HEADER_FIELD
#undef TEST_PART_1_HEADER_VALUE__FRAGMENT_1
#undef TEST_PART_1_HEADER_VALUE__FRAGMENT_2
#undef TEST_PART_1_HEADER
#undef TEST_MULTIPART_PART_1
#undef TEST_MULTIPART_ENTITY



TestSuite *app_multipart_parsing_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, multipart_parsing_test__fragmented_bodypart_1);
    add_test(suite, multipart_parsing_test__fragmented_bodypart_2);
    add_test(suite, multipart_parsing_test__identify_boundary);
    add_test(suite, multipart_parsing_test__several_parts_ok);
    add_test(suite, multipart_parsing_test__headers_ok);
    add_test(suite, multipart_parsing_test__fragmented_header);
    return suite;
}
