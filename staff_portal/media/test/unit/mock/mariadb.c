#include <cgreen/mocks.h>
#include <mysql.h>

int STDCALL mysql_server_init(int argc, char **argv, char **groups)
{ return (int)mock(argc, argv, groups); }

void mysql_server_end(void) {}

MYSQL* mysql_init(MYSQL *in)
{ return (MYSQL *)mock(in); }

int  mysql_options(MYSQL *mysql,enum mysql_option option, const void *arg)
{ return (int)mock(mysql, option, arg); }

void mysql_close(MYSQL *mysql)
{ mock(mysql); }

