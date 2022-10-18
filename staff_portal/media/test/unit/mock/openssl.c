#include <cgreen/mocks.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/ssl.h>
#include <openssl/rsa.h>
#include <openssl/sha.h>
#include <openssl/x509.h>

int OPENSSL_init_ssl(uint64_t opts, const OPENSSL_INIT_SETTINGS *settings)
{ return (int)mock(opts, settings); }

void OPENSSL_cleanse(void *ptr, size_t len)
{ mock(ptr, len); }

const SSL_METHOD *TLS_server_method(void)
{ return (const SSL_METHOD *)mock(); }

SSL_CTX *SSL_CTX_new(const SSL_METHOD *meth)
{ return (SSL_CTX *)mock(meth); }

void SSL_CTX_free(SSL_CTX *ctx)
{ mock(ctx); }

EVP_PKEY *EVP_PKEY_new(void)
{ return (EVP_PKEY *)mock(); }

void EVP_PKEY_free(EVP_PKEY *pkey)
{ mock(pkey); }

int EVP_PKEY_assign(EVP_PKEY *pkey, int type, void *key)
{ return (int)mock(pkey, type, key); }

unsigned long SSL_CTX_set_options(SSL_CTX *ctx, unsigned long op)
{ return (unsigned long)mock(ctx, op); }

int SSL_CTX_set_ciphersuites(SSL_CTX *ctx, const char *str)
{ return (int)mock(ctx, str); }

int SSL_CTX_use_certificate_chain_file(SSL_CTX *ctx, const char *filepath)
{ return (int)mock(ctx, filepath); }

int SSL_CTX_use_PrivateKey_file(SSL_CTX *ctx, const char *filepath, int key_type)
{ return (int)mock(ctx, filepath, key_type); }

X509 *SSL_CTX_get0_certificate(const SSL_CTX *ctx)
{ return (X509 *)mock(ctx); }

int SSL_CTX_set_session_id_context(SSL_CTX *ctx, const unsigned char *sid_ctx, unsigned int sid_ctx_len)
{ return (int)mock(ctx, sid_ctx, sid_ctx_len); }

long SSL_CTX_ctrl(SSL_CTX *ctx, int cmd, long larg, void *parg)
{ return (long)mock(ctx,cmd,larg, parg); }

int RSA_generate_key_ex(RSA *rsa, int bits, BIGNUM *e, BN_GENCB *cb)
{ return (int)mock(rsa, bits, e, cb); }

ASN1_TIME *X509_getm_notBefore(const X509 *xobj)
{ return (ASN1_TIME *)mock(xobj); }

ASN1_TIME *X509_getm_notAfter(const X509 *xobj)
{ return (ASN1_TIME *)mock(xobj); }

const ASN1_TIME *X509_get0_notAfter(const X509 *xobj)
{ return (const ASN1_TIME *)mock(xobj); }

X509_NAME *X509_get_subject_name(const X509 *xobj)
{ return (X509_NAME *)mock(xobj); }

ASN1_INTEGER *X509_get_serialNumber(X509 *x)
{ return (ASN1_INTEGER *)mock(x); }

int X509_NAME_add_entry_by_txt(X509_NAME *name, const char *field, int type,
                               const unsigned char *bytes, int len, int loc,
                               int set)
{ return (int)mock(name, field, type, bytes, len, loc, set); }

int X509_set_pubkey(X509 *x, EVP_PKEY *pkey)
{ return (int)mock(x, pkey); }

int X509_set_issuer_name(X509 *x, X509_NAME *name)
{ return (int)mock(x, name); }

int X509_cmp_current_time(const ASN1_TIME *s)
{ return (int)mock(s); }

ASN1_TIME *X509_gmtime_adj(ASN1_TIME *s, long adj)
{ return (ASN1_TIME *)mock(s, adj); }

int X509_sign(X509 *x, EVP_PKEY *pkey, const EVP_MD *md)
{ return (int)mock(x, pkey, md); }

int SHA1_Init(SHA_CTX *ctx)
{ return (int)mock(ctx); }

int SHA1_Update(SHA_CTX *ctx, const void *data, size_t len)
{ return (int)mock(ctx, data, len); }

int SHA1_Final(unsigned char *md, SHA_CTX *ctx)
{ return (int)mock(md, ctx); }

BIGNUM *BN_new(void)
{ return (BIGNUM *)mock(); }

void BN_free(BIGNUM *a)
{ mock(a); }

int BN_set_word(BIGNUM *a, BN_ULONG w)
{ return (int) mock(a,w); }

char *BN_bn2hex(const BIGNUM *a)
{ return (char *) mock(a); }

int BN_rand(BIGNUM *a, int bits, int top, int bottom)
{ return (int) mock(a, bits, top, bottom); }

int ASN1_INTEGER_set(ASN1_INTEGER *a, long v)
{ return (int) mock(a,v); }



RSA *RSA_new(void)
{ return (RSA *)mock(); }

X509 *X509_new(void)
{ return (X509 *)mock(); }

void X509_free(X509 *x)
{ mock(x); }

int PEM_write_PrivateKey(FILE *fp, EVP_PKEY *x, const EVP_CIPHER *enc,
                         unsigned char *kstr, int klen,
                         pem_password_cb *cb, void *u)
{ return (int)mock(fp, x, enc, kstr, klen, cb, u); }

int PEM_write_X509(FILE *fp, X509 *x)
{ return (int)mock(fp, x); }

unsigned long ERR_get_error(void)
{ return (unsigned long)mock(); }

void ERR_error_string_n(unsigned long e, char *buf, size_t len)
{ mock(e, buf, len); }

