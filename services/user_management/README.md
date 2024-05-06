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
|Python | 3.12.0 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|Elasticsearch| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#elasticsearch) | 
|Logstash| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#logstash) |
|Kibana| 5.6.16 | N/A |
|pipenv | 2023.12.1 | [see here](https://pip.pypa.io/en/stable/) |
|pip| 24.0 | [see here](https://pip.pypa.io/en/stable/) |
|OpenSSL| 3.1.4 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |


## Build
### Virtual Environment
You can create per-project virtual environment using the command:
```bash
PIPENV_VENV_IN_PROJECT=1 pipenv run python -m virtualenv
```
A virtual environment folder `.venv` will be created under the application folder `./user_management`
### Common Python modules
Note in this application the building process on [common python modules](../common/python) is automated , see the `[packages]` section in [`Pipfile`](./Pipfile).

First time to initialize
```shell
pipenv install --dev
```
If you need to modify the `Pipfile` or `pyproject.toml` , update the virtual environment after you are done editing , by the command
```shell
pipenv update
```
Clean up all dependencies in current virtual environment
```shell
pipenv uninstall --all
```

### C extension modules
Manually install it by following command :
```bash
pipenv run pip install ../../common/python/c-ext/ecommerce-common-xxxxx.whl
```

The package title should be `my-c-extention-lib`. Once you need to remove the extension , run
```bash
pipenv run pip uninstall my-c-extention-lib
```

See [the documentation](../common/python/README.md) for build process.

### Database Migration
#### Avoid unused Django models built in database
- Developers can set `managed = False` to the model class `User` and `Group` in module `django.contrib.auth.models`, this service does not need the 2 Django models.
- Alternatively, developers can manually drop the tables `auth_user` or `auth_group` after they are created.

#### Initial migration
```bash
pipenv run python3 -m  user_management.setup
```

The `setup` module above automatically performs following operations :
- Django migration script
  ```bash
  pipenv run python3 manage.py makemigrations user_management  --settings settings.migration
  pipenv run python3 manage.py migrate user_management  <LATEST_MIGRATION_VERSION>  --settings settings.migration  --database site_dba
  ```
- copy custom migration script file(s) at `migrations/django/user_management` to `user_management/migrations` then immediately run the script. These are raw SQL statements required in the application.
- auto-generate default fixture records (which includes default roles, default login users ... etc.) for data migrations in `user_management` application

#### De-initialization
```bash
pipenv run python3 -m  user_management.setup reverse
```

## Run
### application server
```bash
pipenv run python3 ./manage.py runserver --settings  settings.development  8008 \
    >& ../tmp/log/dev/usermgt_app.log &
```

### RPC consumer
```bash
DJANGO_SETTINGS_MODULE="settings.development" SERVICE_BASE_PATH="${PWD}/.."  \
    pipenv run  celery --app=ecommerce_common.util  --config=user_management.celeryconfig \
    worker --concurrency 1 --loglevel=INFO  --hostname=usermgt@%h  -E  \
    --logfile=../tmp/log/dev/usermgt_celery.log  &
```
Note:
*  `-Q` is optional, without specifying `-Q`, Celery will enable all queues defined in celery configuration module (e.g. `user_management.celeryconfig`) on initialization.
* `--logfile` is optional
* `--concurrency` indicates number of celery processes to run at OS level, defaults to number of CPU on your host machine


## Test
### Unit Test
```bash
./run_unit_test
```
### Integration Test
```bash
./run_integration_test
```

## Development
### Code Formatter
```bash
pipenv run black ./src/ ./tests/  ./settings/
```
