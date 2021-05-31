#include <stdio.h>
#include <stdlib.h>
#define PY_SSIZE_T_CLEAN
#include <Python.h>

#include <openssl/opensslconf.h>
#include <openssl/bio.h>
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>

#define DEFPRIMES 3
BIO *bio_err;

static int genrsa_cb(int p, int n, BN_GENCB *cb)
{
    return 1;
}


static int openssl_keygen_rsa(int num_bits, size_t *privkey_sz, unsigned char **privkey, size_t *pubkey_sz, unsigned char **pubkey)
{ // deprecated since OpenSSL 3.0
    int ret = 1;
    int primes = DEFPRIMES;
    const BIGNUM *e = NULL; // for returning public exponent from API
    char *hexe = NULL, *dece = NULL; // only for checking different forms of public exponent
    unsigned long f4 = RSA_F4; // default is 0x10001 , 2**n + 1 must be prime
    BIO *privkey_bio = NULL, *pubkey_bio = NULL;

    BN_GENCB *cb = BN_GENCB_new();
    BIGNUM *bn = BN_new();
    RSA *rsa = RSA_new();

    privkey_bio = BIO_new(BIO_s_mem());
    pubkey_bio = BIO_new(BIO_s_mem());

    BN_GENCB_set(cb, genrsa_cb, bio_err);
    int result_setword = BN_set_word(bn, f4);
    int result_keygen = RSA_generate_multi_prime_key(rsa, num_bits, primes, bn, cb);
    if (!result_setword) {
        const char *errmsg = "failed to set word to big number";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
        goto end;
    }
    if (!result_keygen) {
        const char *errmsg = "failed to generate RSA multi-prime key";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
        goto end;
    }
    RSA_get0_key(rsa, NULL, &e, NULL); // retrieve public exponent
    hexe = BN_bn2hex(e);
    dece = BN_bn2dec(e);
    if (hexe && dece) {
        //printf("exponent e is %s (0x%s)\n", dece, hexe);
    }
    if (!PEM_write_bio_RSAPrivateKey(privkey_bio, rsa, NULL, NULL, 0, NULL, NULL)) {
        const char *errmsg = "failed to convert RSA private key to PEM form";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
        goto end;
    }
    if (!PEM_write_bio_RSAPublicKey(pubkey_bio, rsa, NULL, NULL, 0, NULL, NULL)) {
        const char *errmsg = "failed to convert RSA private key to PEM form";
        PyErr_SetString(PyExc_RuntimeError, errmsg);
        goto end;
    }
    // print the retrieved private / public key
    *privkey_sz = BIO_pending(privkey_bio); // pending ? get key size ?
    *pubkey_sz  = BIO_pending(pubkey_bio);
    *privkey = (unsigned char *)malloc(sizeof(char) * (*privkey_sz + 1));
    *pubkey  = (unsigned char *)malloc(sizeof(char) * (*pubkey_sz + 1));
    if(!(privkey) || !(*pubkey)) {
        PyErr_NoMemory();
        goto end;
    }
    BIO_read(privkey_bio, *privkey, *privkey_sz);
    BIO_read(pubkey_bio,  *pubkey,  *pubkey_sz);
    (*privkey)[*privkey_sz] = '\0';
    (*pubkey)[*pubkey_sz] = '\0';
    //printf("retrieved private key: \n %s \n public key: \n %s \n",
    //        *privkey, *pubkey);
    //printf("End of test, Generating RSA private key, %d bit long modulus (%d primes)\n",
    //               num_bits, primes);
    ret = 0;
end:
    OPENSSL_free(hexe);
    OPENSSL_free(dece);
    BN_GENCB_free(cb);
    BN_free(bn);
    RSA_free(rsa);
    BIO_free_all(privkey_bio);
    BIO_free_all(pubkey_bio );
    return ret;
} // end of openssl_keygen_rsa()


static PyObject* Py_RSA_keygen(PyObject *self, PyObject *args) {
    // the first argument `self` points to module object for module-level functions
    size_t  privkey_sz = 0 , pubkey_sz = 0;
    unsigned char *privkey = NULL, *pubkey = NULL; // serializable C variable
    unsigned int   num_bits = 0;
    PyObject *out = NULL;
    if(!PyArg_ParseTuple(args, "I", &num_bits)) {
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
    int result = openssl_keygen_rsa(num_bits, &privkey_sz, &privkey,
           &pubkey_sz, &pubkey);
    if(result) { goto end; }
    // pass tuple of the 2 keys, each key are null-terminated string
    out = Py_BuildValue("ss", privkey, pubkey );
    //out = Py_BuildValue("I", num_bits);
end:
    if(pubkey) {
        free(pubkey);
    }
    if(privkey) {
        free(privkey);
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

//PyMODINIT_FUNC initkeygen(void) { // no longer work in python 3.x
//    printf("loading initkeygen");
//    (void)Py_InitModule("xxxkeygen", py_method_table);
//}
PyMODINIT_FUNC PyInit_keygen(void) {
    return PyModule_Create(&py_module_def) ;
}



//// int main(void) { // for testing purpose
////     unsigned int num_bits = 256 << 3;
////     size_t  privkey_sz = 0 , pubkey_sz = 0;
////     unsigned char *privkey = NULL, *pubkey = NULL; // serializable C variable
////     int result = openssl_keygen_rsa(num_bits, &privkey_sz, &privkey,
////            &pubkey_sz, &pubkey);
////     if(!result && privkey && pubkey) {
////         printf("retrieved private key: \n %s \n public key: \n %s \n",
////             privkey, pubkey);
////     }
////     if(pubkey) {
////         free(pubkey);
////     }
////     if(privkey) {
////         free(privkey);
////     }
////     return 0;
//// }

// openssl genrsa -out rsa_private.pem 2048
// openssl rsa -in rsa_private.pem -outform PEM -pubout -out rsa_public.pem

// gcc -c -Wint-to-pointer-cast  -Wpointer-to-int-cast  -pthread  -Wall -fdata-sections -ffunction-sections -Wint-to-pointer-cast  -g -gdwarf-2 -Wa,-a,-ad -I/usr/local/include/ ./common/util/c/keygen.c -o tmp/keygen.o

// the order of object file and other options affects linking result .... wierd
// gcc ./tmp/keygen.o  -L/usr/local/lib  -lcrypto -lssl  -o ./tmp/keygen.out

