CMAKE_MINIMUM_REQUIRED(VERSION 3.12.0)
PROJECT(media_app C)

SET(VERSION_MAJOR "0")
SET(VERSION_MINOR "1")
SET(VERSION_PATCH "0")

INCLUDE(GNUInstallDirs)
INCLUDE(CheckCSourceCompiles)
INCLUDE(CMakePushCheckState)
INCLUDE(CTest)

SET(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} ${CMAKE_CURRENT_SOURCE_DIR}/cmake)

FIND_PACKAGE(PkgConfig REQUIRED)
FIND_PACKAGE(Threads REQUIRED)
FIND_PACKAGE(OpenSSL REQUIRED)
FIND_PACKAGE(Cmagic  REQUIRED)
# TODO, optionally running test
FIND_PACKAGE(Cgreen REQUIRED)

IF (OPENSSL_VERSION VERSION_LESS 1.0.2)
    # also to prevent heartbleed bug (see CVE-2014-0160 for detail)
    MESSAGE(SEND_ERROR 
        "   ****************************************************************\n"
        "   * OpenSSL 1.0.2 is required for HTTP/2 interoperability with   *\n"
        "   * web browsers.                                                *\n"
        "   ****************************************************************\n")
ENDIF (OPENSSL_VERSION VERSION_LESS 1.0.2)
IF(OPENSSL_VERSION VERSION_EQUAL "1.1.0" AND OPENSSL_VERSION STRLESS "1.1.0g")
    MESSAGE(WARNING 
        "   ********************************************************************\n"
        "   * OpenSSL 1.1.0 ~ 1.1.0f would cause session resumption failed     *\n"
        "   * when using external cache.                                       *\n"
        "   ********************************************************************\n")
ENDIF(OPENSSL_VERSION VERSION_EQUAL "1.1.0" AND OPENSSL_VERSION STRLESS "1.1.0g")

PKG_CHECK_MODULES(LIBELF  REQUIRED libelf>=0.8.13)
PKG_CHECK_MODULES(LIBUV   REQUIRED libuv>=1.42.0)
PKG_CHECK_MODULES(H2O     REQUIRED libh2o)
PKG_CHECK_MODULES(JANSSON REQUIRED jansson>=2.14)
PKG_CHECK_MODULES(BROTLI_DEC REQUIRED libbrotlidec)
PKG_CHECK_MODULES(BROTLI_ENC REQUIRED libbrotlienc)

PKG_CHECK_MODULES(RHONABWY  REQUIRED  librhonabwy>=1.1.2)
PKG_CHECK_MODULES(GNUTLS    REQUIRED  gnutls>=3.7.2)
PKG_CHECK_MODULES(NETTLE    REQUIRED  nettle>=3.7.2)
PKG_CHECK_MODULES(P11KIT    REQUIRED  p11-kit-1>=0.24.0)

PKG_CHECK_MODULES(MARIADB  REQUIRED  libmariadb>=3.4.1)
PKG_CHECK_MODULES(RABBITMQ REQUIRED  librabbitmq>=0.11.0)

PKG_CHECK_MODULES(AVCODEC   REQUIRED  libavcodec>=58.91.100)
PKG_CHECK_MODULES(AVFORMAT  REQUIRED  libavformat>=58.45.100)
PKG_CHECK_MODULES(AVUTIL    REQUIRED  libavutil>=56.51.100)
PKG_CHECK_MODULES(AVFILTER  REQUIRED  libavfilter>=7.85.100)
PKG_CHECK_MODULES(AVDEVICE  REQUIRED  libavdevice>=58.10.100)

PKG_CHECK_MODULES(LIBUUID  REQUIRED uuid>=2.20.0)
PKG_CHECK_MODULES(LIBCURL  REQUIRED libcurl>=7.69.0)
PKG_CHECK_MODULES(LIBNGHTTP2  REQUIRED libnghttp2>=1.46.0)

