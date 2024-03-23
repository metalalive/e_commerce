# Order Processing service

## Essential Environment Variables
|variable|description|example|
|--------|-----------|-------|
|`SYS_BASE_PATH`| common path of all the services| `${PWD}/..` |
|`SERVICE_BASE_PATH`| base path of the order service | `${PWD}` |
|`CONFIG_FILE_PATH`| path relative to `SERVICE_BASE_PATH` folder, it is JSON configuration file | `settings/development.json` |
||||

## Build
### Pre-requisite
| type | name | version required |
|------|------|------------------|
| SQL Database | MariaDB | `10.3.22` |
| AMQP broker | RabbitMQ | `3.2.4` |
| Rust toolchain | [rust](https://github.com/rust-lang/rust), including Cargo, Analyzer | `>= 1.67.1` |
| DB migration | [liquibase](https://github.com/liquibase/liquibase) | `>= 4.6.2` |

### Optional features
You can build / test this application with following optional features
- mariaDB, append `--features mariadb` to Rust `cargo` command 

### Commands for build
#### For applications
```shell
cargo build  --bin web
cargo build  --bin rpc_consumer
```
- you can add the following options along with the command `cargo build`
  - `--features mariadb` , to enable MariaDB database in this service.
  - `--features amqprs` , to enable publish / subscribe operations to AMQP broker in this service.
  - or enable all options , such as `--features "mariadb amqprs"`

#### Database Migration
If you configure SQL database as the datastore destination in the development server or testing server, ensure to synchronize schema migration
```shell
> /PATH/TO/liquibase --defaults-file=${SERVICE_BASE_PATH}/liquibase.properties \
      --changeLogFile=${SERVICE_BASE_PATH}/migration/changelog_order.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   update

> /PATH/TO/liquibase --defaults-file=${SERVICE_BASE_PATH}/liquibase.properties \
      --changeLogFile=${SERVICE_BASE_PATH}/migration/changelog_order.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   rollback  $VERSION_TAG
```
Note : 
- the parameters above `$HOST`, `$PORT`, `$USER`, `$PASSWORD` should be consistent with database credential set in `${SYS_BASE_PATH}/common/data/secrets.json` , see the structure in [`common/data/secrets_template.json`](../common/data/secrets_template.json)
- the parameter `$DB_NAME` should be `ecommerce_order` for development server, or  `test_ecommerce_order` for testing server, see [reference](../migrations/init_db.sql)
- the subcommand `update` upgrades the schema to latest version
- the subcommand `rollback` rollbacks the schema to specific previous version `$VERSION_TAG` defined in the `migration/changelog_order.xml`


This order-processing service includes resource quota / permission parameters that should be embedded in client's JWT on calling web API.
It is necessary to synchronize these parameters to `user-management` service by the upgrading database schema :
```shell
/PATH/TO/liquibase --defaults-file=${SERVICE_BASE_PATH}/liquibase.properties \
      --changeLogFile=${SERVICE_BASE_PATH}/migration/changelog_usermgt.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   update
```


## Run
### Development API server
```shell=?
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json" \
    cargo run  --bin web --features "mariadb amqprs"
```
### Development RPC consumer
```shell=?
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json" \
    cargo run  --bin rpc_consumer --features "mariadb amqprs"
```

### Development API server with Debugger
I use the plug-in [vimspector](https://github.com/puremourning/vimspector) with NeoVim, please refer to configuration in `./order/.vimspector` as well as the article [NeoVim IDE setup from scratch](https://hackmd.io/@0V3cv8JJRnuK3jMwbJ-EeA/r1XR_hZL3)

## Test
### Unit Test
Run the test cases collected under `PROJECT_HOME/order/tests/unit`
```shell
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    cargo test --test unittest --features "mariadb amqprs" -- \
    <<OPTIONAL-TEST-ENTRY-MODULE-PATH>> --test-threads=1 --nocapture
```
Note:
- the feature options `mariadb`, `amqprs` can also be applied along with the command `cargo test`
- `--test-threads=1` can be added if the feature `mariadb` is enabled
  - this option means you limit number of threads running all the test cases in parallel.
  - this is to reduce number of connections opening for test at the database backend.
- `--nocapture` is optional to allow the program to print all messages to standard output console.

There are private functions in this source-code crate  containing few test cases :
```shell
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}"  cargo test  api::rpc::test_pycelery_deserialize_error
SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}"  cargo test  api::rpc::test_pycelery_deserialize_ok
SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}"  cargo test  repository::mariadb::verify_hex_to_oidbytes
```

### Integration Test
#### For web server and  RPC consumer
```shell=?
cd ${SERVICE_BASE_PATH}/tests/integration

SYS_BASE_PATH="${PWD}/../../.."  SERVICE_BASE_PATH="${PWD}/../.." \
    CONFIG_FILE_PATH="settings/test.json" \
    cargo test --test web
```

Note :
- remind the option `--features mariadb` can be used for integration test, once added to the command, this option will propagate to `order` crate .


