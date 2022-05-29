CMAKE_MINIMUM_REQUIRED(VERSION 3.5.0)
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
PKG_CHECK_MODULES(H2O     REQUIRED libh2o>=0.16.0)
PKG_CHECK_MODULES(JANSSON REQUIRED jansson>=2.14)
PKG_CHECK_MODULES(BROTLI_DEC REQUIRED libbrotlidec)
PKG_CHECK_MODULES(BROTLI_ENC REQUIRED libbrotlienc)

PKG_CHECK_MODULES(RHONABWY  REQUIRED  librhonabwy>=1.1.2)
PKG_CHECK_MODULES(GNUTLS    REQUIRED  gnutls>=3.7.2)
PKG_CHECK_MODULES(NETTLE    REQUIRED  nettle>=3.7.2)
PKG_CHECK_MODULES(P11KIT    REQUIRED  p11-kit-1>=0.24.0)

PKG_CHECK_MODULES(MARIADB  REQUIRED  libmariadb>=3.1.7)
PKG_CHECK_MODULES(RABBITMQ REQUIRED  librabbitmq>=0.11.0)

PKG_CHECK_MODULES(LIBCURL  REQUIRED libcurl>=7.69.0)
PKG_CHECK_MODULES(LIBNGHTTP2  REQUIRED libnghttp2>=1.46.0)

MESSAGE(STATUS "OPENSSL_INCLUDE_DIR : ${OPENSSL_INCLUDE_DIR}")
MESSAGE(STATUS "OPENSSL_LIBRARIES : ${OPENSSL_LIBRARIES}")
MESSAGE(STATUS "H2O_LIBRARY_DIRS : ${H2O_LIBRARY_DIRS}")
MESSAGE(STATUS "H2O_LIBRARIES : ${H2O_LIBRARIES}")
MESSAGE(STATUS "H2O_ROOT_DIR : ${H2O_ROOT_DIR}")
MESSAGE(STATUS "LIBCURL_INCLUDE_DIRS : ${LIBCURL_INCLUDE_DIRS}")
MESSAGE(STATUS "LIBCURL_LIBRARY_DIRS : ${LIBCURL_LIBRARY_DIRS}")
MESSAGE(STATUS "LIBNGHTTP2_INCLUDE_DIRS : ${LIBNGHTTP2_INCLUDE_DIRS}")

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
    ${JANSSON_LIBRARY_DIRS}
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

SET(CMAKE_C_FLAGS_DEBUG  "-O0")
SET(CMAKE_C_FLAGS_RELEASE  "-O2")

SET(APP_HTTPSERVER_SOURCE_FILES
    src/auth.c
    src/cfg_parser.c
    src/middleware.c
    src/network.c
    src/multipart_parser.c
    src/views/init.c
    src/views/common.c
    src/views/initiate_multipart_upload.c
    src/views/complete_multipart_upload.c
    src/views/start_transcoding_file.c
    src/views/upload_part.c )

SET(APP_RPCCONSUMER_HANDLER_SOURCE_FILES
    src/views/async/transcode_video_file.c
    src/views/async/complete_multipart_upload.c )

SET(APP_DBMODEL_SOURCE_FILES
    src/models/pool.c
    src/models/connection.c
    src/models/mariadb.c
    src/models/query.c )

SET(APP_COMMON_SOURCE_FILES
    src/rpc/core.c
    src/rpc/cfg_parser.c
    src/storage/cfg_parser.c
    src/storage/localfs.c
    src/app_cfg.c
    src/utils.c
    src/timer_poll.c
    src/routes.c )

SET(APP_SERVER_SOURCE_FILES
    src/app_server.c
    src/third_party/rhonabwy.c
    ${APP_DBMODEL_SOURCE_FILES}
    ${APP_HTTPSERVER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})

SET(RPC_CONSUMER_SOURCE_FILES
    src/rpc/consumer.c
    ${APP_RPCCONSUMER_HANDLER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})


SET(UNIT_TEST_SOURCE_FILES
    test/unit/main.c
    test/unit/app_cfg.c
    test/unit/cfg_parser.c
    test/unit/network.c
    test/unit/routes.c
    test/unit/auth.c
    test/unit/utils.c
    test/unit/timer_poll.c
    test/unit/middleware.c
    test/unit/multipart_parser.c
    test/unit/models/pool.c
    test/unit/models/connection.c
    test/unit/models/query.c
    test/unit/models/mariadb.c
    test/unit/rpc/cfg_parser.c
    test/unit/rpc/core.c
    test/unit/storage/cfg_parser.c
    test/unit/storage/localfs.c
    test/unit/mock/rhonabwy.c
    test/unit/mock/mariadb.c
    test/unit/mock/rabbitmq.c
    test/unit/mock/openssl.c
    ${APP_DBMODEL_SOURCE_FILES}
    ${APP_HTTPSERVER_SOURCE_FILES}
    ${APP_COMMON_SOURCE_FILES})

SET(INTEGRATION_TEST_SOURCE_FILES
    test/integration/main.c
    test/integration/auth.c
    test/integration/client.c
    test/integration/api/complete_multipart_upload.c
    test/integration/api/initiate_multipart_upload.c
    test/integration/api/upload_part.c
    ${APP_SERVER_SOURCE_FILES} )

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
    ${LIBCURL_LIBRARIES}
    ${LIBNGHTTP2_LIBRARIES}
    ${APP_COMMON_LIBRARIES})

