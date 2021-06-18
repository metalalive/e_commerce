#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#define PY_SSIZE_T_CLEAN
#ifndef SKIP_CPYTHON
#include <Python.h>
#endif

#include <openssl/opensslconf.h>
#include <openssl/bio.h>
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>

BIO *bio_err;

typedef struct _rsa_extra_prime {
    char *r;
    char *crt_p;
    char *crt_e;
} rsa_extra_prime_t;

typedef struct _rsa_privkey {
    char *e;
    char *n;
    char *d;
    char *p;
    char *q;
    char *dp;
    char *dq;
    char *qp;
    rsa_extra_prime_t *extra_primes;
} rsa_privkey_t;

typedef struct _rsa_pubkey {
    char *e;
    char *n;
} rsa_pubkey_t;

static int genrsa_cb(int p, int n, BN_GENCB *cb)
{
    return 1;
}

static char*  openssl_bignum_to_decimal(const BIGNUM *bn) {
    char *dec_bn = NULL; // only for checking different forms of given exponent
    dec_bn = BN_bn2dec(bn);
    if (dec_bn) {
#ifdef APP_DEBUG
        printf("RSA component is %s \n", dec_bn);
#endif
    }
    // The string returned by BN_bn2hex() or BN_bn2dec() must be
    // freed later using OPENSSL_free()
    //OPENSSL_free(dec_bn);
    return dec_bn;
}


static int openssl_keygen_rsa(int num_bits, int num_primes, RSA *rsa)
{ // deprecated since OpenSSL 3.0
    int ret = 1;
    unsigned long f4 = RSA_F4; // default is 0x10001 , 2**n + 1 must be prime
    BN_GENCB *cb = BN_GENCB_new();
    BIGNUM *bn = BN_new();

    BN_GENCB_set(cb, genrsa_cb, bio_err);
    int result_setword = BN_set_word(bn, f4);
    int result_keygen = RSA_generate_multi_prime_key(rsa, num_bits, num_primes, bn, cb);
    if (!result_setword) {
#ifndef SKIP_CPYTHON
        const char *errmsg = "failed to set word to big number";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
#endif
        goto end;
    }
    if (!result_keygen) {
#ifndef SKIP_CPYTHON
        const char *errmsg = "failed to generate RSA multi-prime key";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
#endif
        goto end;
    }
    ret = 0;
end:
    BN_GENCB_free(cb);
    BN_free(bn);
    return ret;
} // end of openssl_keygen_rsa()


static int extract_rsa_components(const RSA *rsa, rsa_privkey_t  *privkey, rsa_pubkey_t  *pubkey) {
    const BIGNUM *e = NULL, *n = NULL, *d = NULL;
    const BIGNUM *dmp1 = NULL, *dmq1 = NULL, *iqmp = NULL;
    const BIGNUM *p = NULL, *q = NULL;
    int result = 0;
    int idx = 0;
    // the variables point to internal structure of a generated RSA key, these RSA components
    // (once pointed) should NOT be freed by caller.
    RSA_get0_key(rsa, &n, &e, &d);
    RSA_get0_factors(rsa, &p, &q);
    RSA_get0_crt_params(rsa, &dmp1, &dmq1, &iqmp);
    pubkey->n = openssl_bignum_to_decimal(n);
    pubkey->e = openssl_bignum_to_decimal(e);
    privkey->n  = openssl_bignum_to_decimal(n);
    privkey->e  = openssl_bignum_to_decimal(e);
    privkey->d  = openssl_bignum_to_decimal(d);
#ifdef APP_DEBUG
    printf("------- p,q ------------ \n");
#endif
    privkey->p  = openssl_bignum_to_decimal(p);
    privkey->q  = openssl_bignum_to_decimal(q);
#ifdef APP_DEBUG
    printf("------- dp, dq, qi ------------ \n");
#endif
    privkey->dp = openssl_bignum_to_decimal(dmp1);
    privkey->dq = openssl_bignum_to_decimal(dmq1);
    privkey->qp = openssl_bignum_to_decimal(iqmp);

    int version = RSA_get_version(rsa);
    int num_extra_primes = RSA_get_multi_prime_extra_count(rsa);
    if(version == RSA_ASN1_VERSION_DEFAULT) {
#ifdef APP_DEBUG
        printf("rsa version : RSA_ASN1_VERSION_DEFAULT \n");
#endif
    } else  if(version == RSA_ASN1_VERSION_MULTI && num_extra_primes > 0) {
#ifdef APP_DEBUG
        printf("rsa version : RSA_ASN1_VERSION_MULTI \n");
#endif
        const BIGNUM *primes[num_extra_primes];
        const BIGNUM *crt_exps[num_extra_primes];
        const BIGNUM *crt_params[num_extra_primes];
        result = RSA_get0_multi_prime_factors(rsa, primes);
        if(!result) {
#ifndef SKIP_CPYTHON
            const char *errmsg = "failed to retrieve extra primes from given RSA key";
            PyErr_SetString(PyExc_RuntimeError, errmsg);
#else
#ifdef APP_DEBUG
            printf("RSA_get0_multi_prime_factors() failure \n");
#endif
#endif
            goto end;
        }
        result = RSA_get0_multi_prime_crt_params(rsa, crt_exps, crt_params);
        if(!result) {
#ifndef SKIP_CPYTHON
            const char *errmsg = "failed to retrieve extra primes from given RSA key";
            PyErr_SetString(PyExc_RuntimeError, errmsg);
#else
#ifdef APP_DEBUG
            printf("RSA_get0_multi_prime_crt_params() failure \n");
#endif
#endif
            goto end;
        }
#ifdef APP_DEBUG
        printf("------- multi-primes, extra CRT params ------------ \n");
#endif
        privkey->extra_primes = (rsa_extra_prime_t *)malloc(num_extra_primes * sizeof(rsa_extra_prime_t));
        for(idx = 0; idx < num_extra_primes; idx++) {
            rsa_extra_prime_t *ep = &privkey->extra_primes[idx];
            ep->r   = openssl_bignum_to_decimal(primes[idx]);
            ep->crt_p = openssl_bignum_to_decimal(crt_params[idx]);
            ep->crt_e = openssl_bignum_to_decimal(crt_exps[idx]);
        }
    }
end:
    return 0;
} // end of extract_rsa_components


