-- note this file works only with github action test environment

CREATE DATABASE `test_ecommerce_payment_replica_1`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

--- note mariaDB docker container does not create the non-root user under the host `localhost`
--- for simplicity in test, I grant the privileges to all hosts which have the username `DB_USERNAME`

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_payment_replica_1`.* TO 'DB_USERNAME'@'%';

GRANT TRIGGER ON `test_ecommerce_payment`.* TO 'DB_USERNAME'@'%';

FLUSH PRIVILEGES;
