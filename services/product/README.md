# Product service
This service records product item, package, ingredients, and all necessary attributes for individual sellers.

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.12.0 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|Poetry| 1.6.1 | [see here](https://python-poetry.org/docs) |
|pip| 24.0.0 | [see here](https://pip.pypa.io/en/stable/) |
|OpenSSL| 3.1.4 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |


Currently, all the build / test commands are running under `v1.0.1` project folder

## Build
### Dependency update
Update all dependency packages specified in the configuration file `v1.0.1/pyproject.toml`
```bash
poetry update
```

### Database Migration
#### Initial migration
```bash
poetry run python3 -m  product.setup
```

#### Subsequent migrations
```bash
poetry run python3  manage.py  makemigrations  <APP_LABEL>  --settings  settings.migration
poetry run python3  manage.py  migrate  <APP_LABEL>  <LATEST_MIGRATION_VERSION>  --settings  settings.migration  --database site_dba
```
Note `<APP_LABEL>` may be `product` or Django internal application `content_type`

#### De-initialization database schema
```bash
poetry run python3 -m  product.setup reverse
```

## Run
### application server
```bash
DJANGO_SETTINGS_MODULE="settings.development" poetry run daphne -p 8009 \
    ecommerce_common.util.django.asgi:application  >&  \
    ./tmp/log/dev/product_app.log &
```

### RPC consumer
```bash
DJANGO_SETTINGS_MODULE="settings.development"  SERVICE_BASE_PATH="${PWD}/../.." \
    poetry run celery --app=ecommerce_common.util  --config=product.celeryconfig \
    --workdir ./src  worker --concurrency 1 --loglevel=INFO  --hostname=productmgt@%h \
    -E  --logfile=./tmp/log/dev/productmgt_celery.log  &
```

## Test
### Unit Test
```bash
./product/v1.0.1/run_unit_test
```

### Integration Test
```bash
./product/v1.0.1/run_integration_test
```

## Development
### Code Formatter
```bash
cd ./v1.0.1
poetry run black ./src/ ./settings/ ./tests/
```
### Linter
```bash
cd ./v1.0.1
poetry run ruff check ./src/  ./settings/ ./tests/
```
