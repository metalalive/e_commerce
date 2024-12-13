CREATE DATABASE `ecommerce_media`        DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `test_ecommerce_media`   DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* TO 'test-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* TO 'dev-dba'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    TO 'app-user'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_media`.*     TO 'app-user'@'localhost';

