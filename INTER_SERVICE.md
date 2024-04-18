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


### Cron Job Consumer
```bash
cd ./services/common/python

DJANGO_SETTINGS_MODULE="ecommerce_common.util.django.internal_settings" \
    SERVICE_BASE_PATH="${PWD}/../.."  poetry run  celery --workdir ./src \
    --app=ecommerce_common.util  --config=ecommerce_common.util.celeryconfig \
    worker --loglevel=INFO  -n common@%h  --concurrency=2 \
    --logfile=./tmp/log/dev/common_celery.log   -E \
    -Q mailing,periodic_default
```

- celery log file can be switched to `./tmp/log/test` for testing purpose

### Cron Job scheduler (celery beat)
collect all periodic tasks to run (gathered from all services)
```bash
cd ./services/common/python

SERVICE_BASE_PATH="${PWD}/../.." poetry run celery --workdir ./src \
    --app=ecommerce_common.util  --config=ecommerce_common.util.celerybeatconfig \
     beat --loglevel=INFO
```

### TODO
- automate the flow which starts the tools

