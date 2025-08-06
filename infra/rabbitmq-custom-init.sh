(
    rabbitmqctl --timeout $NUM_SECS_WAIT4READY wait $RABBITMQ_PID_FILE;
    rabbitmqctl -n $RABBITMQ_NODENAME add_user  $RABBITMQ_DEFAULT_USER  $RABBITMQ_DEFAULT_PASS ;
    rabbitmqctl -n $RABBITMQ_NODENAME set_user_tags  $RABBITMQ_DEFAULT_USER  administrator ;
    rabbitmqctl -n $RABBITMQ_NODENAME set_permissions -p /  $RABBITMQ_DEFAULT_USER  ".*" ".*" ".*" ;
    rabbitmqctl -n $RABBITMQ_NODENAME set_permissions -p /integration_test  $RABBITMQ_DEFAULT_USER  ".*" ".*" ".*" ;
    rabbitmqctl -n $RABBITMQ_NODENAME add_user  $RABBIT_TEST_USER  $RABBIT_TEST_PASS ;
    rabbitmqctl -n $RABBITMQ_NODENAME set_user_tags  $RABBIT_TEST_USER  management ;
    rabbitmqctl -n $RABBITMQ_NODENAME set_permissions -p /integration_test  $RABBIT_TEST_USER  ".*" ".*" ".*" ;
    #rabbitmqctl -n $RABBITMQ_NODENAME list_users ;
    #rabbitmqctl -n $RABBITMQ_NODENAME list_vhosts ;
    #rabbitmqctl -n $RABBITMQ_NODENAME list_exchanges  -p / ;
    echo "[custom-init] initialization completed successfully";
) &

exec rabbitmq-server
