# Product service
This service records product item, package, ingredients, and all necessary attributes for individual sellers.

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.9.6 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|pip| 23.2.1 | [see here](https://pip.pypa.io/en/stable/) |
|OpenSSL| 1.1.1 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |


## Build
### Database Migration
#### Initial migration
```bash
python3 -m  product.setup
```

#### Subsequent migrations
```bash
python3  manage.py  makemigrations  <APP_LABEL>  --settings  product.settings.migration
python3  manage.py  migrate  <APP_LABEL>  <LATEST_MIGRATION_VERSION>  --settings product.settings  --database site_dba
```
Note `<APP_LABEL>` may be `product` or Django internal application `content_type`

#### De-initialization database schema
```bash
python3 -m  product.setup reverse
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
DJANGO_SETTINGS_MODULE='product.settings.development' daphne -p 8009 \
    common.util.python.django.asgi:application  >&  \
    ./tmp/log/dev/product_app.log &
```

### RPC consumer
```bash
DJANGO_SETTINGS_MODULE='product.settings.development'  celery --app=common.util.python \
    --config=product.celeryconfig  worker --concurrency 1 --loglevel=INFO \
    --logfile=./tmp/log/dev/productmgt_celery.log --hostname=productmgt@%h  -E &
```

## Test
### Unit Test
```bash
./product/run_unit_test
```

### Integration Test
```bash
./product/run_integration_test
```

