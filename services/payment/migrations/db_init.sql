
CREATE DATABASE `ecommerce_payment`            DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `ecommerce_payment_replica_1`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

CREATE DATABASE `test_ecommerce_payment`            DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;
CREATE DATABASE `test_ecommerce_payment_replica_1`  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_payment`.* TO 'db-dev-admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_payment_replica_1`.* TO 'db-dev-admin'@'localhost';

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_payment`.* TO 'db-test-admin'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_payment_replica_1`.* TO 'db-test-admin'@'localhost';

GRANT SET USER ON  *.* TO 'app-user'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, TRIGGER ON `ecommerce_payment`.* TO 'app-user'@'localhost';
GRANT SELECT, INSERT, UPDATE, DELETE, TRIGGER ON `test_ecommerce_payment`.* TO 'app-user'@'localhost';

GRANT SELECT, INSERT ON `ecommerce_payment_replica_1`.* TO 'app-user'@'localhost';
GRANT SELECT, INSERT ON `test_ecommerce_payment_replica_1`.* TO 'app-user'@'localhost';

