REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_store`.* FROM 'db-dev-admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_store`.* FROM 'db-test-admin'@'localhost';

REVOKE SET USER ON  *.* FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_store`.* FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_store`.* FROM 'app-user'@'localhost';

DROP DATABASE `ecommerce_store`;
DROP DATABASE `test_ecommerce_store`;
