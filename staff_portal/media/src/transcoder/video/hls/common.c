#include <openssl/evp.h>

#include "storage/cfg_parser.h"
#include "transcoder/video/hls.h"
#include "transcoder/video/ffmpeg.h"

atfp_t  *atfp__video_hls__instantiate(void) {
    // at this point, `atfp_av_ctx_t` should NOT be incomplete type
    size_t tot_sz = sizeof(atfp_hls_t) + sizeof(atfp_av_ctx_t);
    atfp_hls_t  *out = calloc(0x1, tot_sz);
    char *ptr = (char *)out + sizeof(atfp_hls_t);
    out->av = (atfp_av_ctx_t *) ptr;
    out->asa_local.super.storage = app_storage_cfg_lookup("localfs") ; 
    return &out->super;
} // end of atfp__video_hls__instantiate

uint8_t    atfp__video_hls__label_match(const char *label) {
    const char *exp_labels[2] = {"hls", "application/x-mpegURL"};
    return atfp_common__label_match(label, 2, exp_labels);
}


int  atfp_hls__encrypt_document_id (atfp_data_t *fp_data, json_t *kitem, unsigned char **out, size_t *out_sz)
{
    int success = 0;
    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    // NOTE, there is alternative cipher named  EVP_aes_128_cbc_hmac_sha1(), unfortunately it is
    // only intended for usage of TLS protocol.
    success = EVP_EncryptInit_ex (ctx, EVP_aes_128_cbc(), NULL, NULL, NULL);
    if(!success)
        goto done;
    json_t *iv_obj  = json_object_get(kitem, "iv");
    size_t  nbytes_iv  = (size_t) json_integer_value(json_object_get(iv_obj, "nbytes"));
    if(nbytes_iv != HLS__NBYTES_IV)
        goto done;
#if  0 // not used for AES-CBC
    success = EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, nbytes_iv, NULL);
    if(!success)
        goto done;
#endif
    success = atfp_encrypt_document_id (ctx, fp_data, kitem, out, out_sz);
done:
    if(ctx)
        EVP_CIPHER_CTX_free(ctx);
    return success;
} // end of atfp_hls__encrypt_document_id

