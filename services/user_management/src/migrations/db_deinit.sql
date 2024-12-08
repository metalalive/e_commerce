REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt`.*  FROM  'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt`.* FROM 'test-admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt`.* FROM 'dev-admin'@'localhost';
DROP DATABASE `test_ecommerce_usermgt`;
DROP DATABASE `ecommerce_usermgt`;