MESSAGE(STATUS "OPENSSL_INCLUDE_DIR : ${OPENSSL_INCLUDE_DIR}")
MESSAGE(STATUS "OPENSSL_LIBRARIES : ${OPENSSL_LIBRARIES}")
MESSAGE(STATUS "H2O_LIBRARY_DIRS : ${H2O_LIBRARY_DIRS}")
MESSAGE(STATUS "H2O_LIBRARIES : ${H2O_LIBRARIES}")
MESSAGE(STATUS "H2O_ROOT_DIR : ${H2O_ROOT_DIR}")
MESSAGE(STATUS "CMAGIC_INCLUDE_DIR : ${CMAGIC_INCLUDE_DIR}")
MESSAGE(STATUS "LIBCURL_INCLUDE_DIRS : ${LIBCURL_INCLUDE_DIRS}")
MESSAGE(STATUS "LIBCURL_LIBRARY_DIRS : ${LIBCURL_LIBRARY_DIRS}")
MESSAGE(STATUS "LIBNGHTTP2_INCLUDE_DIRS : ${LIBNGHTTP2_INCLUDE_DIRS}")
MESSAGE(STATUS "AVFORMAT_LIBRARY_DIRS: ${AVFORMAT_LIBRARY_DIRS}")
MESSAGE(STATUS "AVUTIL_LIBRARY_DIRS: ${AVUTIL_LIBRARY_DIRS}")

