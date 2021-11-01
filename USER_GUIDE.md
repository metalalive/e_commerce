
switch to python virtual environment for following steps




### Run the system

Note you may need superuser privilege to run some of the commands.

* start SQL database server
```
/PATH/TO/MARIADB/mysqld_safe --defaults-file=mysql_debug.cnf
```

* start AMQP message broker
```
service rabbitmq-server start
```

* start Elasticsearch server , kibana service
```
service elasticsearch start
service kibana start
```
Note :
* Make sure to update default [mapping definition](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/elasticsearch/5.6/basic_usage_cheatsheet.md#mapping) for logging function, which is [log_mapping_template.json](./configure/log_mapping_template.json) to elasticsearch, before it creates first indexed log data.


* update [logstash configure file](./configure/logstash_tcpin_elasticsearch.conf) with the username and password for login to elasticseach, then start Logstash TCP server
```
/PATH/TO/logstash -f  /PROJECT_HOME/configure/logstash_tcpin_elasticsearch.conf --path.settings /etc/logstash/
```
Note logstash TCP input server operates with default port 5959

* Switch to python virtual environment 

* start task queue processes (celery workers) for each service
```
cd ./staff_portal

DJANGO_SETTINGS_MODULE='common.util.python.django.internal_settings' celery --app=common.util.python --config=common.util.python.celeryconfig   worker --loglevel=INFO -n common@%h  --concurrency=2 --logfile=./tmp/log/staffsite/common_celery.log  -E  -Q mailing,periodic_default

DJANGO_SETTINGS_MODULE='user_management.settings'  celery --app=common.util.python --config=user_management.celeryconfig  worker --loglevel=INFO --hostname=usermgt@%h  --concurrency=2 --logfile=./tmp/log/staffsite/usermgt_celery.log  -E -Q usermgt_default
```
Note:
*  `-Q` is optional, without specifying `-Q`, Celery will enable all queues defined in celery configuration module (e.g. `user_management.celeryconfig`) on initialization.
* `--logfile` is optional
* `--concurrency` indicates number of celery processes to run at OS level, defaults to number of CPU on your host machine


* start cron job scheduler (celery beat), collect all periodic tasks to run (gathered from all services)
```
cd ./staff_portal
celery --app=common.util.python  --config=common.util.python.celerybeatconfig  beat --loglevel=INFO
```

* Finally, start the backend applications shown as follows
```
DJANGO_SETTINGS_MODULE='api.settings' daphne -p 8007  common.util.python.django.asgi:application

python3.9 manage.py runserver  --settings web.settings  8006

python3.9 manage.py runserver  --settings user_management.settings  8008

DJANGO_SETTINGS_MODULE='product.settings' daphne -p 8009  common.util.python.django.asgi:application

FASTAPI_CONFIG_FILEPATH="./common/data/fastapi_cfg.json"  uvicorn --host 127.0.0.1 --port 8010  common.util.python.fastapi.main:app 
```

### Background process
It is optional to launch all these python applications as background processes by append the followings to any of the commands above :
```
<ANY_COMMAND_ABOVE>  >&  <PATH/TO/YOUR_LOG_FILE> &
```


