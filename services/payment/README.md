# Payment service
## Features
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
| Rust toolchain | [rust](https://github.com/rust-lang/rust), including Cargo, Analyzer | `>= 1.75.0` |
| Database | [MariaDB](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB/)| `11.2.3` |


#### Database Migration
If you configure SQL database as the datastore destination in the development server or testing server, ensure to synchronize schema migration
```shell
> cd ${SERVICE_BASE_PATH}
> /PATH/TO/liquibase --defaults-file=/liquibase.properties \
      --changeLogFile=./migrations/changelog_primary-root.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   update

> /PATH/TO/liquibase --defaults-file=/liquibase.properties \
      --changeLogFile=./migrations/changelog_replica.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   update

> /PATH/TO/liquibase --defaults-file=./liquibase.properties \
      --changeLogFile=./migrations/changelog_primary-root.xml  \
      --url=jdbc:mariadb://$HOST:$PORT/$DB_NAME   --username=$USER  --password=$PASSWORD \
      --log-level=info   rollback  $VERSION_TAG
```
Note : 
- the parameters above `$HOST`, `$PORT`, `$USER`, `$PASSWORD` should be consistent with database credential set in `${SYS_BASE_PATH}/common/data/secrets.json` , see the structure in [`common/data/secrets_template.json`](../common/data/secrets_template.json)
- the parameter `$DB_NAME` should be `ecommerce_payment` for development server, or  `test_ecommerce_payment` for testing server, see [reference](../migrations/init_db.sql)
- the subcommand `update` upgrades the schema to latest version
- the subcommand `rollback` rollbacks the schema to specific previous version `$VERSION_TAG` defined in the `migration/changelog_payment.xml`


### Optional features
### Commands for build
## Run
### Development API server
```bash
cargo build --bin web

SYS_BASE_PATH="${PWD}/../"  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json"  cargo run --bin web
```

### Cron Job
```bash
cargo build --bin sync_refund_req

SYS_BASE_PATH="${PWD}/../"  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json"  cargo run --bin sync_refund_req
```

## Development
### Code formatter
```bash
cargo fmt
```
### Linter
```bash
cargo clippy
```
## Test
### Unit Test
```bash
SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    cargo test --test unit -- <specific-test-case-path>  --test-threads=1
```

#### Extra tools for 3rd party interaction
Few test cases interacts with 3rd party processors like Stripe, they require following tools installed before running these tests.
##### [WebDriver](https://developer.mozilla.org/en-US/docs/Web/WebDriver) client
- Provide interface which allows other programs to control web browsers like Chrome or FireFox, so my test code can manipulate the HTML elements of a given webpage hosted in these 3rd-party processor platforms, through the WebDriver interface.
- Remind the payment service in this project must not access all sensitive data like credit-card number, clients should fill the payment form with the sensitive data directly in 3rd-party processor website.
- This is to achieve automated website UI testing, change the charge / refund status during the test if required, the goal is to ensure my code for 3rd party interaction actually works as expected.
- In this payment service, the 3rd party processor code is tested with FireFox browser. Install the [`geckodriver`](https://github.com/mozilla/geckodriver) with the following command

```bash
cargo  install  geckodriver
```

Then start the web driver by running :

```bash
/PATH/TO/geckodriver
```

The path to the driver defaults be the folder for cargo binary executables

### Integration Test
```bash
SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/test.json" cargo test --test integration \
    -- <specific-test-case-path>  --test-threads=1
```
### Reference
- [Web API documentation (OpenAPI v3.0 specification)](./doc/api/openapi.yaml)
