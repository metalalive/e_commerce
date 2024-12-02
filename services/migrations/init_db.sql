
CREATE DATABASE `ecommerce_usermgt_v2`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_product`     DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_media`       DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_order`       DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt_v2`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_order`.* TO 'ecomSite2TestAdmin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt_v2`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_order`.* TO 'ecomSite2DBA'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt_v2`.*  TO 'user_mgt_admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  TO 'prodev_admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    TO 'media_admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_order`.*    TO 'order_admin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_media`.*     TO 'media_admin'@'localhost';

