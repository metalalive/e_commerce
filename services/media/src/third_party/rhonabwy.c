#include <assert.h>
#include <orcania.h>
#include <yder.h>
#include <h2o.h>
#include "third_party/rhonabwy.h"

#ifdef R_WITH_CURL
#include <curl/curl.h>
#include <string.h>
#define _R_HEADER_CONTENT_TYPE "Content-Type"

struct _r_response_str {
  char * ptr;
  char * ptr_bak; // FIXME : figure out when data corruption occurs ?
  size_t len;
};

struct _r_expected_content_type {
  const char * expected;
  int found;
};


static size_t write_response(char *ptr, size_t size, size_t nmemb, void * userdata) {
  struct _r_response_str * resp = (struct _r_response_str *)userdata;
  size_t len = (size*nmemb);
  resp->ptr = o_realloc(resp->ptr, (resp->len + len + 1));
  if (resp->ptr != NULL) {
    memcpy(resp->ptr+resp->len, ptr, len);
    resp->len += len;
    resp->ptr[resp->len] = '\0';
    return len;
  } else {
    return 0;
  }
}

static size_t write_header(void * buffer, size_t size, size_t nitems, void * user_data) {
  const char * header = (const char *)buffer;
  struct _r_expected_content_type * expected_content_type = (struct _r_expected_content_type *)user_data;
  
  if (o_strncasecmp(header, _R_HEADER_CONTENT_TYPE, o_strlen(_R_HEADER_CONTENT_TYPE)) == 0 &&
     o_strstr(header+o_strlen(_R_HEADER_CONTENT_TYPE)+1, expected_content_type->expected)) {
    expected_content_type->found = 1;
  }
  return nitems * size;
}
#endif // end of R_WITH_CURL


static char * DEV_r_get_http_content(const char * url, app_x5u_t *x5u, const char * expected_content_type) {
  char * to_return = NULL;
#ifdef R_WITH_CURL
  CURL *curl;
  struct curl_slist *list = NULL;
  struct _r_response_str resp = {.ptr = NULL, .len = 0};
  struct _r_expected_content_type ct = {.found = 0, .expected = expected_content_type};
  int status = 0;

  curl = curl_easy_init();
  if(curl != NULL) {
    do {
      if (curl_easy_setopt(curl, CURLOPT_URL, url) != CURLE_OK) {
        break;
      }
      if (curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_response) != CURLE_OK) {
        break;
      }
      if (curl_easy_setopt(curl, CURLOPT_WRITEDATA, &resp) != CURLE_OK) {
        break;
      }
      if ((list = curl_slist_append(list, "User-Agent: Rhonabwy/" RHONABWY_VERSION_STR)) == NULL) {
        break;
      }
      if (curl_easy_setopt(curl, CURLOPT_HTTPHEADER, list) != CURLE_OK) {
        h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
        break;
      }
      if (curl_easy_setopt(curl, CURLOPT_NOPROGRESS, 1L) != CURLE_OK) {
        break;
      }
      if (x5u->flags & R_FLAG_FOLLOW_REDIRECT) {
        if (curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1) != CURLE_OK) {
          break;
        }
      }
      if (x5u->flags & R_FLAG_IGNORE_SERVER_CERTIFICATE) {
          // this could happen in some cases e.g. TLS v1.3 ticket resumption
          // and use of valid pre-shared key in subsequent connection,
          // so peers can skip verifying each other's certificate
        if (curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0) != CURLE_OK) {
          h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
          break;
        }
        if (curl_easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 0) != CURLE_OK) {
          h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
          break;
        }
      } else { // by default , this server only verifies cert from auth server
          curl_easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 1L);
          // server sends CertificateRequest in TLS 1.3 handshake
          curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
      }
      if (curl_easy_setopt(curl, CURLOPT_TIMEOUT, 5L) != CURLE_OK) {
        h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
        break;
      }
      if(x5u->ca_path) {
          if (curl_easy_setopt(curl, CURLOPT_CAPATH, x5u->ca_path) == CURLE_OK) {
              curl_easy_setopt(curl, CURLOPT_SSLCERTTYPE, x5u->ca_format);
              curl_easy_setopt(curl, CURLOPT_SSL_ENABLE_ALPN, 1L); // forced to be HTTP/2
              curl_easy_setopt(curl, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_2TLS);
          } else {
              h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
              break;
          }
      }
      if (o_strlen(expected_content_type)) {
        if (curl_easy_setopt(curl, CURLOPT_HEADERFUNCTION, write_header) != CURLE_OK) {
          h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
          break;
        }
        if (curl_easy_setopt(curl, CURLOPT_WRITEHEADER, &ct) != CURLE_OK) {
          h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
          break;
        }
      }
      if (curl_easy_perform(curl) != CURLE_OK) {
        break;
      }
      if (curl_easy_getinfo (curl, CURLINFO_RESPONSE_CODE, &status) != CURLE_OK) {
        break;
      }
    } while (0);

    curl_easy_cleanup(curl);
    curl_slist_free_all(list);
    
    if (status >= 200 && status < 300) {
      if (!o_strlen(expected_content_type)) {
        to_return = resp.ptr;
      } else {
        if (ct.found) {
          to_return = resp.ptr;
        } else {
          o_free(resp.ptr);
        }
      }
    } else {
      h2o_error_printf("[3pty][rhonabwy] line: %d, status:%d \n", __LINE__, status);
      o_free(resp.ptr);
    }
  }
#else
  (void)url;
  (void)x5u;
  (void)expected_content_type;
  h2o_error_printf("[3pty][rhonabwy] line: %d \n", __LINE__);
#endif
  return to_return;
} // end of DEV_r_get_http_content


int DEV_r_jwks_import_from_uri(jwks_t * jwks, const char * uri, app_x5u_t *x5u) {
  int ret;
  json_t * j_result = NULL;
  char * x5u_content = NULL;

  if (jwks != NULL && uri != NULL) {
    if ((x5u_content = DEV_r_get_http_content(uri, x5u, "application/json")) != NULL) {
      j_result = json_loads(x5u_content, JSON_DECODE_ANY, NULL);
      if (j_result != NULL) {
        ret = r_jwks_import_from_json_t(jwks, j_result);
      } else {
        h2o_error_printf("[3pty][rhonabwy] line: %d, Error DEV_r_get_http_content\n", __LINE__);
        ret = RHN_ERROR;
      }
      json_decref(j_result);
      o_free(x5u_content);
    } else {
      h2o_error_printf("[3pty][rhonabwy] line: %d, x5u - Error getting x5u content\n", __LINE__);
      ret = RHN_ERROR;
    }
  } else {
    ret = RHN_ERROR_PARAM;
  }
  return ret;
} // end of DEV_r_jwks_import_from_uri
