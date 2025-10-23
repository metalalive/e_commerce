CMAKE_MINIMUM_REQUIRED(VERSION 3.12.0)
PROJECT(media_app C)

SET(VERSION_MAJOR "0")
SET(VERSION_MINOR "1")
SET(VERSION_PATCH "1")

INCLUDE(GNUInstallDirs)
INCLUDE(CheckCSourceCompiles)
INCLUDE(CMakePushCheckState)
INCLUDE(CTest)

option(ONLY_REFORMAT "Set to ON to skip finding required packages for regular build targets, useful for 'reformat' target only." OFF)
if(ONLY_REFORMAT)
    set(_PKG_REQUIRED "")
else()
    set(_PKG_REQUIRED "REQUIRED")
endif()

SET(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} ${CMAKE_CURRENT_SOURCE_DIR}/cmake)

FIND_PACKAGE(PkgConfig ${_PKG_REQUIRED})
FIND_PACKAGE(Threads ${_PKG_REQUIRED})
PKG_CHECK_MODULES(OPENSSL ${_PKG_REQUIRED} libssl>=3.0.0 libcrypto>=3.0.0)
PKG_CHECK_MODULES(CMAGIC  ${_PKG_REQUIRED} libmagic>=5.39)
# TODO, should be optional for running test
FIND_PACKAGE(Cgreen ${_PKG_REQUIRED})

PKG_CHECK_MODULES(LIBELF  ${_PKG_REQUIRED} libelf>=0.8.13)
PKG_CHECK_MODULES(LIBUV   ${_PKG_REQUIRED} libuv>=1.42.0)
PKG_CHECK_MODULES(H2O     ${_PKG_REQUIRED} libh2o)
PKG_CHECK_MODULES(JANSSON ${_PKG_REQUIRED} jansson>=2.14)
PKG_CHECK_MODULES(BROTLI_DEC ${_PKG_REQUIRED} libbrotlidec)
PKG_CHECK_MODULES(BROTLI_ENC ${_PKG_REQUIRED} libbrotlienc)

PKG_CHECK_MODULES(RHONABWY  ${_PKG_REQUIRED}  librhonabwy>=1.1.9)
PKG_CHECK_MODULES(GNUTLS    ${_PKG_REQUIRED}  gnutls>=3.7.2)
PKG_CHECK_MODULES(NETTLE    ${_PKG_REQUIRED}  nettle>=3.7.2)
PKG_CHECK_MODULES(P11KIT    ${_PKG_REQUIRED}  p11-kit-1>=0.24.0)

PKG_CHECK_MODULES(MARIADB  ${_PKG_REQUIRED}  libmariadb>=3.4.1)
PKG_CHECK_MODULES(RABBITMQ ${_PKG_REQUIRED}  librabbitmq>=0.11.0)

PKG_CHECK_MODULES(AVCODEC   ${_PKG_REQUIRED}  libavcodec>=58.91.100)
PKG_CHECK_MODULES(AVFORMAT  ${_PKG_REQUIRED}  libavformat>=58.45.100)
PKG_CHECK_MODULES(AVUTIL    ${_PKG_REQUIRED}  libavutil>=56.51.100)
PKG_CHECK_MODULES(AVFILTER  ${_PKG_REQUIRED}  libavfilter>=7.85.100)
PKG_CHECK_MODULES(AVDEVICE  ${_PKG_REQUIRED}  libavdevice>=58.10.100)

PKG_CHECK_MODULES(LIBUUID  ${_PKG_REQUIRED} uuid>=2.20.0)
PKG_CHECK_MODULES(LIBCURL  ${_PKG_REQUIRED} libcurl>=7.69.0)
PKG_CHECK_MODULES(LIBNGHTTP2  ${_PKG_REQUIRED} libnghttp2>=1.46.0)

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

SET(ITEST_START_SOURCE_FILES
    src/utils.c
    test/integration/main.c
    test/integration/auth.c
    test/integration/client.c
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
    test/integration/api/discard_committed_file.c )

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

SET(REFMT_EXTENSIONS c h)
SET(REFMT_HEADER_DIRS "${CMAKE_CURRENT_SOURCE_DIR}/include" "${CMAKE_CURRENT_SOURCE_DIR}/test")
SET(HEADER_SEARCH_PATTERNS "")
FOREACH(hdr_dir IN LISTS REFMT_HEADER_DIRS)
    FOREACH(ext IN LISTS REFMT_EXTENSIONS)
        LIST(APPEND HEADER_SEARCH_PATTERNS "${hdr_dir}/*.${ext}")
    ENDFOREACH()
ENDFOREACH()
file(GLOB_RECURSE REFMT_HEADER_FILES ${HEADER_SEARCH_PATTERNS})

SET(HEALTH_CHECK_SOURCE_FILES     src/app_server/health_check.c)

