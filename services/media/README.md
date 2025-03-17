# Media application
## Features
#### Multi-Part File Upload
- with limit of maximum allowed active upload requests
- media file spliting to smaller chunks, keep track of the chunks uploaded
- currently support only local file system on Linux server
#### Access Control List (ACL) Management
- File resource owner can modify file-level ACL, whether a transcoded file is visible to everyone
- Privileged users and the owner can modify user-level ACL, which users can transcode the file, view a transcoded file, or edit access permission on other users
#### Transcoding
- For video, currently support only mp4 (input) to HLS (output)
- For image, currently support JPG / GIF / PNG / TIFF
- configurable limit for custom quality resolutions
- asynchronous job for long-running transcoding tasks, with progress monitor
- concurrent transcoding of multiple files is supported
#### video streaming
- currently support only HLS
- optional caching file-streaming elements in reverse proxy (currnetly Nginx)

## High-Level Architecture

```mermaid
flowchart LR
    subgraph Clients
      PUBU(["👤 Public Users"])
      subgraph PrivilegedClients
        FRESO(["👤 File Resource Owner"])
        PRIVU(["👤 Privileged Users"])
      end
    end
    
    subgraph caching
        CACHE_NGINX[Nginx]
    end

    subgraph RPC-comsumer
        TRANS_ASYNC_JOB[Asynchronous Transcoding Job]
    end

    subgraph Web-Service-Layer
        MPFU[Multipart File Upload]
        ACLMGT[Access-Control List management]
        subgraph "Transcoding"
          TRANS_INIT[Initialization]
          TRANS_PROG_MON[Progress Monitor]
        end
        subgraph Transcoded-File-Fetch
          IMG_FETCH[Image]
          VID_STREAM[Video Streaming]
        end
    end

    subgraph Data-Store-Layer
      MARIA[MariaDB]
    end
    
    FRESO --> MPFU
    PrivilegedClients --> ACLMGT
    PrivilegedClients --> TRANS_INIT
    PrivilegedClients --> TRANS_PROG_MON
    TRANS_INIT --> TRANS_ASYNC_JOB
    Clients --> CACHE_NGINX --> Transcoded-File-Fetch
    Web-Service-Layer --> Data-Store-Layer
    RPC-comsumer --> Data-Store-Layer
```

## Prerequisite 
| type | name | version required |
|------|------|------------------|
| Database | MariaDB | `11.2.3` |
| Build tool | [Cmake](https://cmake.org/cmake/help/latest/index.html) | `>= 3.21.0` |
| | [gcc](https://gcc.gnu.org/onlinedocs/) with [c17](https://en.wikipedia.org/wiki/C17_(C_standard_revision)) stardard | `>= 10.3.0` |
| Dependency | [H2O](https://github.com/h2o/h2o) | after 2024 Dec |
| | [OpenSSL](https://github.com/openssl/openssl) | `>= 3.1.4` |
| | [brotli](https://github.com/google/brotli) | `>= 1.0.2` |
| | [jansson](https://github.com/akheron/jansson) | `>= 2.14` |
| | [libuuid](https://github.com/util-linux/util-linux/tree/master/libuuid) | `>= 2.20.0` |
| | [rhonabwy](https://github.com/babelouest/rhonabwy) | `>= 1.1.2` |
| | [gnutls](https://github.com/gnutls/gnutls) | `>= 3.7.2` |
| | [nettle](https://github.com/gnutls/nettle) | `>= 3.7.2` |
| | [p11-kit](https://github.com/p11-glue/p11-kit) | `>= 0.24.0` |
| | [MariaDB connector/C](https://github.com/mariadb-corporation/mariadb-connector-c) | `>= 3.4.1` |
| | [Rabbitmq/C](https://github.com/alanxz/rabbitmq-c) | `>= 0.11.0` |
| | [FFMpeg](https://github.com/FFmpeg/FFmpeg) | `>= 4.3.8` |
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

## Build
For full build / test instructions please refer to [this github action workflow script](../../.github/workflows/media-ci.yaml)

### Database setup
#### Grant access to Database
- Currently this application works only with MariaDB.
- Grant admin roles full access to the database
  - for `site_dba` role (in `common/data/secrets.json`), it is the database `ecommerce_media`
  - for `test_site_dba` role, it is the database `test_ecommerce_media`
  - see [`init_db.sql`](./migration/init_db.sql) for detail

#### Database Schema Migration
This application requires mariaDB database in the development or testing server, ensure to synchronize schema migration
```shell
> cd /PATH/TO/PROJECT_HOME/services/media
> /PATH/TO/liquibase --defaults-file=./liquibase.properties \
--changeLogFile=./migration/changelog_media.xml \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME \
      --username=$USER  --password=$PASSWORD \
      --log-level=info   update

> /PATH/TO/liquibase --defaults-file=./liquibase.properties \
      --changeLogFile=./migration/changelog_media.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME  \
      --username=$USER  --password=$PASSWORD \
      --log-level=info   rollback  $VERSION_TAG
```
Note : 
- the parameters above `$HOST`, `$PORT`, `$USER`, `$PASSWORD` should be consistent with database credential set in `${SYS_BASE_PATH}/common/data/secrets.json` , see the structure in [`common/data/secrets_template.json`](../common/data/secrets_template.json)
- the parameter `$DB_NAME` can be either `ecommerce_media` for development server, or  `test_ecommerce_media` for testing server, see [reference](./migration/init_db.sql)
- the subcommand `update` upgrades the schema to latest version
- the subcommand `rollback` rollbacks the schema to specific previous version `$VERSION_TAG` defined in the `migration/changelog_media.xml`


### Configuration
```bash
cd /PATH/TO/PROJECT_HOME/services/media
mkdir -p build
cd build

CC="/PATH/TO/gcc/10.3.0/installed/bin/gcc"   PKG_CONFIG_PATH="<YOUR_PATH_TO_PKG_CFG>" \
    cmake -DCMAKE_PREFIX_PATH="/PATH/TO/cgreen/installed"  -DPYVENV_PATH="/PATH/TO/python/venv" \
    -DNGINX_INSTALL_PATH="/PATH/TO/nginx/server/install" \
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

### Reference
- [API documentation (OpenAPI v3.0 specification)](./apidoc.yaml)
