CREATE DATABASE `ecommerce_order`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_order`.* TO 'test-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_order`.* TO 'dev-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_order`.*  TO 'app-user'@'localhost';

