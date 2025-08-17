#!/bin/bash

# mariaDB docker container , internal behavior (as of v11.8.2) :
# - start temporary server for internal use (why ?)
# - start running all scripts under the path `/docker-entrypoint-initdb.d/`
# - stop temporary server
# - start database server
#
# Important Note :
# - the temporary server is running with a socket file located at `/run/mysqld/mysqld.sock`
#   in the container, strangely it listens to port number zero (0).
# - user-defined startup script is actually running before launching real database server,
#   that mean is it impossible to access the database server using command `mariadb` with
#   IP address / domain name and TCP protocol.
# - the only option is to access this temporary server through a function `docker_process_sql`
#   in `docker-entrypoint.sh`, which is part of mariaDB docker implementation.
# - currently this approach seems hacky, not sure it would be changed a lot in stable
#   versions.

docker_process_sql --verbose <<-EOSQL
  CREATE DATABASE \`$DB_NAME\`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
  CREATE USER '$SITEDBA_UNAME'@'localhost' IDENTIFIED BY '$SITEDBA_PASSWORD';
  CREATE USER '$SITEDBA_UNAME'@'$CONTAINER_IP_RANGE' IDENTIFIED BY '$SITEDBA_PASSWORD';
  GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON \`$DB_NAME\`.* TO '$SITEDBA_UNAME'@'localhost';
  GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON \`$DB_NAME\`.* TO '$SITEDBA_UNAME'@'$CONTAINER_IP_RANGE';
  CREATE USER '$DB_APP_UNAME'@'%' IDENTIFIED BY '$DB_APP_PASSWORD';
  GRANT SELECT, INSERT, UPDATE, DELETE ON \`$DB_NAME\`.*  TO '$DB_APP_UNAME'@'%';
  FLUSH PRIVILEGES;
EOSQL
