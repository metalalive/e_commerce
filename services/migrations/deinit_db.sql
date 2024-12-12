REVOKE SELECT, INSERT, UPDATE, DELETE ON `test_ecommerce_media`.*   FROM 'media_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    FROM  'media_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* FROM 'restauDBA'@'localhost';
DROP DATABASE `ecommerce_media`;

