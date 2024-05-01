# Store-Front service
## Features
- manage price of available products for sale

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.12.0 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 11.2.3 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB/) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|pipenv | 2023.12.1 | [see here](https://pip.pypa.io/en/stable/) |
|pip| 24.0 | [see here](https://pip.pypa.io/en/stable/) |
|OpenSSL| 3.1.4 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |


## Build
### Database Migration
```bash
pipenv run python -m  store.command  [subcommand] [args]
```

Where `subcommand` could be one of followings :
| `subcommand` | arguments | description |
|-|-|-|
|`auth_migrate_forward`| `N/A` | Always upgrade to latest revision provided by the codebase. Do NOT manually modify the default migration script in `/YOUR-PROJECT-HOME/migrations/alembic/store` |
|`store_migrate_forward`| `[prev-rev-id]  [new-rev-id]  [new-label]` | `[prev-rev-id]` is an alphanumeric byte sequence representing revision of current head in the migration history. <br><br> `[new-rev-id]` has the same format as `[prev-rev-id]` but it labels revision of the new head. <br><br> For initial migration upgrade, `[prev-rev-id]` has to be `init`. <br><br> Example : `python3 -m store.command store_migrate_forward  init  00001  init-app-tables` |
|`migrate_backward`| `[target-rev-id]` | `[target-rev-id]` is an alphanumeric byte sequence representing revision at previous point of the migration history. <br><br> To downgrade back to initial state `[target-rev-id]` can also be `init`, which removes all created schemas and entire migration folder.<br><br> Example : `python3 -m store.command  migrate_backward  00001` |
||||


## Run
### Development Server
```bash
APP_SETTINGS="settings.development" pipenv run uvicorn  --host 127.0.0.1 \
    --port 8011 store.entry:app  >& ./tmp/log/dev/store_app.log &
```

### Production Server
(TODO)

## Test
### Integration Test
```bash
APP_SETTINGS="settings.test" SERVICE_BASE_PATH="${PWD}/.." pipenv run pytest -v -s --keepdb ./tests/storeprofile/models.py
APP_SETTINGS="settings.test" SERVICE_BASE_PATH="${PWD}/.." pipenv run pytest -v -s --keepdb ./tests/storeprofile/api.py
APP_SETTINGS="settings.test" pipenv run pytest -v -s --keepdb ./tests/staff.py
APP_SETTINGS="settings.test" pipenv run pytest -v -s --keepdb ./tests/business_hours.py
APP_SETTINGS="settings.test" pipenv run pytest -v -s --keepdb ./tests/products.py
```

