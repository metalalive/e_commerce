#include <cgreen/mocks.h>
#include <curl/curl.h>

char *curl_easy_unescape(CURL *handle, const char *string, int length, int *outlength) {
    return (char *)mock(handle, string, length, outlength);
}
