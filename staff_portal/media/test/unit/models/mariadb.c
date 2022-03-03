#include <cgreen/cgreen.h>
#undef is_null  // workaround
//// NOTE: the macro `is_null` in `cgreen/cgreen.h` conflicts the same name in `mysql.h`
#include <cgreen/mocks.h>
#include <cgreen/unit.h>
#include "models/mariadb.h"
#include <mysql.h>

Ensure(app_model_mariadb_init_error) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    expect(mysql_init,  will_return(NULL));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_MEMORY_ERROR));
    assert_that(conn.pool, is_equal_to(NULL));
    assert_that(conn.lowlvl_handle, is_equal_to(NULL));
} // end of app_model_mariadb_init_error


Ensure(app_model_mariadb_init_set_option_error) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    MYSQL expect_mysql = {0};
    expect(mysql_init,  will_return(&expect_mysql));
    expect(mysql_close, when(mysql, is_equal_to(&expect_mysql)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
    expect(mysql_options, will_return(1), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_CONFIG_ERROR));
    assert_that(conn.pool, is_equal_to(NULL));
    assert_that(conn.lowlvl_handle, is_equal_to(NULL));
} // end of app_model_mariadb_init_set_option_error


Ensure(app_model_mariadb_init_test_ok) {
    DBA_RES_CODE result = DBA_RESULT_OK;
    db_conn_t conn = {0};
    db_pool_t pool = {0};
    MYSQL expect_mysql = {0};
    expect(mysql_init,  will_return(&expect_mysql));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_READ_DEFAULT_GROUP)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_NONBLOCK)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_CONNECT_TIMEOUT)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_READ_TIMEOUT)));
    expect(mysql_options, will_return(0), when(option, is_equal_to(MYSQL_OPT_WRITE_TIMEOUT)));
    result = app_db_mariadb_conn_init(&conn, &pool);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    assert_that(conn.pool, is_equal_to(&pool));
    assert_that(conn.lowlvl_handle, is_equal_to(&expect_mysql));
    expect(mysql_close, when(mysql, is_equal_to(&expect_mysql)));
    result = app_db_mariadb_conn_deinit(&conn);
    assert_that(result, is_equal_to(DBA_RESULT_OK));
    assert_that(conn.lowlvl_handle, is_equal_to(NULL));
} // end of app_model_mariadb_init_test_ok


TestSuite *app_model_mariadb_tests(void)
{
    TestSuite *suite = create_test_suite();
    add_test(suite, app_model_mariadb_init_error);
    add_test(suite, app_model_mariadb_init_set_option_error);
    add_test(suite, app_model_mariadb_init_test_ok);
    return suite;
}

