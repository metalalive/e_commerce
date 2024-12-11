CREATE DATABASE `test_db_whatever_replica`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_db_whatever_replica`.* TO 'DB_USERNAME'@'localhost';

-- following grant commands does not work in default mariaDB docker
-- GRANT SELECT ON `mysql`.`user` TO 'DB_USERNAME'@'localhost';
-- GRANT SELECT ON `mysql`.`db` TO 'DB_USERNAME'@'localhost';

GRANT SHOW DATABASES ON *.* TO 'DB_USERNAME'@'localhost';
FLUSH PRIVILEGES;
