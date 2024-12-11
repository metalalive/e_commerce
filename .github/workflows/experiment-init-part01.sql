CREATE DATABASE `test_db_whatever_replica`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

CREATE TABLE `test_db_whatever_replica`.`mytable123` (
  `col3` int(10) unsigned NOT NULL,
  `col4` decimal(16,2) unsigned NOT NULL,
  `col5` enum('Stripe','wise') NOT NULL,
  PRIMARY KEY (`col3`,`col4`)
);

CREATE TABLE `test_db_whatever`.`mytable123` (
  `col3` int(10) unsigned NOT NULL,
  `col4` decimal(16,2) unsigned NOT NULL,
  `col5` enum('Stripe','wise') NOT NULL,
  PRIMARY KEY (`col3`,`col4`)
);

--- note mariaDB docker container does not create the non-root user under the host `localhost`
--- for simplicity in test, I grant the privileges to all hosts which have the username `DB_USERNAME`
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_db_whatever_replica`.* TO 'DB_USERNAME'@'%';
GRANT TRIGGER ON `test_db_whatever`.* TO 'DB_USERNAME'@'%';

-- following grant commands does not work in default mariaDB docker

-- GRANT SELECT ON `mysql`.`user` TO 'DB_USERNAME'@'localhost';
-- GRANT SELECT ON `mysql`.`db` TO 'DB_USERNAME'@'localhost';
-- GRANT SHOW DATABASES ON *.* TO 'DB_USERNAME'@'localhost';

FLUSH PRIVILEGES;
