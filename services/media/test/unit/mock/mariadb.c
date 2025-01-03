#include <cgreen/mocks.h>
#include <mysql.h>

int STDCALL mysql_server_init(int argc, char **argv, char **groups)
{ return (int)mock(argc, argv, groups); }

void mysql_server_end(void) {}

MYSQL* mysql_init(MYSQL *in)
{ return (MYSQL *)mock(in); }

int  mysql_optionsv(MYSQL *mysql, enum mysql_option option, ...)
{ return (int)mock(mysql, option); }

void mysql_close(MYSQL *mysql)
{ mock(mysql); }

unsigned int STDCALL mysql_errno(MYSQL *mysql)
{ return (unsigned int)mock(mysql); }

const char * STDCALL mysql_error(MYSQL *mysql)
{ return (const char *)mock(mysql); }

unsigned int STDCALL mysql_get_timeout_value_ms(const MYSQL *mysql)
{ return (unsigned int)mock(mysql); }

MYSQL_RES *	STDCALL mysql_use_result(MYSQL *mysql)
{ return (MYSQL_RES *)mock(mysql); }

my_ulonglong STDCALL mysql_affected_rows(MYSQL *mysql)
{ return (my_ulonglong)mock(mysql); }

unsigned int STDCALL mysql_num_fields(MYSQL_RES *res)
{ return (unsigned int)mock(res); }

my_socket STDCALL mysql_get_socket(MYSQL *mysql)
{ return (my_socket)mock(mysql); }


int  STDCALL mysql_free_result_start(MYSQL_RES *result)
{ return (int)mock(result); }

int  STDCALL mysql_free_result_cont(MYSQL_RES *result, int status)
{ return (int)mock(result, status); }

int  STDCALL mysql_fetch_row_start(MYSQL_ROW *ret, MYSQL_RES *result)
{ return (int)mock(ret, result); }

int  STDCALL mysql_fetch_row_cont(MYSQL_ROW *ret, MYSQL_RES *result, int status)
{ return (int)mock(ret, result, status); }

int STDCALL mysql_real_connect_start(MYSQL **ret, MYSQL *mysql, const char *host,
     const char *user,  const char *passwd, const char *db, unsigned int port,
     const char *unix_socket, unsigned long clientflag)
{
    return (int) mock(ret, mysql, host, user, passwd, db, port,
            unix_socket, clientflag);
}

int STDCALL mysql_real_connect_cont(MYSQL **ret, MYSQL *mysql, int status)
{ return (int)mock(ret, mysql, status); }

int STDCALL mysql_close_start(MYSQL *sock)
{ return (int)mock(sock); }

int STDCALL mysql_close_cont(MYSQL *sock, int status)
{ return (int)mock(sock, status); }

int  STDCALL mysql_real_query_start(int *ret, MYSQL *mysql, const char *q, unsigned long length)
{ return (int)mock(ret, mysql, q, length); }

int  STDCALL mysql_real_query_cont(int *ret, MYSQL *mysql, int status)
{ return (int)mock(ret, mysql, status); }

int STDCALL mysql_next_result_start(int *ret, MYSQL *mysql)
{ return (int)mock(ret, mysql); }

int STDCALL mysql_next_result_cont(int *ret, MYSQL *mysql, int status)
{ return (int)mock(ret, mysql, status); }


