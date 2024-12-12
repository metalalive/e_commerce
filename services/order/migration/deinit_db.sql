REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_order`.*    FROM  'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_order`.* FROM 'test-dba'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_order`.* FROM 'dev-dba'@'localhost';
DROP DATABASE `ecommerce_order`;

