REVOKE SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_media`.*   FROM 'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    FROM  'app-user'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* FROM 'test-dba'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* FROM 'dev-dba'@'localhost';
DROP DATABASE `test_ecommerce_media`;
DROP DATABASE `ecommerce_media`;

