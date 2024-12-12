REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  FROM  'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* FROM 'test-dba'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* FROM 'dev-dba'@'localhost';
DROP DATABASE `ecommerce_product` ;

