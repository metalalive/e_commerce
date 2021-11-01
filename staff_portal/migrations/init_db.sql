
CREATE DATABASE `ecommerce_usermgt`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_product`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_fileupload`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_fileupload`.* TO 'restauTestDBA'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_fileupload`.* TO 'restauDBA'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt`.*  TO 'user_mgt_admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  TO 'prodev_admin'@'localhost';

