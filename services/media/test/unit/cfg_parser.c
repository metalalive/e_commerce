#include <sysexits.h>
#include <openssl/ssl.h>
#include <sys/resource.h>
#include <h2o.h>
#include <h2o/serverutil.h>
#include <cgreen/cgreen.h>
#include <cgreen/mocks.h>

#include "app_cfg.h"
#include "cfg_parser.h"

Ensure(cfg_pid_file_tests) {
    int       result = 0;
    json_t   *obj = NULL;
    app_cfg_t app_cfg = {.pid_file = NULL};
    result = appcfg_parse_pid_file(NULL, NULL);
    assert_that(result, is_equal_to(EX_CONFIG));
    obj = json_string("/path/to/invalid/not_permitted.pid");
    result = appcfg_parse_pid_file(obj, &app_cfg);
    json_decref(obj);
    assert_that(result, is_equal_to(EX_NOINPUT));
    assert_that(app_cfg.pid_file, is_equal_to(NULL));
    app_load_envvars(&app_cfg.env_vars);
    const char *filename = "./tmp/proc/media_server_test.pid";
    obj = json_string(filename);
    result = appcfg_parse_pid_file(obj, &app_cfg);
    json_decref(obj);
    if (app_cfg.pid_file) {
        fclose(app_cfg.pid_file);
        remove(filename);
    }
    assert_that(result, is_equal_to(EX_OK));
    assert_that(app_cfg.pid_file, is_not_equal_to(NULL));
}

Ensure(cfg_max_conn_tests) {
    int       result = 0;
    json_t   *obj = NULL;
    app_cfg_t app_cfg = {.max_connections = 0};
    obj = json_integer((json_int_t)INT_MAX);
    result = parse_cfg_max_conns(obj, &app_cfg);
    json_decref(obj);
    assert_that(app_cfg.max_connections, is_equal_to(0));
    struct rlimit curr_setting = {.rlim_cur = 0, .rlim_max = 0};
    getrlimit(RLIMIT_NOFILE, &curr_setting);
    curr_setting.rlim_cur = curr_setting.rlim_max + 1;
    obj = json_integer((json_int_t)curr_setting.rlim_cur);
    result = parse_cfg_max_conns(obj, &app_cfg);
    json_decref(obj);
    assert_that(app_cfg.max_connections, is_equal_to(0));
    curr_setting.rlim_cur = curr_setting.rlim_max - 1;
    obj = json_integer((json_int_t)curr_setting.rlim_cur);
    result = parse_cfg_max_conns(obj, &app_cfg);
    json_decref(obj);
    assert_that(app_cfg.max_connections, is_equal_to(curr_setting.rlim_cur));
} // end of cfg_max_conn_tests

// static void _test_gen_x509_cert(EVP_PKEY *pkey, const char *cert_path, int not_before, int
// not_after)
// {
//     FILE *file = NULL;
//     X509 *x509 = X509_new();
//     ASN1_INTEGER_set(X509_get_serialNumber(x509), 1234); // set simple serial number for test
//     X509_gmtime_adj(X509_getm_notBefore(x509), not_before);
//     X509_gmtime_adj(X509_getm_notAfter(x509) , not_after);
//     X509_set_pubkey(x509, pkey);
//     X509_NAME *subj_name = X509_get_subject_name(x509);
//     X509_NAME_add_entry_by_txt(subj_name, "C",  MBSTRING_ASC, (unsigned char *)"TW", -1, -1, 0);
//     X509_NAME_add_entry_by_txt(subj_name, "O",  MBSTRING_ASC, (unsigned char *)"My company Inc.",
//     -1, -1, 0); X509_NAME_add_entry_by_txt(subj_name, "CN", MBSTRING_ASC, (unsigned char
//     *)"localhost", -1, -1, 0); X509_set_issuer_name(x509, subj_name); X509_sign(x509, pkey,
//     NULL); file = fopen(cert_path, "w+"); PEM_write_X509(file, x509); fclose(file);
//     X509_free(x509);
// }

