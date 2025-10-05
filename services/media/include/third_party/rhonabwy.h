#ifndef MEIDA__THIRDPARTY_RHONABWY_H
#define MEIDA__THIRDPARTY_RHONABWY_H
#ifdef __cplusplus
extern "C" {
#endif

#include <rhonabwy.h>

typedef struct {
    char *ca_path;
    char *ca_format; // "PEM" or "DES"
    int   flags;     // original x5u_flags is moved here
} app_x5u_t;

// cloned from r_jwks_import_from_uri(...)  in Rhonabwy, replace the argument x5u_flags
// with the pointer to struct app_x5u_t because this application requires to set up more
//  detail when sending HTTPS request.
int DEV_r_jwks_import_from_uri(jwks_t *jwks, const char *uri, app_x5u_t *x5u);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEIDA__THIRDPARTY_RHONABWY_H
