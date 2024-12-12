CREATE DATABASE `ecommerce_product`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* TO 'test-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* TO 'dev-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  TO 'app-user'@'localhost';

