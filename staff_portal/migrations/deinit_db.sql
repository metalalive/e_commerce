
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_usermgt`.*  FROM  'user_mgt_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_product`.*  FROM  'prodev_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_media`.*    FROM  'media_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_order`.*    FROM  'order_admin'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE ON `ecommerce_store`.*    FROM  'store_admin'@'localhost';

REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_usermgt`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_product`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_media`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_order`.* FROM 'restauTestDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `test_ecommerce_store`.* FROM 'restauTestDBA'@'localhost';

REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_usermgt`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_product`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_media`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_order`.* FROM 'restauDBA'@'localhost';
REVOKE SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER, GRANT OPTION ON `ecommerce_store`.* FROM 'restauDBA'@'localhost';

DROP DATABASE `ecommerce_usermgt` ;
DROP DATABASE `ecommerce_product` ;
DROP DATABASE `ecommerce_media`;
DROP DATABASE `ecommerce_order`;
DROP DATABASE `ecommerce_store`;