Ensure(cfg_listener_ssl_tests) {
    int result = 0;
    // this test case generates self-signed CA certificate
    const char    *privkey_path = "media/data/certs/test/localhost.private.key";
    const char    *cert_path = "media/data/certs/test/localhost.crt";
    const char    *ciphersuite_list = "TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256";
    const uint16_t tls12 = 0x0303;
    const uint16_t tls13 = 0x0304;
    json_t        *obj = json_object();
    json_object_set_new(obj, "cert_file", json_string(cert_path));
    json_object_set_new(obj, "privkey_file", json_string(privkey_path));
    json_object_set_new(obj, "cipher_suites", json_string(ciphersuite_list));
    struct app_cfg_security_t security = (struct app_cfg_security_t){.ctx = NULL};
    // SSL_CTX is incomplete type in openssl so it cannot be declared as aggregate value, use
    // pointer instead
    char     mock_ssl_ctx_rawbytes[32] = {0};
    SSL_CTX *mock_ssl_ctx = (SSL_CTX *)&mock_ssl_ctx_rawbytes[0];
    // ensure the files do NOT exist
    // remove(privkey_path);
    // remove(cert_path   );
    json_object_set_new(obj, "min_version", json_integer(tls12));
    result = parse_cfg_listener_ssl(&security, obj);
    assert_that(result, is_equal_to(EX_CONFIG));
    json_object_set_new(obj, "min_version", json_integer(tls13));
    { // failed at loading private key
        expect(TLS_server_method);
        expect(SSL_CTX_new, will_return(mock_ssl_ctx));
        expect(SSL_CTX_set_options);
        expect(SSL_CTX_ctrl, will_return(1));
        expect(SSL_CTX_set_session_id_context);
        expect(SSL_CTX_use_PrivateKey_file, will_return(-1)); // assume it returns error
        expect(SSL_CTX_free);
        result = parse_cfg_listener_ssl(&security, obj);
        assert_that(result, is_equal_to(EX_CONFIG));
    }
    { // assume private key exists, mocking openssl won't generate any key
        //// EVP_PKEY *pkey = EVP_PKEY_new();
        //// RSA *rsa = RSA_new();
        //// BIGNUM *e = BN_new();
        //// BN_set_word(e, RSA_F4);
        //// RSA_generate_key_ex(rsa, 2048, e, NULL);
        //// EVP_PKEY_assign_RSA(pkey, rsa);
        //// file = fopen(privkey_path, "w+");
        //// // do NOT pass EVP_des_ede3_cbc() to EVP_CIPHER argument
        //// // , this test case does NOT require to encrypt the PEM file, so disable pass-phrase
        /// prompt / //  (entering the password by human will block automatic testing) /
        /// PEM_write_PrivateKey(file, pkey, NULL, NULL, 0, NULL, NULL); / fclose(file); /
        /// BN_free(e);
        expect(TLS_server_method);
        expect(SSL_CTX_new, will_return(mock_ssl_ctx));
        expect(SSL_CTX_set_options);
        expect(SSL_CTX_ctrl, will_return(1));
        expect(SSL_CTX_set_session_id_context);
        expect(SSL_CTX_use_PrivateKey_file, will_return(1));
        expect(SSL_CTX_use_certificate_chain_file, will_return(-1)); // assume it returns error
        expect(SSL_CTX_free);
        result = parse_cfg_listener_ssl(&security, obj);
        assert_that(result, is_equal_to(EX_CONFIG));
    }
    { // assume the certificate exists but expires
        // _test_gen_x509_cert(pkey, cert_path, -240, -120);  // 2 minutes before now, already
        // expired
        expect(TLS_server_method);
        expect(SSL_CTX_new, will_return(mock_ssl_ctx));
        expect(SSL_CTX_set_options);
        expect(SSL_CTX_ctrl, will_return(1));
        expect(SSL_CTX_set_session_id_context);
        expect(SSL_CTX_use_PrivateKey_file, will_return(1));
        expect(SSL_CTX_use_certificate_chain_file, will_return(1));
        expect(SSL_CTX_get0_certificate);
        expect(X509_get0_notAfter);
        expect(X509_cmp_current_time, will_return(-1)); // assume it returns -1 as expiry hint
        expect(SSL_CTX_free);
        result = parse_cfg_listener_ssl(&security, obj);
        assert_that(result, is_equal_to(EX_CONFIG));
        assert_that(security.ctx, is_equal_to(NULL));
    }
    { // every SSL functions work well
        // _test_gen_x509_cert(pkey, cert_path, -120,  120);  // 2 minutes from now
        expect(TLS_server_method);
        expect(SSL_CTX_new, will_return(mock_ssl_ctx));
        expect(SSL_CTX_set_options);
        expect(SSL_CTX_ctrl, will_return(1));
        expect(SSL_CTX_set_session_id_context);
        expect(SSL_CTX_use_PrivateKey_file, will_return(1));
        expect(SSL_CTX_use_certificate_chain_file, will_return(1));
        expect(SSL_CTX_get0_certificate);
        expect(X509_get0_notAfter);
        expect(X509_cmp_current_time, will_return(1));
        expect(SSL_CTX_set_ciphersuites, will_return(1));
        result = parse_cfg_listener_ssl(&security, obj);
        assert_that(result, is_equal_to(0));
        assert_that(security.ctx, is_equal_to(mock_ssl_ctx));
    }
    // RSA_free(rsa);
    // EVP_PKEY_free(pkey);
    // remove(privkey_path);
    // remove(cert_path   );
    json_decref(obj);
} // end of cfg_listener_ssl_tests

TestSuite *appserver_cfg_parser_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, cfg_pid_file_tests);
    add_test(suite, cfg_max_conn_tests);
    add_test(suite, cfg_listener_ssl_tests);
    return suite;
}
