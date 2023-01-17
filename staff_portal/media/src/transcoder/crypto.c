#include <time.h>
#include "base64.h"
#include "datatypes.h"
#include "transcoder/file_processor.h"

// build path for cacheable encrypted files
size_t  atfp_get_encrypted_file_basepath (const char *basepath, char *out, size_t o_sz,
        const char *doc_id, size_t id_sz) {
    size_t  nwrite = 0;
#define  PATTERN  "%s/%s/%s"
    size_t  expect_out_sz = strlen(basepath) + 1 + sizeof(ATFP_CACHED_FILE_FOLDERNAME)
                 + 1 + strlen(doc_id) + 1;
    if (o_sz >= expect_out_sz)
        nwrite = snprintf(out, o_sz, PATTERN, basepath, ATFP_CACHED_FILE_FOLDERNAME, doc_id);
    return nwrite;
#undef   PATTERN
} // end of atfp_get_encrypted_file_basepath


const char * atfp_get_crypto_key (json_t *_keyinfo, const char *_key_id, json_t **_item_out)
{
    if(!_keyinfo || !_key_id || !_item_out)
        return NULL;
    const char *valid_id = NULL;
    int ret = 0;
    time_t  most_recent_ts = 0;
    *_item_out = NULL;
    ret = strncmp(_key_id, ATFP__CRYPTO_KEY_MOST_RECENT, sizeof(ATFP__CRYPTO_KEY_MOST_RECENT) - 1);
    if(ret) { // find specific ID
        *_item_out = json_object_get(_keyinfo, _key_id);
        if(*_item_out)
            valid_id = _key_id;
    } else { // find the most recent ID
        json_t  *kitem = NULL;
        json_object_foreach(_keyinfo, _key_id, kitem) {
            // not good implementation, but it is required to store timestamp 
            json_t  *ts_obj = json_object_get(kitem, "timestamp");
            if(!ts_obj) {
                valid_id = NULL;
                *_item_out = NULL;
                break;
            }
            // NOTE,not good coding style, time_t is not necessarily a 64-bit integer
            time_t  _ts = (time_t) json_integer_value(ts_obj);
            if(difftime(_ts, most_recent_ts) > 0) {
                most_recent_ts = _ts;
                valid_id = _key_id;
                *_item_out = kitem;
            }
        } // end of key item iteration
    }
    return valid_id;
} // end of atfp_get_crypto_key


int  atfp_encrypt_document_id (EVP_CIPHER_CTX *ctx, atfp_data_t *fp_data, json_t *kitem, unsigned char **out, size_t *out_sz)
{ // the RFC8216 only accpet aes-128-cbc 
    int success = 0;
    unsigned char *ciphertext = NULL,  *key_buf = NULL, *iv_buf = NULL;
    if(!fp_data || !kitem || !out || !out_sz || fp_data->usr_id == 0
            || fp_data->upld_req_id == 0)
        goto done;
    json_t *key_obj = json_object_get(kitem, "key");
    json_t *iv_obj  = json_object_get(kitem, "iv");
    if(!key_obj || !iv_obj)
        goto done;
    size_t  nbytes_key = (size_t) json_integer_value(json_object_get(key_obj, "nbytes"));
    size_t  nbytes_iv  = (size_t) json_integer_value(json_object_get(iv_obj, "nbytes"));
    success = EVP_CIPHER_CTX_set_key_length(ctx, nbytes_key);
    if(!success)
        goto done;
    {
        long  nwrite_key = 0, nwrite_iv = 0;
        const char *key_hex = json_string_value(json_object_get(key_obj, "data"));
        const char *iv_hex  = json_string_value(json_object_get(iv_obj, "data"));
        key_buf = OPENSSL_hexstr2buf (key_hex, &nwrite_key);
        iv_buf  = OPENSSL_hexstr2buf (iv_hex , &nwrite_iv);
        if(!key_buf || !iv_buf || nwrite_key!=nbytes_key || nwrite_iv!=nbytes_iv )
            goto done;
        success = EVP_EncryptInit_ex (ctx, NULL, NULL, (const unsigned char *)&key_buf[0], 
                (const unsigned char *)&iv_buf[0]);
        if(!success)
            goto done;
    }
    int  ct_sz = 0, num_encrypt = 0,  max_ct_sz = 0;
    { // prepare plaintext
#define  PATTERN   "%d/%08x"
        size_t  pre_cal_pt_sz = sizeof(PATTERN) + USR_ID_STR_SIZE + UPLOAD_INT2HEX_SIZE(fp_data->upld_req_id);
        char plaintext[pre_cal_pt_sz];
        size_t  pt_sz = snprintf(&plaintext[0], pre_cal_pt_sz, PATTERN, fp_data->usr_id,
                    fp_data->upld_req_id );
        int  block_sz = EVP_CIPHER_CTX_block_size(ctx);
        max_ct_sz = pt_sz - (pt_sz % block_sz) + block_sz + 1;
        ciphertext = calloc(max_ct_sz, sizeof(unsigned char));
        success = EVP_EncryptUpdate(ctx, ciphertext, &num_encrypt,
                (const unsigned char *)&plaintext[0], pt_sz);
        if(!success)
            goto done;
#undef   PATTERN
    } // TODO, what if the crypto algorithm supports AEAD / MAC ??
    ct_sz = num_encrypt;
    success = EVP_EncryptFinal_ex(ctx, &ciphertext[ct_sz], &num_encrypt);
    if(!success)
        goto done;
    ct_sz += num_encrypt;
    assert(max_ct_sz >= ct_sz);
    {
        unsigned char  *_out = NULL; size_t  _out_sz = 0;
        _out = base64_encode((const unsigned char *)ciphertext, (size_t)ct_sz, &_out_sz);
        if(_out && _out_sz > 0) {
            // for small data, the new-line char generated from base64 encoder is present only at
            // the end of the output, it should be safe to remove the new-line char
            _out[--_out_sz] = 0;
        }
        *out_sz = _out_sz;
        *out    = _out;
    }
done:
    if(iv_buf)
        OPENSSL_free(iv_buf);
    if(key_buf)
        OPENSSL_free(key_buf); 
    if(ciphertext)
        free(ciphertext);
    return  success;
} // end of atfp_encrypt_document_id

