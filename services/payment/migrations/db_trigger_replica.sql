
CREATE DEFINER='app-user'@'localhost'  TRIGGER `test_ecommerce_payment`.`rep_insert_chargeline`  AFTER INSERT ON `test_ecommerce_payment`.`charge_line`  FOR EACH ROW  INSERT INTO `test_ecommerce_payment_replica_1`.`charge_line` (`buyer_id`,`create_time`,`store_id`,`product_id`,`amt_orig_unit`,`amt_orig_total`,`qty_orig`,`qty_rej`,`qty_rfnd`,`amt_rfnd_unit`,`amt_rfnd_total`) VALUES (NEW.`buyer_id`, NEW.`create_time`, NEW.`store_id`, NEW.`product_id`, NEW.`amt_orig_unit`, NEW.`amt_orig_total`, NEW.`qty_orig`, NEW.`qty_rej`, NEW.`qty_rfnd`, NEW.`amt_rfnd_unit`, NEW.`amt_rfnd_total`) ;

CREATE DEFINER='app-user'@'localhost'  TRIGGER `ecommerce_payment`.`rep_insert_chargeline`  AFTER INSERT ON `ecommerce_payment`.`charge_line`  FOR EACH ROW  INSERT INTO `ecommerce_payment_replica_1`.`charge_line` (`buyer_id`,`create_time`,`store_id`,`product_id`,`amt_orig_unit`,`amt_orig_total`,`qty_orig`,`qty_rej`,`qty_rfnd`,`amt_rfnd_unit`,`amt_rfnd_total`) VALUES (NEW.`buyer_id`, NEW.`create_time`, NEW.`store_id`, NEW.`product_id`, NEW.`amt_orig_unit`, NEW.`amt_orig_total`, NEW.`qty_orig`, NEW.`qty_rej`, NEW.`qty_rfnd`, NEW.`amt_rfnd_unit`, NEW.`amt_rfnd_total`) ;

