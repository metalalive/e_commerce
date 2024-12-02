
CREATE DATABASE `ecommerce_store`          DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `test_ecommerce_store`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_store`.* TO 'db-dev-admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_store`.* TO 'db-test-admin'@'localhost';

GRANT SET USER ON  *.* TO 'app-user'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_store`.* TO 'app-user'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_store`.* TO 'app-user'@'localhost';