static void _rsa_free_copied_components(rsa_privkey_t  *privkey, rsa_pubkey_t  *pubkey, int num_extra_primes) {
    int idx = 0;
    if(privkey && privkey->d) {
        OPENSSL_free(privkey->n);
        OPENSSL_free(privkey->e);
        OPENSSL_free(privkey->d);
        OPENSSL_free(privkey->p );
        OPENSSL_free(privkey->q );
        OPENSSL_free(privkey->dp);
        OPENSSL_free(privkey->dq);
        OPENSSL_free(privkey->qp);
        if(privkey->extra_primes) {
            for(idx = 0; idx < num_extra_primes; idx++) {
                rsa_extra_prime_t *ep = &privkey->extra_primes[idx];
                OPENSSL_free(ep->r);
                OPENSSL_free(ep->crt_p);
                OPENSSL_free(ep->crt_e);
            }
            memset((void*)privkey->extra_primes, 0x0, sizeof(rsa_extra_prime_t) * num_extra_primes);
            free(privkey->extra_primes);
        }
        memset((void*)privkey, 0x0, sizeof(rsa_privkey_t));
    }
    if(pubkey && pubkey->e) {
        OPENSSL_free(pubkey->e);
        OPENSSL_free(pubkey->n);
        memset((void*)pubkey, 0x0, sizeof(rsa_pubkey_t));
    }
} // end of _rsa_free_copied_components


#ifndef SKIP_CPYTHON
static PyObject* Py_RSA_keygen(PyObject *self, PyObject *args) {
    // the first argument `self` points to module object for module-level functions
    rsa_privkey_t  privkey = {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL};
    rsa_pubkey_t   pubkey = {NULL, NULL};
    unsigned int   num_bits = 0;
    int num_primes = -1;
    PyObject *out = NULL, *py_pubkey_dict=NULL, *py_privkey_dict=NULL, *py_extra_primes = NULL;
    RSA *rsa = NULL;
    int idx = 0;

    if(!PyArg_ParseTuple(args, "Ii", &num_bits, &num_primes)) {
        const char *errmsg = "error when parsing arguments";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
        goto end;
    }
    if(num_bits < OPENSSL_RSA_FIPS_MIN_MODULUS_BITS || num_bits > OPENSSL_RSA_MAX_MODULUS_BITS) {
        char  errmsg[64];
        const char *format = "number of bits has to range between %d and %d \0";
        sprintf(&errmsg[0], format, OPENSSL_RSA_FIPS_MIN_MODULUS_BITS, OPENSSL_RSA_MAX_MODULUS_BITS);
        PyErr_SetString(PyExc_ValueError, errmsg);
        goto end;
    }
    int num_extra_primes = num_primes - RSA_DEFAULT_PRIME_NUM;
    if(num_primes < RSA_DEFAULT_PRIME_NUM) {
        char  errmsg[80];
        const char *format = "number of primes must be at least %d, but receive %d \0";
        sprintf(&errmsg[0], format, RSA_DEFAULT_PRIME_NUM, num_primes);
        PyErr_SetString(PyExc_ValueError, errmsg);
        goto end;
    }
    rsa = RSA_new();
    int result = openssl_keygen_rsa(num_bits, num_primes, rsa);
    if(result) { goto end; }
    result = extract_rsa_components((const RSA *)rsa, &privkey, &pubkey);
    if(result) { goto end; }
    // pass dictionary of the 2 keys, all components of each key in nested dict
    py_pubkey_dict  = Py_BuildValue("{s:s,s:s}", "n", pubkey.n, "e", pubkey.e);
    // PyObject * py_privkey_extra_dicts[num_extra_primes]; // this will cause strange compile error
    if(num_extra_primes > 0 && privkey.extra_primes) {
        py_extra_primes = PyList_New((Py_ssize_t)0x0);
        for(idx = 0; idx < num_extra_primes; idx++) {
            rsa_extra_prime_t *ep = &privkey.extra_primes[idx];
            PyObject *py_extra_prime = PyDict_New();
            PyDict_SetItemString(py_extra_prime, "r"    , PyUnicode_FromString(ep->r    ));
            PyDict_SetItemString(py_extra_prime, "crt_p", PyUnicode_FromString(ep->crt_p));
            PyDict_SetItemString(py_extra_prime, "crt_e", PyUnicode_FromString(ep->crt_e));
            PyList_Append(py_extra_primes, py_extra_prime);
        }
        // va_list has to work under function with variadic arguments
        // there is no stardard way to dynamically product va_list from scratch
        //va_list extra_prime_args;
        //va_start(extra_prime_args, the_argument_before_variadic_arguments);
        //py_extra_primes = Py_VaBuildValue(&py_extraprimes_fmt[0], extra_prime_args);
        //va_end(extra_prime_args);
    } else {
        py_extra_primes = Py_None; //Py_BuildValue("s", NULL);
    }
    py_privkey_dict = Py_BuildValue("{s:s,s:s,s:s,s:s,s:s,s:s,s:s,s:s,s:O}", "n", privkey.n,
            "e", privkey.e, "d", privkey.d, "p", privkey.p, "q", privkey.q, "dp", privkey.dp,
            "dq", privkey.dq, "qp", privkey.qp, "extra_primes", py_extra_primes);
    out = Py_BuildValue("{s:O,s:O}", "private", py_privkey_dict, "public", py_pubkey_dict);
    ////out = Py_BuildValue("{s:s,s:i}", "private", "ir823", "public", -87);
end:
    _rsa_free_copied_components(&privkey, &pubkey, num_extra_primes);
    if(rsa) {
        RSA_free(rsa);
    }
    return out;
} // end of Py_RSA_keygen()


