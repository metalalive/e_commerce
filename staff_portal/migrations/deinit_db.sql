
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_fileupload`.* FROM 'restauTestDBA'@'localhost';

REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_fileupload`.* FROM 'restauDBA'@'localhost';

REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt`.*  FROM  'user_mgt_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  FROM  'prodev_admin'@'localhost';

DROP DATABASE `ecommerce_usermgt`   ;
DROP DATABASE `ecommerce_product`   ;
DROP DATABASE `ecommerce_fileupload`;