SET(REFMT_SRC_FILES
    ${APP_SERVER_SOURCE_FILES}    src/app_server/main.c
    ${RPC_CONSUMER_SOURCE_FILES}  src/rpc/consumer_main.c
    ${UNIT_TEST_SOURCE_FILES} ${ITEST_START_SOURCE_FILES}
    ${ITEST_RPCCONSUMER_SOURCE_FILES} ${HEALTH_CHECK_SOURCE_FILES}
)

SET(APP_SERVER_EXE_NAME    app_server.out)
SET(RPC_CONSUMER_EXE_NAME  rpc_consumer.out)
SET(HEALTH_CHECK_EXE_NAME  appserver_health_check.out)

ADD_EXECUTABLE(${APP_SERVER_EXE_NAME}   ${APP_SERVER_SOURCE_FILES}    src/app_server/main.c)
ADD_EXECUTABLE(${RPC_CONSUMER_EXE_NAME} ${RPC_CONSUMER_SOURCE_FILES}  src/rpc/consumer_main.c)
ADD_EXECUTABLE(${HEALTH_CHECK_EXE_NAME} ${HEALTH_CHECK_SOURCE_FILES})
TARGET_INCLUDE_DIRECTORIES(${APP_SERVER_EXE_NAME}    PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_INCLUDE_DIRECTORIES(${RPC_CONSUMER_EXE_NAME}  PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_INCLUDE_DIRECTORIES(${HEALTH_CHECK_EXE_NAME}  PUBLIC ${LIBCURL_INCLUDE_DIRS})
TARGET_LINK_LIBRARIES(${APP_SERVER_EXE_NAME}    ${APP_SERVER_DEV_LIBRARIES}   ${EXTRA_LIBS})
TARGET_LINK_LIBRARIES(${RPC_CONSUMER_EXE_NAME}  ${RPC_CONSUMER_DEV_LIBRARIES} ${EXTRA_LIBS})
TARGET_LINK_LIBRARIES(${HEALTH_CHECK_EXE_NAME}  ${LIBCURL_LIBRARIES})

if(ONLY_REFORMAT)
    ADD_CUSTOM_TARGET(reformat
        COMMAND clang-format-18 -i --style=file ${REFMT_SRC_FILES} ${REFMT_HEADER_FILES}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
else()
    SET(UNIT_TEST_EXE_NAME unit_test.out)
    ADD_EXECUTABLE(${UNIT_TEST_EXE_NAME} ${UNIT_TEST_SOURCE_FILES})
    TARGET_INCLUDE_DIRECTORIES(${UNIT_TEST_EXE_NAME} PUBLIC ${CGREEN_INCLUDE_DIR})
    TARGET_LINK_LIBRARIES(${UNIT_TEST_EXE_NAME} ${APP_UNIT_TEST_LIBRARIES} ${CGREEN_LIBRARIES})

    SET(ITEST_START_EXE_NAME    itest_start.out)
    SET(ITEST_RPC_CONSUMER_EXE_NAME  itest_rpc_consumer.out)
    ADD_EXECUTABLE(${ITEST_START_EXE_NAME}   ${ITEST_START_SOURCE_FILES})
    ADD_EXECUTABLE(${ITEST_RPC_CONSUMER_EXE_NAME} ${ITEST_RPCCONSUMER_SOURCE_FILES})
    TARGET_INCLUDE_DIRECTORIES(${ITEST_START_EXE_NAME}    PUBLIC  ${CGREEN_INCLUDE_DIR})
    TARGET_INCLUDE_DIRECTORIES(${ITEST_RPC_CONSUMER_EXE_NAME}  PUBLIC  ${CGREEN_INCLUDE_DIR})
    TARGET_LINK_LIBRARIES(${ITEST_START_EXE_NAME}    ${APP_SERVER_DEV_LIBRARIES}   ${CGREEN_LIBRARIES})
    TARGET_LINK_LIBRARIES(${ITEST_RPC_CONSUMER_EXE_NAME}  ${RPC_CONSUMER_DEV_LIBRARIES} ${CGREEN_LIBRARIES})
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        # Add the linker option -rdynamic for the target
        TARGET_LINK_OPTIONS(${ITEST_START_EXE_NAME} PRIVATE -rdynamic)
        MESSAGE(STATUS "symbols added to app-server executable")
    endif()
endif() # end of ONLY_REFORMAT


# TODO, better way to scale app servers, without manually adding more custom targets
# SET(APP_REP1_CFG_PATH  ${CMAKE_CURRENT_SOURCE_DIR}/settings/dev-replica-1.json)
# SET(ITEST_APPCFG_PATH  ${CMAKE_CURRENT_SOURCE_DIR}/settings/test.json)
# 
# ADD_CUSTOM_TARGET(app_server_replica_1
#     COMMAND
#         ##gdb -args
#         ${CMAKE_CURRENT_SOURCE_DIR}/build/${APP_SERVER_EXE_NAME}  ${APP_REP1_CFG_PATH}
#     WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
#     DEPENDS ${APP_SERVER_EXE_NAME}  VERBATIM)
# 