static PyMethodDef py_method_table[] = {
    {
        "RSA_keygen",  // method name which can be invoked at python level code
        Py_RSA_keygen, // method implementation in C extension
        METH_VARARGS,  // flag to expect positional arguments passed by python caller.
        "generate RSA key pair" // comment ?
    },
    {NULL, NULL, 0 , NULL} // sentinel
};

// module definition structure
static struct PyModuleDef py_module_def = {
    PyModuleDef_HEAD_INIT,
    "keygen", // name of this module (TODO:include package ?)
    NULL, // no document provided at python level
    -1, // memory size for storing module state, which can be put in:
        // * per-module memory area, if multiple sub-intepreters are in use
        // * static global area (?), so not support multiple sub-intepreters
        // -1 means "multiple sub-intepreters not supported"
        // https://docs.python.org/3/c-api/module.html#c.PyModuleDef.m_size
    py_method_table // the method table above
};

PyMODINIT_FUNC PyInit_keygen(void) {
    return PyModule_Create(&py_module_def) ;
}
#endif // end of not SKIP_CPYTHON


#ifdef SKIP_CPYTHON
int main(void) { // for testing purpose
    rsa_privkey_t  privkey = {NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL};
    rsa_pubkey_t   pubkey = {NULL, NULL};
    unsigned int num_bits = 128 << 3;
    // 3 is suggested, currently roll back to typical 2-prime RSA key
    int num_primes = 3; // RSA_DEFAULT_PRIME_NUM
    RSA *rsa = RSA_new();
    int result = openssl_keygen_rsa(num_bits, num_primes, rsa);
#ifdef APP_DEBUG
    printf("openssl_keygen_rsa result: %d \n", result);
#endif
    if(result) { goto end; }
    result = extract_rsa_components((const RSA *)rsa, &privkey, &pubkey);
#ifdef APP_DEBUG
    printf("extract_rsa_components result: %d \n", result);
#endif
end:
    _rsa_free_copied_components(&privkey, &pubkey, num_primes - RSA_DEFAULT_PRIME_NUM);
    RSA_free(rsa);
    return 0;
}
#endif // end of SKIP_CPYTHON

// openssl genrsa -out rsa_private.pem 2048
// openssl rsa -in rsa_private.pem -outform PEM -pubout -out rsa_public.pem

//rm -rf ./tmp/keygen.o ; gcc -c -Wint-to-pointer-cast  -Wpointer-to-int-cast  -pthread  -Wall  -g -gdwarf-2 -DSKIP_CPYTHON -DAPP_DEBUG -I/usr/local/include/ ./common/util/c/keygen.c -o tmp/keygen.o 

// the order of object file and other options affects linking result .... wierd
// gcc ./tmp/keygen.o  -L/usr/local/lib  -lcrypto -lssl  -o ./tmp/keygen.out

// memory-leak check
// valgrind --leak-check=full --show-leak-kinds=all   ./tmp/keygen.out

