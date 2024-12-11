SHOW TABLES FROM `test_db_whatever`;
SHOW TABLES FROM `test_db_whatever_replica`;

CREATE TRIGGER `test_db_whatever`.`experiment_trig`  AFTER INSERT ON `test_db_whatever`.`mytable123`  FOR EACH ROW  INSERT INTO test_db_whatever_replica.mytable123 (`col3`,`col4`,`col5`) VALUES (NEW.`col3`, NEW.`col4`, NEW.`col5`);
          
SHOW TRIGGERS FROM `test_db_whatever`;

INSERT INTO `test_db_whatever`.`mytable123` (`col3`,`col4`,`col5`) VALUES (8964, 290.3 ,'Stripe');

SELECT * FROM `test_db_whatever_replica`.`mytable123`;
