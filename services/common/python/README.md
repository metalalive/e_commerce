## Common modules in this project
### Build
For python modules :
```shell
poetry build
```

For experimenal C extension modules :
```shell
poetry run python -m build ./c_exts
```
The package named `my-c-extention-lib` can be used in other project applications of this project, with the top-level package name `c_exts` . 
once you installed this package in another project application, you could verify loading / linking with following command :
```shell
python -c "from c_exts.util import keygen"
```

## Cron Job Consumer
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