# find out whether pthread provides function to set CPU affinity
CMAKE_PUSH_CHECK_STATE()
SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -pthread")
CHECK_C_SOURCE_COMPILES("
#define _GNU_SOURCE
#include <sched.h>
#include <pthread.h>
int main(void) {
    pthread_t tid = 0;
    cpu_set_t s;
    CPU_ZERO(&s);
    pthread_setaffinity_np(tid, sizeof(cpu_set_t), &s);
    return 0;
}
" HAS_PTHREAD_SETAFFINITY_NP)
CMAKE_POP_CHECK_STATE()

# find out whether libc provides backtrace()
CMAKE_PUSH_CHECK_STATE()
FIND_LIBRARY(LIBC_BACKTRACE_LIB "execinfo")
CHECK_C_SOURCE_COMPILES("
#include <execinfo.h>
int main(void) {
    int stdout_fd = 2;
    void *p[10];
    int num_entries = backtrace(p, 10);
    backtrace_symbols_fd(p, num_entries, stdout_fd);
    return 0;
}" LIBC_HAS_BACKTRACE)
CMAKE_POP_CHECK_STATE()

IF (LIBC_HAS_BACKTRACE)
    ADD_DEFINITIONS("-DLIBC_HAS_BACKTRACE")
    IF (LIBC_BACKTRACE_LIB)
        LIST(APPEND EXTRA_LIBS ${LIBC_BACKTRACE_LIB})
    ENDIF()
ENDIF ()

IF(DEFINED PYVENV_PATH)
    SET(PYVENV_BIN_PATH   ${PYVENV_PATH}/bin)
    SET(PYVENV_EXEC_PATH  ${PYVENV_BIN_PATH}/python)
    IF (EXISTS ${PYVENV_PATH} AND EXISTS ${PYVENV_BIN_PATH}
             AND EXISTS ${PYVENV_EXEC_PATH})
        SET(PYVENV_AVAIL  ON)
    ELSE()
        SET(PYVENV_AVAIL  OFF)
    ENDIF()
ELSE()
    SET(PYVENV_AVAIL  OFF)
ENDIF()

IF(NOT ${PYVENV_AVAIL})
    MESSAGE(SEND_ERROR "Path to Python virtual environment is missing \n")
ENDIF()

IF(DEFINED NGINX_INSTALL_PATH)
    SET(NGINX_EXECUTABLE_PATH  ${NGINX_INSTALL_PATH}/sbin/nginx)
    SET(NGINX_CONFIG_BASEPATH  ${NGINX_INSTALL_PATH}/conf)
    IF(EXISTS ${NGINX_EXECUTABLE_PATH} AND EXISTS ${NGINX_CONFIG_BASEPATH}
            AND EXISTS ${NGINX_INSTALL_PATH}/logs)
        SET(NGINX_PATH_EXISTS  ON)
    ELSE()
        SET(NGINX_PATH_EXISTS  OFF)
    ENDIF()
ELSE()
    SET(NGINX_PATH_EXISTS  OFF)
ENDIF()

IF (${NGINX_PATH_EXISTS} AND DEFINED CDN_USERNAME AND DEFINED CDN_USERGRP
        AND ${PYVENV_AVAIL})
    SET(CDN_ENABLE  ON)
ELSE()
    SET(CDN_ENABLE  OFF)
ENDIF()

MESSAGE(STATUS "PYVENV_PATH: ${PYVENV_PATH}")
MESSAGE(STATUS "Nginx installed path exists: ${NGINX_PATH_EXISTS}")
MESSAGE(STATUS "CDN_USERNAME: ${CDN_USERNAME}")
MESSAGE(STATUS "CDN_USERGRP:  ${CDN_USERGRP}")
MESSAGE(STATUS "CDN_ENABLE: ${CDN_ENABLE}")


INCLUDE_DIRECTORIES(
    include 
    ${LIBELF_INCLUDE_DIRS}
    ${LIBUV_INCLUDE_DIRS}
    ${H2O_INCLUDE_DIRS}
    ${RHONABWY_INCLUDE_DIRS}
    ${GNUTLS_INCLUDE_DIRS}
    ${NETTLE_INCLUDE_DIRS}
    ${P11KIT_INCLUDE_DIRS}
    ${MARIADB_INCLUDE_DIRS}
    ${RABBITMQ_INCLUDE_DIRS}
    ${CMAGIC_INCLUDE_DIR}
    ${AVFORMAT_INCLUDE_DIRS}
    ${AVCODEC_INCLUDE_DIRS}
    ${AVUTIL_INCLUDE_DIRS}
    ${AVFILTER_INCLUDE_DIRS}
    ${AVDEVICE_INCLUDE_DIRS}
    ${LIBUUID_INCLUDE_DIRS}
    ${LIBCURL_INCLUDE_DIRS}
    ${LIBNGHTTP2_INCLUDE_DIRS}
    ${JANSSON_INCLUDE_DIRS})

LINK_DIRECTORIES(
    ${LIBELF_LIBRARY_DIRS}
    ${LIBUV_LIBRARY_DIRS}
    ${H2O_LIBRARY_DIRS}
    ${BROTLI_DEC_LIBRARY_DIRS}
    ${BROTLI_ENC_LIBRARY_DIRS}
    ${RHONABWY_LIBRARY_DIRS}
    ${GNUTLS_LIBRARY_DIRS}
    ${NETTLE_LIBRARY_DIRS}
    ${P11KIT_LIBRARY_DIRS}
    ${MARIADB_LIBRARY_DIRS}
    ${RABBITMQ_LIBRARY_DIRS}
    ${AVFORMAT_LIBRARY_DIRS}
    ${AVCODEC_LIBRARY_DIRS}
    ${AVUTIL_LIBRARY_DIRS}
    ${AVFILTER_LIBRARY_DIRS}
    ${AVDEVICE_LIBRARY_DIRS}
    ${JANSSON_LIBRARY_DIRS}
    ${LIBUUID_LIBRARY_DIRS}
    ${LIBCURL_LIBRARY_DIRS}
    ${LIBNGHTTP2_LIBRARY_DIRS} )




SET(CC_WARNING_FLAGS "-Wall -Wno-unused-value -Wno-unused-function -Wno-nullability-completeness -Wno-expansion-to-defined -Werror=implicit-function-declaration -Werror=incompatible-pointer-types")

IF ("${CMAKE_C_COMPILER_ID}" STREQUAL "GNU")
    IF (NOT ("${CMAKE_C_COMPILER_VERSION}" VERSION_LESS "4.6"))
        SET(CC_WARNING_FLAGS "${CC_WARNING_FLAGS} -Wno-unused-but-set-variable")
    ENDIF ()
    IF (NOT ("${CMAKE_C_COMPILER_VERSION}" VERSION_LESS "4.5"))
        SET(CC_WARNING_FLAGS "${CC_WARNING_FLAGS} -Wno-unused-result")
    ENDIF ()
ENDIF ()

SET(CMAKE_C_FLAGS "-std=c17 -g3 ${CC_WARNING_FLAGS} ${CMAKE_C_FLAGS}")

IF (CMAKE_SYSTEM_NAME STREQUAL "Linux")
    SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -pthread -D_GNU_SOURCE")
ELSE () # for FreeBSD etc.
    SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -pthread")
ENDIF ()

IF (HAS_PTHREAD_SETAFFINITY_NP)
    SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -DHAS_PTHREAD_SETAFFINITY_NP")
ENDIF ()

## SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -DDEBUG_MULTIPART")

# cmake defaults to a Debug build, whereas the application defaults to an optimised build
IF (NOT CMAKE_BUILD_TYPE)
    SET(CMAKE_BUILD_TYPE Release)
ENDIF(NOT CMAKE_BUILD_TYPE)

if(DEFINED PROCEED_TRANSCODING_TEST)
    add_compile_definitions(PROCEED_TRANSCODING_TEST=${PROCEED_TRANSCODING_TEST})
endif()

SET(CMAKE_C_FLAGS_DEBUG  "-O0")
SET(CMAKE_C_FLAGS_RELEASE  "-O2")

SET(APP_HTTPSERVER_SOURCE_FILES
    src/acl.c
    src/cfg_parser.c
    src/app_server/auth.c
    src/app_server/middleware.c
    src/app_server/multipart_parser.c
    src/app_server/network.c
    src/rpc/reply.c
    src/api/init.c
    src/api/common.c
    src/api/filefetch_common.c
    src/api/initiate_file_nonstream.c
    src/api/initiate_file_stream.c
    src/api/stream_file_lookup.c
    src/api/edit_filelvl_access_ctrl.c
    src/api/edit_usrlvl_access_ctrl.c
    src/api/read_usrlvl_access_ctrl.c
    src/api/monitor_job_progress.c
    src/api/initiate_multipart_upload.c
    src/api/complete_multipart_upload.c
    src/api/start_transcoding_file.c
    src/api/discard_committed_file.c
    src/api/upload_part.c )

SET(APP_TRANSCODE_COMMON_SOURCE_FILES
    src/transcoder/cfg_parser.c
    src/transcoder/file_processor.c
    src/transcoder/segment.c
    src/transcoder/storage.c
    src/transcoder/image/metadata.c
    src/transcoder/image/storage.c
    src/transcoder/video/metadata.c 
    src/transcoder/video/storage.c
    src/transcoder/video/hls/common.c )

SET(APP_TRANSCODE_TABLE_IN_RPC_CONSUMER__SOURCE_FILES
    src/transcoder/table_rpc_consumer.c
    src/transcoder/image/ffmpeg/in/table_rpc_consumer.c
    src/transcoder/image/ffmpeg/out/table_rpc_consumer.c
    src/transcoder/video/hls/table_rpc_consumer.c )

SET(APP_TRANSCODER_SOURCE_FILES
    ${APP_TRANSCODE_COMMON_SOURCE_FILES}
    src/transcoder/image/ffmpeg/in/init.c
    src/transcoder/image/ffmpeg/in/avcontext.c
    src/transcoder/image/ffmpeg/out/init.c
    src/transcoder/image/ffmpeg/out/avcontext.c
    src/transcoder/image/ffmpeg/out/avfilter.c
    src/transcoder/video/hls/init.c 
    src/transcoder/video/hls/deinit.c 
    src/transcoder/video/hls/avcontext.c 
    src/transcoder/video/hls/avfilter.c 
    src/transcoder/video/hls/output.c 
    src/transcoder/video/mp4/preload.c 
    src/transcoder/video/mp4/avcontext.c 
    src/transcoder/video/mp4/init.c )

SET(APP_TRANSCODE_TABLE_IN_HTTP_SERVER__SOURCE_FILES
    src/transcoder/table_api_server.c
    src/transcoder/video/hls/table_api_server.c )

SET(APP_STREAMING_SOURCE_FILES
    ${APP_TRANSCODE_COMMON_SOURCE_FILES}
    src/transcoder/validation.c
    src/transcoder/crypto.c
    src/transcoder/cache.c
    src/transcoder/video/hls/seeker/common.c
    src/transcoder/video/hls/seeker/master_playlist.c
    src/transcoder/video/hls/seeker/secondary_playlist.c
    src/transcoder/video/hls/seeker/cryptokey.c
    src/transcoder/video/hls/seeker/segment.c
    src/transcoder/video/hls/init_stream.c )

SET(APP_TRANSCODED_REMOVAL_SOURCE_FILES
    src/transcoder/removal.c )

SET(APP_RPCCONSUMER_HANDLER_SOURCE_FILES
    src/api/async/start_transcode.c
    src/api/async/finalize_transcode.c )

SET(APP_DBMODEL_SOURCE_FILES
    src/models/cfg_parser.c
    src/models/pool.c
    src/models/connection.c
    src/models/mariadb.c
    src/models/query.c )

SET(APP_STORAGE_SOURCE_FILES
    src/storage/cfg_parser.c
    src/storage/localfs.c )

SET(APP_COMMON_SOURCE_FILES
    src/rpc/core.c
    src/rpc/cfg_parser.c
    src/app_cfg.c
    src/utils.c
    src/base64.c
    src/timer_poll.c
    src/routes.c )

SET(APP_SERVER_SOURCE_FILES
    src/app_server/worker.c
    src/third_party/rhonabwy.c
    ${APP_DBMODEL_SOURCE_FILES}
    ${APP_STORAGE_SOURCE_FILES}
    ${APP_STREAMING_SOURCE_FILES}
    ${APP_TRANSCODED_REMOVAL_SOURCE_FILES}
    ${APP_TRANSCODE_TABLE_IN_HTTP_SERVER__SOURCE_FILES}
    ${APP_HTTPSERVER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})

SET(RPC_CONSUMER_SOURCE_FILES
    src/rpc/consumer.c
    ${APP_DBMODEL_SOURCE_FILES}
    ${APP_STORAGE_SOURCE_FILES}
    ${APP_TRANSCODER_SOURCE_FILES}
    ${APP_TRANSCODE_TABLE_IN_RPC_CONSUMER__SOURCE_FILES}
    ${APP_RPCCONSUMER_HANDLER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})


SET(UNIT_TEST_SOURCE_FILES
    test/unit/main.c
    test/unit/app_cfg.c
    test/unit/cfg_parser.c
    test/unit/routes.c
    test/unit/acl.c
    test/unit/utils.c
    test/unit/timer_poll.c
    test/unit/app_server/auth.c
    test/unit/app_server/middleware.c
    test/unit/app_server/multipart_parser.c
    test/unit/app_server/network.c
    test/unit/models/cfg_parser.c
    test/unit/models/pool.c
    test/unit/models/connection.c
    test/unit/models/query.c
    test/unit/models/mariadb.c
    test/unit/transcoder/cfg_parser.c
    test/unit/transcoder/file_processor.c
    test/unit/transcoder/cache.c
    test/unit/transcoder/crypto.c
    test/unit/transcoder/removal.c
    test/unit/transcoder/storage.c
    test/unit/transcoder/validation.c
    test/unit/transcoder/video/storage.c
    test/unit/transcoder/video/mp4/init.c
    test/unit/transcoder/video/mp4/preload.c
    test/unit/transcoder/video/mp4/avcontext.c
    test/unit/transcoder/video/hls/avcontext.c
    test/unit/transcoder/video/hls/avfilter.c
    test/unit/transcoder/video/hls/output.c
    test/unit/transcoder/video/hls/init.c
    test/unit/transcoder/video/hls/init_stream.c
    test/unit/transcoder/video/hls/seeker/master_playlist.c
    test/unit/transcoder/video/hls/seeker/secondary_playlist.c
    test/unit/transcoder/video/hls/seeker/cryptokey.c
    test/unit/transcoder/video/hls/seeker/segment.c
    test/unit/transcoder/image/storage.c
    test/unit/transcoder/image/ffmpeg/in/init.c
    test/unit/transcoder/image/ffmpeg/in/avcontext.c
    test/unit/transcoder/image/ffmpeg/out/init.c
    test/unit/transcoder/image/ffmpeg/out/avcontext.c
    test/unit/transcoder/image/ffmpeg/out/avfilter.c
    test/unit/rpc/cfg_parser.c
    test/unit/rpc/core.c
    test/unit/rpc/reply.c
    test/unit/views/common.c
    test/unit/storage/cfg_parser.c
    test/unit/storage/localfs.c
    test/unit/mock/rhonabwy.c
    test/unit/mock/mariadb.c
    test/unit/mock/rabbitmq.c
    test/unit/mock/openssl.c
    test/unit/mock/curl.c
    test/unit/mock/uuid.c
    test/unit/mock/ffmpeg/libavformat.c
    test/unit/mock/ffmpeg/libavcodec.c
    test/unit/mock/ffmpeg/libavfilter.c
    test/unit/mock/ffmpeg/libavutil.c
    ${APP_DBMODEL_SOURCE_FILES}
    ${APP_STORAGE_SOURCE_FILES}
    ${APP_TRANSCODER_SOURCE_FILES}
    ${APP_TRANSCODE_TABLE_IN_RPC_CONSUMER__SOURCE_FILES}
    ${APP_STREAMING_SOURCE_FILES}
    ${APP_TRANSCODED_REMOVAL_SOURCE_FILES}
    ${APP_HTTPSERVER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})

SET(ITEST_APPSRV_SOURCE_FILES
    test/integration/main.c
    test/integration/auth.c
    test/integration/client.c
    test/integration/rpc_consumer.c
    test/integration/api/access_control_list.c
    test/integration/api/monitor_job_progress.c
    test/integration/api/start_transcoding_file.c
    test/integration/api/complete_multipart_upload.c
    test/integration/api/initiate_multipart_upload.c
    test/integration/api/upload_part.c
    test/integration/api/filefetch_common.c
    test/integration/api/initiate_file_nonstream.c
    test/integration/api/initiate_file_stream.c
    test/integration/api/stream_file_lookup.c
    test/integration/api/discard_committed_file.c
    ${APP_SERVER_SOURCE_FILES} )

SET(ITEST_RPCCONSUMER_SOURCE_FILES
    test/integration/rpc_consumer.c
    ${RPC_CONSUMER_SOURCE_FILES} )

SET(APP_COMMON_LIBRARIES
    ${LIBELF_LIBRARIES}
    ${LIBUV_LIBRARIES}
    ${BROTLI_DEC_LIBRARIES}
    ${BROTLI_ENC_LIBRARIES}
    ${H2O_LIBRARIES}
    ${JANSSON_LIBRARIES} )

SET(APP_SERVER_DEV_LIBRARIES
    ${OPENSSL_LIBRARIES}
    ${RHONABWY_LIBRARIES}
    ${GNUTLS_LIBRARIES}
    ${NETTLE_LIBRARIES}
    ${P11KIT_LIBRARIES}
    ${MARIADB_LIBRARIES}
    ${RABBITMQ_LIBRARIES}
    ${AVFORMAT_LIBRARIES}
    ${AVCODEC_LIBRARIES}
    ${LIBCURL_LIBRARIES}
    ${LIBUUID_LIBRARIES}
    ${LIBNGHTTP2_LIBRARIES}
    ${APP_COMMON_LIBRARIES})

SET(RPC_CONSUMER_DEV_LIBRARIES
    ${OPENSSL_LIBRARIES}
    ${MARIADB_LIBRARIES}
    ${RABBITMQ_LIBRARIES}
    ${CMAGIC_LIBRARIES}
    ${AVFORMAT_LIBRARIES}
    ${AVCODEC_LIBRARIES}
    ${AVUTIL_LIBRARIES}
    ${AVFILTER_LIBRARIES}
    ${AVDEVICE_LIBRARIES}
    ${LIBUUID_LIBRARIES}
    ${APP_COMMON_LIBRARIES})

SET(APP_UNIT_TEST_LIBRARIES ${APP_COMMON_LIBRARIES})


SET(APP_SERVER_EXE_NAME   app_server.out)
SET(RPC_CONSUMER_EXE_NAME rpc_consumer.out)
ADD_EXECUTABLE(${APP_SERVER_EXE_NAME}   ${APP_SERVER_SOURCE_FILES}    src/app_server/main.c)
ADD_EXECUTABLE(${RPC_CONSUMER_EXE_NAME} ${RPC_CONSUMER_SOURCE_FILES}  src/rpc/consumer_main.c)
TARGET_INCLUDE_DIRECTORIES(${APP_SERVER_EXE_NAME}    PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_INCLUDE_DIRECTORIES(${RPC_CONSUMER_EXE_NAME}  PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${APP_SERVER_EXE_NAME}    ${APP_SERVER_DEV_LIBRARIES}   ${EXTRA_LIBS})
TARGET_LINK_LIBRARIES(${RPC_CONSUMER_EXE_NAME}  ${RPC_CONSUMER_DEV_LIBRARIES} ${EXTRA_LIBS})

SET(UNIT_TEST_EXE_NAME unit_test.out)
ADD_EXECUTABLE(${UNIT_TEST_EXE_NAME} ${UNIT_TEST_SOURCE_FILES})
TARGET_INCLUDE_DIRECTORIES(${UNIT_TEST_EXE_NAME} PUBLIC ${CGREEN_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${UNIT_TEST_EXE_NAME} ${APP_UNIT_TEST_LIBRARIES} ${CGREEN_LIBRARIES})

SET(ITEST_API_SERVER_EXE_NAME    itest_app_server.out)
SET(ITEST_RPC_CONSUMER_EXE_NAME  itest_rpc_consumer.out)
ADD_EXECUTABLE(${ITEST_API_SERVER_EXE_NAME}   ${ITEST_APPSRV_SOURCE_FILES})
ADD_EXECUTABLE(${ITEST_RPC_CONSUMER_EXE_NAME} ${ITEST_RPCCONSUMER_SOURCE_FILES}  src/rpc/consumer_main.c)
TARGET_INCLUDE_DIRECTORIES(${ITEST_API_SERVER_EXE_NAME}    PUBLIC  ${CGREEN_INCLUDE_DIR})
TARGET_INCLUDE_DIRECTORIES(${ITEST_RPC_CONSUMER_EXE_NAME}  PUBLIC  ${CGREEN_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${ITEST_API_SERVER_EXE_NAME}    ${APP_SERVER_DEV_LIBRARIES}   ${CGREEN_LIBRARIES})
TARGET_LINK_LIBRARIES(${ITEST_RPC_CONSUMER_EXE_NAME}  ${RPC_CONSUMER_DEV_LIBRARIES} ${CGREEN_LIBRARIES})

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    # Add the linker option -rdynamic for the target
    TARGET_LINK_OPTIONS(${ITEST_API_SERVER_EXE_NAME} PRIVATE -rdynamic)
    MESSAGE(STATUS "symbols added to app-server executable")
endif()

# TODO, better way to scale app servers, without manually adding more custom targets
SET(DEV_CDN_CFG_PATH   ${CMAKE_CURRENT_SOURCE_DIR}/settings/dev-cdn.json)
SET(APP_REP1_CFG_PATH  ${CMAKE_CURRENT_SOURCE_DIR}/settings/dev-replica-1.json)
SET(DEV_APPCFG_PATH    ${CMAKE_CURRENT_SOURCE_DIR}/settings/development.json)
SET(ITEST_APPCFG_PATH  ${CMAKE_CURRENT_SOURCE_DIR}/settings/test.json)

# bash script for creating another shell process to run in specific python virtual environment
SET(PYVENV_SHELL_FILEPATH  $ENV{PWD}/tmp_shell)
#FILE(WRITE  ${CMAKE_CURRENT_SOURCE_DIR}/../tmp_shell "source ${PYVENV_BIN_PATH}/activate;")
#FILE(APPEND ${CMAKE_CURRENT_SOURCE_DIR}/../tmp_shell "python --version;")
    
ADD_CUSTOM_TARGET(dev_app_server
    COMMAND ${CMAKE_COMMAND} -E echo "source ${PYVENV_BIN_PATH}/activate;" >  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python -m  media.task_loader DevAppSrvCertRenewal  ${DEV_APPCFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
    COMMAND bash ${PYVENV_SHELL_FILEPATH}
    COMMAND 
        ##gdb -args
        ##valgrind --leak-check=full   --show-leak-kinds=definite 
        ${CMAKE_CURRENT_SOURCE_DIR}/build/${APP_SERVER_EXE_NAME}  ${DEV_APPCFG_PATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${APP_SERVER_EXE_NAME}
    VERBATIM # properly esacpe every token of a command string
)

ADD_CUSTOM_TARGET(app_server_replica_1
    COMMAND ${CMAKE_COMMAND} -E echo "source ${PYVENV_BIN_PATH}/activate;" >  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python -m  media.task_loader DevAppSrvCertRenewal  ${APP_REP1_CFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
    COMMAND bash ${PYVENV_SHELL_FILEPATH}
    COMMAND
        ##gdb -args
        ${CMAKE_CURRENT_SOURCE_DIR}/build/${APP_SERVER_EXE_NAME}  ${APP_REP1_CFG_PATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${APP_SERVER_EXE_NAME}  VERBATIM)

IF(${CDN_ENABLE})
    ADD_CUSTOM_TARGET(dev_cdn_setup
        COMMAND ${CMAKE_COMMAND} -E echo "source ${PYVENV_BIN_PATH}/activate;" >  ${PYVENV_SHELL_FILEPATH}
        COMMAND ${CMAKE_COMMAND} -E echo "python -m  media.task_loader DevAppCdnCertRenewal  ${DEV_CDN_CFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
        COMMAND ${CMAKE_COMMAND} -E echo "python -m  media.task_loader NginxConfigGenerator  ${DEV_CDN_CFG_PATH}  ${CDN_USERNAME}  ${CDN_USERGRP}  2  ${DEV_APPCFG_PATH}  ${APP_REP1_CFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
        COMMAND ${CMAKE_COMMAND} -E echo "sudo -u ${CDN_USERNAME}  ${PYVENV_EXEC_PATH} -m  media.task_loader NginxPathSetup   ${DEV_CDN_CFG_PATH}  ${NGINX_INSTALL_PATH}" >>  ${PYVENV_SHELL_FILEPATH}
        COMMAND bash ${PYVENV_SHELL_FILEPATH}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
        DEPENDS ${NGINX_EXECUTABLE_PATH} ${NGINX_CONFIG_BASEPATH}
        VERBATIM)
ENDIF()

ADD_CUSTOM_TARGET(dev_rpc_worker
    ##gdb -args
    ##valgrind --leak-check=full
    ${CMAKE_CURRENT_SOURCE_DIR}/build/${RPC_CONSUMER_EXE_NAME}  ${DEV_APPCFG_PATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${RPC_CONSUMER_EXE_NAME})


ADD_CUSTOM_TARGET(unit_test
    # uncomment the following line, make cgreen test suite run in single (main) process
    # without forking, then you can set breakpoints using debugger (currently only supports GDB)
    ${CMAKE_COMMAND} -E env "CGREEN_NO_FORK=xxx"
    ##gdb -args # uncomment this line for debugging
    valgrind --leak-check=full # uncomment this line for extra memory check
    ${CMAKE_CURRENT_SOURCE_DIR}/build/${UNIT_TEST_EXE_NAME}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${UNIT_TEST_EXE_NAME})


ADD_CUSTOM_TARGET(itest_app_server
    ##${CMAKE_COMMAND} -E env "LD_LIBRARY_PATH=/path/to/somewhere/else/lib:$ENV{$LD_LIBRARY_PATH}"
    COMMAND ${CMAKE_COMMAND} -E echo "source ${PYVENV_BIN_PATH}/activate;" >  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python3 -m media.task_loader  TestAppSrvCertRenewal ${ITEST_APPCFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python3 -m media.task_loader  FilechunkSetup  ${ITEST_APPCFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python3 -m media.test.integration.auth  --host=localhost --port=8008  --path2privkey=${CMAKE_CURRENT_SOURCE_DIR}/../tmp/cache/test/jwks/media-rsa-privkey.json  --sslcertpath=${CMAKE_CURRENT_SOURCE_DIR}/data/certs/test &" >>  ${PYVENV_SHELL_FILEPATH}

    COMMAND bash ${PYVENV_SHELL_FILEPATH}
    COMMAND sleep 5
    COMMAND ${CMAKE_COMMAND} -E env "CGREEN_NO_FORK=xxx"
        "LD_LIBRARY_PATH=${AVFORMAT_LIBRARY_DIRS}:$ENV{$LD_LIBRARY_PATH}"
        ##gdb -args # uncomment this line for debugging
        valgrind --leak-check=full
        ${CMAKE_CURRENT_SOURCE_DIR}/build/${ITEST_API_SERVER_EXE_NAME}  ${ITEST_APPCFG_PATH}
    COMMAND ${CMAKE_COMMAND} -E echo "source ${PYVENV_BIN_PATH}/activate;" >  ${PYVENV_SHELL_FILEPATH}
    COMMAND ${CMAKE_COMMAND} -E echo "python -m  media.task_loader  FilechunkTeardown   ${ITEST_APPCFG_PATH};" >>  ${PYVENV_SHELL_FILEPATH}
    COMMAND bash ${PYVENV_SHELL_FILEPATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS  ${ITEST_API_SERVER_EXE_NAME} ${ITEST_RPC_CONSUMER_EXE_NAME}
    VERBATIM)

ADD_CUSTOM_TARGET(itest_rpc_worker
    COMMAND ${CMAKE_COMMAND} -E env "CGREEN_NO_FORK=xxx"
        "LD_LIBRARY_PATH=${AVFORMAT_LIBRARY_DIRS}:$ENV{$LD_LIBRARY_PATH}"
        ##gdb -args
        ${CMAKE_CURRENT_SOURCE_DIR}/build/${ITEST_RPC_CONSUMER_EXE_NAME}  ${ITEST_APPCFG_PATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${ITEST_RPC_CONSUMER_EXE_NAME})

