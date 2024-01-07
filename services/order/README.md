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
| Rust toolchain | [rust](https://github.com/rust-lang/rust), including Cargo, Analyzer | `>= 1.67.1` |
| DB migration | [liquibase](https://github.com/liquibase/liquibase) | `>= 4.6.2` |

### Optional features
You can build / test this application with following optional features
- mariaDB, append `--features mariadb` to Rust `cargo` command 

### Commands for build
For applications
```shell
cargo build  --bin web
cargo build  --bin rpc_consumer
```

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

## Run
### Development API server
```shell=?
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json" \
    cargo run  --bin web
```
### Development RPC consumer
```shell=?
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json" \
    cargo run  --bin rpc_consumer
```

### Development API server with Debugger
I use the plug-in [vimspector](https://github.com/puremourning/vimspector) with NeoVim, please refer to configuration in `./order/.vimspector` as well as the article [NeoVim IDE setup from scratch](https://hackmd.io/@0V3cv8JJRnuK3jMwbJ-EeA/r1XR_hZL3)

## Test
### Unit Test
Run the test cases collected under `PROJECT_HOME/order/tests/unit`
```shell
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    cargo test --test unittest -- --test-threads=1 --nocapture
```
Note:
- `--test-threads=1` should be added if the feature `mariadb` is enabled and you cannot change maximum number of connections opening at the database backend, this option means you limit number of threads running all the test cases in parallel.
- `--nocapture` is optional to allow the program to print all messages to standard output console.

There are private functions in this source-code crate  containing few test cases :
```shell
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}"  cargo test  api::rpc
```

### Integration Test
For web server
```shell=?
cd ${SERVICE_BASE_PATH}/tests/integration

SYS_BASE_PATH="${PWD}/../../.."  SERVICE_BASE_PATH="${PWD}/../.." \
    CONFIG_FILE_PATH="settings/test-in-mem.json" \
    cargo test --test web
```

Note
- the configuration files in `settings` folder could be `test-in-mem.json` or `test-sql-db.json` for different datastore destinations.


For RPC consumer
```shell=?
cd ${SERVICE_BASE_PATH}/tests/integration

SYS_BASE_PATH="${PWD}/../../.."  SERVICE_BASE_PATH="${PWD}/../.." \
    CONFIG_FILE_PATH="settings/test.json" \
    cargo test --test rpcsub
```

