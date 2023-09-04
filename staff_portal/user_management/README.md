# User-Management service
## Features
- authentication (currently only support JWT and JWKS rotation)
- authorization (currently only support user-level)
- Hierarchical user / group management
- user account management
- Role-based access control
- Quota management

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.9.6 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|Elasticsearch| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#elasticsearch) | 
|Logstash| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#logstash) |
|Kibana| 5.6.16 | N/A |
|pip| 23.2.1 | [see here](https://pip.pypa.io/en/stable/) |
|OpenSSL| 1.1.1 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |


## Build
#### Avoid unused Django models built in database
- Developers can set `managed = False` to the model class `User` and `Group` in module `django.contrib.auth.models`, this service does not need the 2 Django models.
- Alternatively, developers can manually drop the tables `auth_user` or `auth_group` after they are created.

### Database Migration
#### Initial migration
```bash
python3 -m  user_management.setup
```

The `setup` module above automatically performs following operations :
- Django migration script
  ```bash
  python3 manage.py makemigrations user_management  --settings user_management.settings.migration
  python3 manage.py migrate user_management  <LATEST_MIGRATION_VERSION>  --settings user_management.settings  --database site_dba
  ```
- copy custom migration script file(s) at `migrations/django/user_management` to `user_management/migrations` then immediately run the script. These are raw SQL statements required in the application.
- auto-generate default fixture records (which includes default roles, default login users ... etc.) for data migrations in `user_management` application

#### De-initialization
```bash
python3.9 -m  user_management.setup reverse
```

### Experimental C-extension
Currently the package `my-c-extention-lib` is used as demonstration of interfacing low-level C library like OpenSSL. 
```bash
python3 ./common/util/c/setup.py install --record ./tmp/log/dev/setuptools-install-c-exts.log ;
// clean unused built files
rm -rf ./build  ./dist  ./my_c_extention_lib.egg-info ;
```
Once you need to remove the installed extension , run
```bash
python -m pip uninstall my-c-extention-lib;
```

## Run
### application server
```bash
python3 manage.py runserver --settings  user_management.settings.development  8008 >& ./tmp/log/dev/usermgt_app.log &
```

### Cron Job heartbeat
`celerybeat` trigger events to the cron jobs required by all the services.

```bash
celery --app=common.util.python --config=common.util.python.celerybeatconfig \
    beat  --loglevel=INFO --logfile=./tmp/log/dev/celerybeat.log &
```

### RPC consumer
```bash
DJANGO_SETTINGS_MODULE='user_management.settings.development'  celery \
    --app=common.util.python --config=user_management.celeryconfig  worker \
    --concurrency 1 --loglevel=INFO --logfile=./tmp/log/dev/usermgt_celery.log \
    --hostname=usermgt@%h  -E &
```
Note:
*  `-Q` is optional, without specifying `-Q`, Celery will enable all queues defined in celery configuration module (e.g. `user_management.celeryconfig`) on initialization.
* `--logfile` is optional
* `--concurrency` indicates number of celery processes to run at OS level, defaults to number of CPU on your host machine


## Test
### Unit Test
```bash
python3 ./manage.py test tests.python.middlewares.cors  --settings user_management.settings.test  --verbosity=2  
python3 ./manage.py test tests.python.middlewares.csrf  --settings user_management.settings.test  --verbosity=2
python3 -m unittest tests.python.keystore.persistence  -v
python3 -m unittest tests.python.keystore.keygen  -v
python3 -m unittest tests.python.keystore.manager  -v
python3 -m unittest tests.python.util.graph -v
```
### Integration Test
```bash
./user_management/run_integration_test
```

