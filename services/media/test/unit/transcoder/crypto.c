#include <cgreen/cgreen.h>
#include <cgreen/unit.h>
#include <cgreen/mocks.h>

#include "base64.h"
#include "transcoder/file_processor.h"

#define KEY_ID_1 "40D24411"
#define KEY_ID_2 "7159F2BF"
#define KEY_ID_3 "D4691CAC"
#define ALG_1    "cy12"
#define ALG_2    "cs13"
#define ALG_3    "me51"
#define UTEST_KEYMAP_RAWDATA \
    "{" \
    "\"" KEY_ID_1 "\":{\"key\":{\"nbytes\":4,\"data\":\"3C0EA878\"},\"alg\":\"" ALG_1 \
    "\",\"timestamp\":1666684717}," \
    "\"" KEY_ID_2 "\":{\"key\":{\"nbytes\":4,\"data\":\"534B56F6\"},\"alg\":\"" ALG_2 \
    "\",\"timestamp\":1666685421}," \
    "\"" KEY_ID_3 "\":{\"key\":{\"nbytes\":4,\"data\":\"5E30A5CF\"},\"alg\":\"" ALG_3 \
    "\",\"timestamp\":1666685591}" \
    "}"

Ensure(atfp_hls_test__crypto__get_valid_key) {
    const char *id_found = NULL;
    json_t     *item_found = NULL;
    json_t     *key_map = json_loadb(UTEST_KEYMAP_RAWDATA, sizeof(UTEST_KEYMAP_RAWDATA) - 1, 0, NULL);
#define VERIFY(_idx) \
    { \
        item_found = NULL; \
        id_found = atfp_get_crypto_key(key_map, KEY_ID_##_idx, &item_found); \
        assert_that(id_found, is_equal_to_string(KEY_ID_##_idx)); \
        assert_that(item_found, is_not_null); \
        assert_that(json_string_value(json_object_get(item_found, "alg")), is_equal_to_string(ALG_##_idx)); \
    }
    VERIFY(2)
    VERIFY(1)
    VERIFY(3)
#undef VERIFY
    id_found = atfp_get_crypto_key(key_map, ATFP__CRYPTO_KEY_MOST_RECENT, &item_found);
    assert_that(id_found, is_equal_to_string(KEY_ID_3));
    assert_that(json_string_value(json_object_get(item_found, "alg")), is_equal_to_string(ALG_3));
    json_decref(key_map);
} // end of  atfp_hls_test__crypto__get_valid_key

Ensure(atfp_hls_test__crypto__key_not_found) {
    const char *id_found = NULL;
    json_t     *item_found = NULL;
    json_t     *key_map = json_loadb(UTEST_KEYMAP_RAWDATA, sizeof(UTEST_KEYMAP_RAWDATA) - 1, 0, NULL);
    { // get rid of timestamp field
        json_t *item = json_object_get(key_map, KEY_ID_2);
        json_object_del(item, "timestamp");
        item_found = NULL;
        id_found = atfp_get_crypto_key(key_map, ATFP__CRYPTO_KEY_MOST_RECENT, &item_found);
        assert_that(item_found, is_null);
        assert_that(id_found, is_null);
    }
    {
        const char *nonexist_key_id = "Y8dk93l0";
        item_found = NULL;
        id_found = atfp_get_crypto_key(key_map, nonexist_key_id, &item_found);
        assert_that(item_found, is_null);
        assert_that(id_found, is_null);
    }
    json_decref(key_map);
} // end of  atfp_hls_test__crypto__key_not_found

#undef UTEST_KEYMAP_RAWDATA
#undef KEY_ID_1
#undef KEY_ID_2
#undef KEY_ID_3
#undef ALG_1
#undef ALG_2
#undef ALG_3

#define UTEST_KEY_HEX      "3c053ea878"
#define UTEST_IV_HEX       "4b56f6"
#define UTEST_KEY_OCTET    "\x3c\x05\x3e\xa8\x78"
#define UTEST_IV_OCTET     "\x4b\x56\xf6"
#define UTEST_KEY_OCTET_SZ (sizeof(UTEST_KEY_OCTET) - 1)
#define UTEST_IV_OCTET_SZ  (sizeof(UTEST_IV_OCTET) - 1)
#define UTEST_KEYITEM_RAWDATA \
    "{" \
    "\"key\":{\"nbytes\":5,\"data\":\"" UTEST_KEY_HEX "\"}," \
    "\"iv\":{\"nbytes\":3,\"data\":\"" UTEST_IV_HEX "\"}" \
    "}"
#define UTEST_ENCRYPT_BLOCK_SZ  128
#define UTEST_USR_ID            1493
#define UTEST_UPLD_REQ_ID       0x0dead5ea
#define EXPECT_PLAINTEXT        "1493/0dead5ea"
#define EXPECT_CIPHERTEXT_PART1 "\xe3\x7a\xc9\x6d\x2c\x5b\xf8\x30\x05\x7d\x1a"
#define EXPECT_CIPHERTEXT_PART2 "\xa8\xc6\xd2\xc5\x01"
Ensure(atfp_hls_test__crypto__encrypt_doc_id_ok) {
    int            mock_ctx = 0x1234;
    atfp_data_t    mock_fp_data = {.usr_id = UTEST_USR_ID, .upld_req_id = UTEST_UPLD_REQ_ID};
    json_t        *key_item = json_loadb(UTEST_KEYITEM_RAWDATA, sizeof(UTEST_KEYITEM_RAWDATA) - 1, 0, NULL);
    unsigned char *out = NULL;
    size_t         out_sz = 0;
    {
        const char *expect_key_octet = UTEST_KEY_OCTET, *expect_iv_octet = UTEST_IV_OCTET;
        long        expect_key_len = UTEST_KEY_OCTET_SZ, expect_iv_len = UTEST_IV_OCTET_SZ;
        expect(
            EVP_CIPHER_CTX_set_key_length, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            when(keylen, is_equal_to(UTEST_KEY_OCTET_SZ))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(expect_key_octet), when(str, is_equal_to_string(UTEST_KEY_HEX)),
            will_set_contents_of_parameter(len, &expect_key_len, sizeof(long))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(expect_iv_octet), when(str, is_equal_to_string(UTEST_IV_HEX)),
            will_set_contents_of_parameter(len, &expect_iv_len, sizeof(long))
        );
        expect(
            EVP_EncryptInit_ex, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            when(key, is_equal_to_string(expect_key_octet)), when(iv, is_equal_to_string(expect_iv_octet))
        );
        expect(EVP_CIPHER_CTX_block_size, will_return(UTEST_ENCRYPT_BLOCK_SZ));
        const char *expect_ct[2] = {EXPECT_CIPHERTEXT_PART1, EXPECT_CIPHERTEXT_PART2};
        size_t      expect_ct_sz[2] = {
            strlen(EXPECT_CIPHERTEXT_PART1) * sizeof(char), strlen(EXPECT_CIPHERTEXT_PART2) * sizeof(char)
        };
        expect(
            EVP_EncryptUpdate, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            when(in, is_equal_to_string(EXPECT_PLAINTEXT)),
            will_set_contents_of_parameter(outl, &expect_ct_sz[0], sizeof(int)),
            will_set_contents_of_parameter(out, expect_ct[0], expect_ct_sz[0])
        );
        expect(
            EVP_EncryptFinal_ex, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            will_set_contents_of_parameter(outl, &expect_ct_sz[1], sizeof(int)),
            will_set_contents_of_parameter(out, expect_ct[1], expect_ct_sz[1])
        );
        expect(CRYPTO_free, when(addr, is_equal_to(expect_iv_octet)));
        expect(CRYPTO_free, when(addr, is_equal_to(expect_key_octet)));
    }
    int success =
        atfp_encrypt_document_id((EVP_CIPHER_CTX *)&mock_ctx, &mock_fp_data, key_item, &out, &out_sz);
    assert_that(success, is_equal_to(1));
    assert_that(out_sz, is_greater_than(0));
    assert_that(out, is_not_null);
    if (out && out_sz > 0) {
        const char *expect_doc_id_octet = EXPECT_CIPHERTEXT_PART1 EXPECT_CIPHERTEXT_PART2;
        size_t         expect_doc_id_octet_sz = strlen(expect_doc_id_octet);
        size_t         actual_doc_id_octet_sz = 0;
        unsigned char *actual_doc_id_octet =
            base64_decode((const unsigned char *)out, out_sz, &actual_doc_id_octet_sz);
        assert_that(actual_doc_id_octet_sz, is_equal_to(expect_doc_id_octet_sz));
        assert_that(actual_doc_id_octet, begins_with_string(expect_doc_id_octet));
        free(actual_doc_id_octet);
        free(out);
    }
    json_decref(key_item);
} // end of  atfp_hls_test__crypto__encrypt_doc_id_ok

Ensure(atfp_hls_test__crypto__encrypt_doc_id_error) {
    int            mock_ctx = 0x1234;
    atfp_data_t    mock_fp_data = {.usr_id = UTEST_USR_ID, .upld_req_id = UTEST_UPLD_REQ_ID};
    json_t        *key_item = json_loadb(UTEST_KEYITEM_RAWDATA, sizeof(UTEST_KEYITEM_RAWDATA) - 1, 0, NULL);
    unsigned char *out = NULL;
    size_t         out_sz = 0;
    {
        const char *expect_key_octet = UTEST_KEY_OCTET, *expect_iv_octet = UTEST_IV_OCTET;
        long        expect_key_len = UTEST_KEY_OCTET_SZ, expect_iv_len = UTEST_IV_OCTET_SZ;
        expect(
            EVP_CIPHER_CTX_set_key_length, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            when(keylen, is_equal_to(UTEST_KEY_OCTET_SZ))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(expect_key_octet), when(str, is_equal_to_string(UTEST_KEY_HEX)),
            will_set_contents_of_parameter(len, &expect_key_len, sizeof(long))
        );
        expect(
            OPENSSL_hexstr2buf, will_return(expect_iv_octet), when(str, is_equal_to_string(UTEST_IV_HEX)),
            will_set_contents_of_parameter(len, &expect_iv_len, sizeof(long))
        );
        expect(
            EVP_EncryptInit_ex, will_return(1), when(ctx, is_equal_to(&mock_ctx)),
            when(key, is_equal_to_string(expect_key_octet)), when(iv, is_equal_to_string(expect_iv_octet))
        );
        expect(EVP_CIPHER_CTX_block_size, will_return(UTEST_ENCRYPT_BLOCK_SZ));
        expect(
            EVP_EncryptUpdate, will_return(0), when(ctx, is_equal_to(&mock_ctx)),
            when(in, is_equal_to_string(EXPECT_PLAINTEXT))
        ); // assume error happened
        expect(CRYPTO_free, when(addr, is_equal_to(expect_iv_octet)));
        expect(CRYPTO_free, when(addr, is_equal_to(expect_key_octet)));
    }
    int success =
        atfp_encrypt_document_id((EVP_CIPHER_CTX *)&mock_ctx, &mock_fp_data, key_item, &out, &out_sz);
    assert_that(success, is_equal_to(0));
    assert_that(out_sz, is_equal_to(0));
    assert_that(out, is_null);
    json_decref(key_item);
} // end of  atfp_hls_test__crypto__encrypt_doc_id_error
#undef EXPECT_CIPHERTEXT_PART1
#undef EXPECT_CIPHERTEXT_PART2
#undef EXPECT_PLAINTEXT
#undef UTEST_UPLD_REQ_ID
#undef UTEST_USR_ID
#undef UTEST_ENCRYPT_BLOCK_SZ
#undef UTEST_KEYITEM_RAWDATA
#undef UTEST_KEY_HEX
#undef UTEST_IV_HEX
#undef UTEST_KEY_OCTET
#undef UTEST_IV_OCTET

TestSuite *app_transcoder_crypto_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, atfp_hls_test__crypto__get_valid_key);
    add_test(suite, atfp_hls_test__crypto__key_not_found);
    add_test(suite, atfp_hls_test__crypto__encrypt_doc_id_ok);
    add_test(suite, atfp_hls_test__crypto__encrypt_doc_id_error);
    return suite;
}
