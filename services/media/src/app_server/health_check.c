#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <getopt.h>
#include <curl/curl.h>

// Dummy write callback function to discard response body data
size_t write_callback(void *contents, size_t size, size_t nmemb, void *userp) {
    (void)contents; // Suppress unused parameter warning
    (void)userp;    // Suppress unused parameter warning
    return size * nmemb;
}

int main(int argc, char *argv[]) {
    CURL    *curl = NULL;
    CURLcode res;
    long     http_code = 0; // Initialize to a non-success value

    int   timeout_secs = 0, port = 0, opt;
    char *host = NULL, *uri_path = NULL, *cert_path = NULL;

    static struct option long_options[] = {
        {"timeout-secs", required_argument, 0, 't'},
        {"port", required_argument, 0, 'p'},
        {"host", required_argument, 0, 'h'},
        {"uri-path", required_argument, 0, 'u'},
        {"cert-path", required_argument, 0, 'c'}, // New argument for certificate path
        {0, 0, 0, 0}
    };

    // Parse command-line arguments
    while ((opt = getopt_long(argc, argv, "t:p:h:u:c:", long_options, NULL)) != -1) {
        switch (opt) {
        case 't':
            timeout_secs = atoi(optarg);
            break;
        case 'p':
            port = atoi(optarg);
            break;
        case 'h':
            host = optarg;
            break;
        case 'u':
            uri_path = optarg;
            break;
        case 'c':
            cert_path = optarg;
            break;
        default:
            fprintf(
                stderr,
                "Usage: %s --timeout-secs <seconds> --port <port> --host <host> --uri-path <path> "
                "--cert-path <path_to_certs>\n",
                argv[0]
            );
            exit(1);
        }
    }

    // Validate required arguments
    if (timeout_secs <= 0 || port <= 0 || !host || !uri_path || !cert_path) {
        fprintf(
            stderr, "Error: All arguments (--timeout-secs, --port, --host, --uri-path, --cert-path) are "
                    "required and must be valid.\n"
        );
        fprintf(
            stderr,
            "Usage: %s --timeout-secs <seconds> --port <port> --host <host> --uri-path <path> --cert-path "
            "<path_to_certs>\n",
            argv[0]
        );
        exit(1);
    }

    // Build the URL string (assuming HTTPS for secure request with certificates)
    char url[512]; // Buffer for the full URL
    snprintf(url, sizeof(url), "https://%s:%d%s", host, port, uri_path);

    // Initialize libcurl
    curl_global_init(CURL_GLOBAL_DEFAULT);

    // Get a curl easy handle
    curl = curl_easy_init();
    if (curl) {
        // Set URL
        curl_easy_setopt(curl, CURLOPT_URL, url);
        // Set timeout for the entire operation, including connection
        curl_easy_setopt(curl, CURLOPT_TIMEOUT, (long)timeout_secs);
        // Set to HTTP GET method
        curl_easy_setopt(curl, CURLOPT_HTTPGET, 1L);
        // Set a dummy write function to discard response body if any is received
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
        // Enable HTTP/2 over TLS (HTTPS)
        curl_easy_setopt(curl, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_2TLS);
        // Set CA certificate path for SSL verification
        curl_easy_setopt(curl, CURLOPT_CAINFO, cert_path);
        // Ensure SSL verification is enabled (default for HTTPS but good to be explicit)
        curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 1L);
        curl_easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 2L); // Verify common name and subject alt names
        // Perform the request
        res = curl_easy_perform(curl);

        if (res != CURLE_OK) {
            // An error occurred during the request (e.g., connection refused, timeout, DNS resolution)
            fprintf(
                stdout, "Error: Server at %s:%d is not reachable or responsive (%s). Not ready.\n", host,
                port, curl_easy_strerror(res)
            );
            http_code = 1; // Indicate failure
        } else {
            // Get HTTP response code if the request was performed successfully
            curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);

            if (http_code >= 200 && http_code < 500) {
                // 3xx, 4xx are acceptable since the health check in this app only cares about
                // connectivity at TCP layer not application layer.
                fprintf(stdout, "Server at %s:%d responded with status %ld. Ready.\n", host, port, http_code);
                http_code = 0; // Indicate success
            } else {
                fprintf(
                    stdout, "Server at %s:%d responded with status %ld. Not ready.\n", host, port, http_code
                );
                http_code = 1; // Indicate failure
            }
        }

        // Clean up curl handle
        curl_easy_cleanup(curl);
    } else {
        fprintf(stdout, "Error: Failed to initialize cURL.\n");
        http_code = 1; // Indicate failure
    }

    // Clean up libcurl
    curl_global_cleanup();

    return (int)http_code;
}
