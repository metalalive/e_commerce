
switch to python virtual environment for following steps

### Migration

* no need to run `django makemigrations`
* create `django_migration` database table

```
python3.9 manage.py migrate contenttypes  0002   --settings user_management.settings  --database site_dba
python3.9 manage.py migrate auth          0018   --settings user_management.settings  --database site_dba
python3.9 manage.py migrate user_management 0033 --settings user_management.settings  --database site_dba 
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
Note logstash TCP input server operates with default port 5959

* Switch to python virtual environment 

* start task queue processes (celery workers) for each service
```
cd ./staff_portal

DJANGO_SETTINGS_MODULE='restaurant.global_settings' celery --app=common.util.python --config=common.util.python.celeryconfig   worker --loglevel=INFO -n common@%h  -E  -Q mailing,periodic_default

DJANGO_SETTINGS_MODULE='user_management.settings'  celery --app=common.util.python --config=user_management.celeryconfig  worker --loglevel=INFO --hostname=usermgt@%h  -E -Q usermgt_default
```


* start cron job scheduler (celery beat), collect all periodic tasks to run (gathered from all services)
```
cd ./staff_portal
celery --app=common.util.python  --config=common.util.python.celerybeatconfig  beat --loglevel=INFO
```

* Finally, start backend application service
```
python3.9 manage.py runserver  --settings web.settings  8006
python3.9 manage.py runserver  --settings api.settings  8007
python3.9 manage.py runserver  --settings user_management.settings  8008
```


