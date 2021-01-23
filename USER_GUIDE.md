
switch to python virtual environment for following steps

### Migration

* no need to run `django makemigrations`
* create `django_migration` database table

```
python3.9 manage.py migrate contenttypes  0002
python3.9 manage.py migrate auth          0018
python3.9 manage.py migrate user_management 0033
```

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

* start task queue service (celery workers)
```
cd ./staff_portal
celery --app=common.util worker --loglevel=INFO -E -Q mailing,reporting,periodic_default,celery
```

* start cron job scheduler (celery beat)
```
cd ./staff_portal
celery --app=common.util  beat --loglevel=INFO
```

* Finally, start backend application service
```
python3.9 manage.py runserver  <YOUR_APP_SERVER_PORT>
```


