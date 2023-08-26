
### Requirement

| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.9 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|Elasticsearch| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#elasticsearch) | 
|Logstash| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#logstash) |
|Kibana| 5.6.16 | N/A |
|virtualenv| 20.0.20 | N/A|
|OpenSSL| 1.1.1 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |

For each virtual environment, install the python libraries recorded in following files
* [environment #1, for web, authentication, product applications](./staff_portal/requirements_1.txt) 
* [requirement #2, for file-uploading application](./staff_portal/requirements_2.txt) 

Note
* upgragde PIP to latest version after installing python
* install virtualenv , create python virtual environment only for this backend system.
* install C extension built for this project, by running `python ./common/util/c/setup.py install --record ./tmp/setuptools_install_files.txt` . Once you need to remove the installed extension , run `python -m pip uninstall my-c-extention-lib ; rm -rf ./build`
* switch to the virtual environment you created above, before installing all other required libraries.


### Schema Migration
#### Python/Django
For initializing database schema, run the commands below in following order.
```
python3.9 -m  product.setup
```
The modules above automatically performs following operations :
* create `django_migration` database table
* create migration file(s) in the 2 Django applications: `contenttypes` and `auth`
* copy hand-written migration file(s) for  `user_management` application path of the applications, that is, `user_management/migrations`. since there are raw SQL statements required during the migration.
* migrate to database
* auto-generate fixture data (which includes default roles, default login users ... etc.) for data migrations in `user_management` application

For de-initializing database schema, run the commands below.
```
python3.9 -m  product.setup reverse
```

##### Side note
By default Django provides a command which generates migration file template as shown below, the commands below are covered by `user_management.setup` so you do not need to run them manually :
```bash
python3.9 manage.py makemigrations product          --settings product.settings.migration
```

* Then you run `migrate` command on each of the application :

```
python3.9 manage.py migrate product       0004  --settings product.settings  --database site_dba
```



### Test
To run the test suite, execute following commands :
```
source PATH/TO/YOUR_VIRTUAL_ENV/bin/activate
cd ./staff_portal
./run_unit_test
```
You can also run any single test case by copying any line of command in the script file `run_unit_test`

Note
* For Django applications, you can also run specific test case by assigning full path of a test case function. Such as `product.tests.integration.models.SimpleSaleableItemDeletionTestCase.test_soft_delete_bulk_ok` along with the command `./manage.py test`
* you can decide how much detail to print on console by setting different value to `--verbosity` option.
* `--keepdb` keeps database schema after testing, for any test case related to database schema change, you may need to omit the option `--keepdb`