SET(RPC_CONSUMER_DEV_LIBRARIES
    ${OPENSSL_LIBRARIES}
    ${RABBITMQ_LIBRARIES}
    ${APP_COMMON_LIBRARIES})

SET(APP_UNIT_TEST_LIBRARIES ${APP_COMMON_LIBRARIES})


SET(APP_SERVER_EXE_NAME   app_server.out)
SET(RPC_CONSUMER_EXE_NAME rpc_consumer.out)
ADD_EXECUTABLE(${APP_SERVER_EXE_NAME}   ${APP_SERVER_SOURCE_FILES}    src/app_server_main.c)
ADD_EXECUTABLE(${RPC_CONSUMER_EXE_NAME} ${RPC_CONSUMER_SOURCE_FILES}  src/rpc/consumer_main.c)
TARGET_INCLUDE_DIRECTORIES(${APP_SERVER_EXE_NAME}    PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_INCLUDE_DIRECTORIES(${RPC_CONSUMER_EXE_NAME}  PUBLIC ${OPENSSL_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${APP_SERVER_EXE_NAME}    ${APP_SERVER_DEV_LIBRARIES}   ${EXTRA_LIBS})
TARGET_LINK_LIBRARIES(${RPC_CONSUMER_EXE_NAME}  ${RPC_CONSUMER_DEV_LIBRARIES} ${EXTRA_LIBS})

SET(UNIT_TEST_EXE_NAME unit_test.out)
ADD_EXECUTABLE(${UNIT_TEST_EXE_NAME} ${UNIT_TEST_SOURCE_FILES})
TARGET_INCLUDE_DIRECTORIES(${UNIT_TEST_EXE_NAME} PUBLIC ${CGREEN_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${UNIT_TEST_EXE_NAME} ${APP_UNIT_TEST_LIBRARIES} ${CGREEN_LIBRARIES})

SET(INTEGRATION_TEST_EXE_NAME  integration_test.out)
ADD_EXECUTABLE(${INTEGRATION_TEST_EXE_NAME} ${INTEGRATION_TEST_SOURCE_FILES})
TARGET_INCLUDE_DIRECTORIES(${INTEGRATION_TEST_EXE_NAME} PUBLIC ${CGREEN_INCLUDE_DIR})
TARGET_LINK_LIBRARIES(${INTEGRATION_TEST_EXE_NAME} ${APP_SERVER_DEV_LIBRARIES} ${CGREEN_LIBRARIES})


SET(DEV_APPCFG_PATH ${CMAKE_CURRENT_SOURCE_DIR}/settings/development.json)
ADD_CUSTOM_TARGET(dev_app_server
    python -m  media.setup_test_env DevCertRenewal  ${DEV_APPCFG_PATH}
    COMMAND
       ##valgrind --leak-check=full
       ##gdb -args
       ${CMAKE_CURRENT_SOURCE_DIR}/build/${APP_SERVER_EXE_NAME} ${CMAKE_CURRENT_SOURCE_DIR}/settings/development.json
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${APP_SERVER_EXE_NAME})

ADD_CUSTOM_TARGET(dev_rpc_worker
    ##gdb -args
    ${CMAKE_CURRENT_SOURCE_DIR}/build/${RPC_CONSUMER_EXE_NAME} ${CMAKE_CURRENT_SOURCE_DIR}/settings/development.json
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${RPC_CONSUMER_EXE_NAME})


ADD_CUSTOM_TARGET(unit_test
    # uncomment the following line, make cgreen test suite run in single (main) process
    # without forking, then you can set breakpoints using debugger (currently only supports GDB)
    ${CMAKE_COMMAND} -E env "CGREEN_NO_FORK=xxx"
    ##gdb -args # uncomment this line for debugging
    ##valgrind --leak-check=full
    ${CMAKE_CURRENT_SOURCE_DIR}/build/${UNIT_TEST_EXE_NAME}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS ${UNIT_TEST_EXE_NAME})

SET(ITEST_APPCFG_PATH ${CMAKE_CURRENT_SOURCE_DIR}/settings/test.json)

ADD_CUSTOM_TARGET(integration_test
    ##${CMAKE_COMMAND} -E env "LD_LIBRARY_PATH=/path/to/somewhere/else/lib:$ENV{$LD_LIBRARY_PATH}"
    python -m media.setup_test_env  TestCertRenewal    ${ITEST_APPCFG_PATH}
    COMMAND python -m media.setup_test_env  StartTestDatabase  ${ITEST_APPCFG_PATH}  ${LIQUIBASE_PATH}
    COMMAND ${CMAKE_CURRENT_SOURCE_DIR}/build/${RPC_CONSUMER_EXE_NAME} ${ITEST_APPCFG_PATH} &
    COMMAND ${CMAKE_COMMAND} -E env "CGREEN_NO_FORK=xxx"
        gdb -args # uncomment this line for debugging
        ##valgrind --leak-check=full
        ${CMAKE_CURRENT_SOURCE_DIR}/build/${INTEGRATION_TEST_EXE_NAME} ${ITEST_APPCFG_PATH}
    COMMAND python -m  media.setup_test_env EndTestDatabase  ${ITEST_APPCFG_PATH} ${LIQUIBASE_PATH}
    COMMAND python -m  media.setup_test_env KillRPCconsumer  ${ITEST_APPCFG_PATH}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    DEPENDS  ${INTEGRATION_TEST_EXE_NAME} ${RPC_CONSUMER_EXE_NAME})

