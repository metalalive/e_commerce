## Features
- video transcoding
  - currently support only mp4 (input) and HLS (output)
- image transformng
  - currently support JPG/GIF/PNG/TIFF
- video streaming
  - currently support only HLS
  - optional reverse proxy / load balancing
- multi-part file upload
- user-defined file access control

## Build
### Prerequisite 
| type | name | version required |
|------|------|------------------|
| Database | MariaDB | `10.3.22` |
| Message Queue | RabbitMQ | `3.2.4` |
| Build tool | [Cmake](https://cmake.org/cmake/help/latest/index.html) | `>= 3.5.0` |
| | [gcc](https://gcc.gnu.org/onlinedocs/) with [c17](https://en.wikipedia.org/wiki/C17_(C_standard_revision)) stardard | `>= 10.3.0` |
| Dependency | [H2O](https://github.com/h2o/h2o) | `>= 2.3.0-DEV` |
| | [OpenSSL](https://github.com/openssl/openssl) | `>= 1.1.1c` |
| | [brotli](https://github.com/google/brotli) | `>= 1.0.2` |
| | [jansson](https://github.com/akheron/jansson) | `>= 2.14` |
| | [libuuid](https://github.com/util-linux/util-linux/tree/master/libuuid) | `>= 2.20.0` |
| | [rhonabwy](https://github.com/babelouest/rhonabwy) | `>= 1.1.2` |
| | [gnutls](https://github.com/gnutls/gnutls) | `>= 3.7.2` |
| | [nettle](https://github.com/gnutls/nettle) | `>= 3.7.2` |
| | [p11-kit](https://github.com/p11-glue/p11-kit) | `>= 0.24.0` |
| | [MariaDB connector/C](https://github.com/mariadb-corporation/mariadb-connector-c) | `>= 3.1.7` |
| | [Rabbitmq/C](https://github.com/rabbitmq/rabbitmq-c) | `>= 0.11.0` |
| | [FFMpeg](https://github.com/FFmpeg/FFmpeg) | `>= 4.3.3` |
| | [libcurl](https://github.com/curl/curl) | `>= 7.69.1` |
| Test | [nghttp2](https://github.com/nghttp2/nghttp2) | `>= 1.46.0` |
| | [cgreen](https://github.com/cgreen-devs/cgreen) | `>= 2.14` |
| Automation | python interpreter | `>= 3.9.6` |
| | python packages | [detail](./py_venv_requirement.txt) |
| DB migration | [liquibase](https://github.com/liquibase/liquibase) | `>= 4.6.2` |
| | | |
| | | |

Note: 
* `Nettle` is automatically built when building `gnutls` 
* `nghttp2` enables http/2 protocol in `libcurl` for testing

### Grant access to Database
- Currently this application works only with MariaDB.
- Grant admin roles full access to the database
  - for `site_dba` role (in `common/data/secrets.json`), it is the database `ecommerce_media`
  - for `test_site_dba` role, it is the database `test_ecommerce_media`
  - see [`init_db.sql`](../migrations/init_db.sql) for detail

#### Configuration
```bash
cd /PATH/TO/PROJECT_HOME/staff_portal

CC="/PATH/TO/gcc/10.3.0/installed/bin/gcc"   PKG_CONFIG_PATH="<YOUR_PATH_TO_PKG_CFG>" \
    cmake -DCMAKE_PREFIX_PATH="/PATH/TO/cgreen/installed"  -DLIQUIBASE_PATH="/PATH/TO/liquibase"  \
        -DPYVENV_PATH="/PATH/TO/python/venv"  -DNGINX_INSTALL_PATH="/PATH/TO/nginx/server/install" \
        -DCDN_USERNAME=<OS_USER_NAME>   -DCDN_USERGRP=<OS_USER_GROUP>   ..
```
Note
- `<YOUR_PATH_TO_PKG_CFG>` should include:
  - `/PATH/TO/brotli/pkgconfig`
  - `/PATH/TO/libuv/pkgconfig`
  - `/PATH/TO/h2o/pkgconfig`
  - `/PATH/TO/jansson/pkgconfig`
  - `/PATH/TO/rhonabwy/pkgconfig`
  - `/PATH/TO/gnutls/pkgconfig`
  - `/PATH/TO/nettle/pkgconfig`
  - `/PATH/TO/p11-kit/pkgconfig`
  - `/PATH/TO/mariadb/pkgconfig`
  - `/PATH/TO/rabbitmq-c/pkgconfig`
  - `/PATH/TO/ffmpeg/pkgconfig`
  - `/PATH/TO/libuuid/pkgconfig`
  - `/PATH/TO/libcurl/pkgconfig`
  - `/PATH/TO/nghttp2/pkgconfig`
  - `/PATH/TO/openssl/pkgconfig`
- omit parameters `NGINX_INSTALL_PATH`, `CDN_USERNAME`, `CDN_USERGRP` if you don't need reverse proxy


### Database Migration (for development server)
```bash
make  dev_db_init
```
Note this command performs only schema migration

### Compile and Run
| env | target | command |
|-----|--------|---------|
| development | app server (primary) | `make  dev_app_server` |
| development | app server (secondary) | `make  app_server_replica_1` |
| | generate nginx config file, for reverse proxy | `make  dev_cdn_setup` |
| development | RPC consumer  | `make  dev_rpc_worker` |
| unit test |  | `make  unit_test` |
| integration test | app server | `make  itest_app_server` |
| integration test | RPC consumer | `make  itest_rpc_worker` |

