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
### For this application
First time to build / install modules for this application in per-project virtual environment:
```bash
PIPENV_VENV_IN_PROJECT=1 pipenv install --dev
```
Alternatively, you can switch to application base folder and explcitly specify virtual environment
```bash
PIPENV_VENV_IN_PROJECT=1 pipenv run python -m virtualenv
```

- A virtual environment folder `.venv` will be created under the application folder `./store`
- Note [`Pipfile`](./Pipfile) already references path to [common python modules](../common/python), that makes `pipenv` installation automatically link to the common modules, no need to build the common python module explicitly.

If you need to modify the `Pipfile` or `pyproject.toml` later, update the virtual environment after you are done editing `Pipfile` , by the command
```shell
pipenv update
```

### C extension modules (for experiment purpose)
in common python module, build the wheel:
```bash
cd common/python
pipenv run python -m build ./c_exts
```
Then manually install it by following command :
```bash
pipenv run pip install  ../common/python/c_exts/dist/my_c_extension_lib-xxxxx.whl
```

The package title should be `my-c-extention-lib`. Once you need to remove the extension , run
```bash
pipenv run pip uninstall my-c-extention-lib
```

### Database Migration
```bash
pipenv run python -m  store.command  [subcommand] [args]
```

Where `subcommand` could be one of followings :
| `subcommand` | arguments | description |
|-|-|-|
|`auth_migrate_forward`| `N/A` | Always upgrade to latest revision provided by the codebase. Do NOT manually modify the default migration script in `/YOUR-PROJECT-HOME/migrations/alembic/store` |
|`store_migrate_forward`| `--prev-rev-id=<APP_REVISION_ID>  --new-rev-id=<APP_REVISION_ID> --new-label=<APP_REVISION_ID>` | `[prev-rev-id]` is an alphanumeric byte sequence representing revision of current head in the migration history. <br><br> `[new-rev-id]` has the same format as `[prev-rev-id]` but it labels revision of the new head. <br><br> For initial migration upgrade, `[prev-rev-id]` has to be `init`. <br><br> Example : `python3 -m store.command store_migrate_forward  init  00001  init-app-tables` |
|`migrate_backward`| `--prev-rev-id=<APP_REVISION_ID>` | `--prev-rev-id` is an alphanumeric byte sequence representing revision at previous point of the migration history. <br><br> To downgrade back to initial state , `--prev-rev-id` can also be `base`, which removes all created schemas and entire migration folder.<br><br> |
||||

Examples:
```bash
pipenv run python ./command.py store_migrate_forward --prev-rev-id=init \
    --new-rev-id=000001  --new-label=create-all-tables
pipenv run python ./command.py  migrate_backward  --prev-rev-id=base

```

## Run
### Development Server
```bash
APP_SETTINGS="settings.development" pipenv run uvicorn  --host 127.0.0.1 \
    --port 8011 store.entry.web:app  >& ./tmp/log/dev/store_app.log &
```

### RPC Consumer
```bash
SYS_BASE_PATH="${PWD}/.." PYTHONPATH="${PYTHONPATH}:${PWD}/settings"   pipenv run celery \
    --app=ecommerce_common.util  --config=settings.development   --workdir ./src  worker \
    --concurrency 1  --loglevel=INFO  --hostname=storefront@%h  -E
```

### Production Server
(TODO)

## Test
### Integration Test
```bash
./run_test
```

## Development
### Code Formatter
```bash
pipenv run black ./src/ ./tests/  ./settings/ ./command.py
```
