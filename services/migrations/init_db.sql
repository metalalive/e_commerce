
CREATE DATABASE `ecommerce_media`       DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_order`       DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* TO 'restauTestDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_order`.* TO 'ecomSite2TestAdmin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* TO 'restauDBA'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_order`.* TO 'ecomSite2DBA'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    TO 'media_admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_order`.*    TO 'order_admin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_media`.*     TO 'media_admin'@'localhost';

