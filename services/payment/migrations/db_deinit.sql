REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_payment`.* FROM 'db-dev-admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_payment_replica_1`.* FROM 'db-dev-admin'@'localhost';

REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_payment`.* FROM 'db-test-admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_payment_replica_1`.* FROM 'db-test-admin'@'localhost';

REVOKE SET USER ON  *.* FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, TRIGGER ON `ecommerce_payment`.* FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, TRIGGER ON `test_ecommerce_payment`.* FROM 'app-user'@'localhost';

REVOKE SELECT, INSERT ON `ecommerce_payment_replica_1`.* FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT ON `test_ecommerce_payment_replica_1`.* FROM 'app-user'@'localhost';

DROP DATABASE `ecommerce_payment`;
DROP DATABASE `ecommerce_payment_replica_1`;

DROP DATABASE `test_ecommerce_payment`;
DROP DATABASE `test_ecommerce_payment_replica_1`;

