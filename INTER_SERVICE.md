## Run essential services
### AMQP message broker
```
service rabbitmq-server start
service rabbitmq-server stop
```

### Elasticsearch server , kibana service
```
service elasticsearch start
service kibana start
service kibana stop
service elasticsearch stop
```
Note :
* Make sure to update default [mapping definition](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/elasticsearch/5.6/basic_usage_cheatsheet.md#mapping) for logging function, which is [log_mapping_template.json](./configure/log_mapping_template.json) to elasticsearch, before it creates first indexed log data.


* update [logstash configure file](./configure/logstash_tcpin_elasticsearch.conf) with the username and password for login to elasticseach, then start Logstash TCP server
```
/PATH/TO/logstash -f  /PROJECT_HOME/configure/logstash_tcpin_elasticsearch.conf --path.settings /etc/logstash/
```
Note logstash TCP input server operates with default port 5959


### Cron Job Consumer and scheduler
See [`README.md`](./services/common/python/README.md) in common python project setup

### TODO
- automate the flow which starts the tools

