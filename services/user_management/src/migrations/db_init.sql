CREATE DATABASE `ecommerce_usermgt`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
--- django test automatically creates database, no need to create it manually
CREATE DATABASE `test_ecommerce_usermgt`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt`.* TO 'test-admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt`.* TO 'dev-admin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt`.*  TO 'app-user'@'localhost';
