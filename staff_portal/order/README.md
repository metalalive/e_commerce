# Order Processing service
## Build
```shell
cargo build  --bin web
```

## Run
### Essential Environment Variables
|variable|description|example|
|--------|-----------|-------|
|`SYS_BASE_PATH`| common path of all the services| `${PWD}/..` |
|`SERVICE_BASE_PATH`| base path of the order service | `${PWD}` |
|`SECRET_FILE_PATH`| path relative to `SYS_BASE_PATH` folder, it contains required credential data | `common/data/secret.json` |
|`CONFIG_FILE_PATH`| path relative to `SERVICE_BASE_PATH` folder, it is JSON configuration file | `settings/development.json` |
||||

### Development API server
```shell=?
cd ${SERVICE_BASE_PATH}

SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    SECRET_FILE_PATH="common/data/secret.json" \
    CONFIG_FILE_PATH="settings/development.json" \
    cargo run  --bin web
```

### Development API server with Debugger
I use the plug-in [vimspector](https://github.com/puremourning/vimspector) with NeoVim, please refer to configuration in `./order/.vimspector` as well as the article [NeoVim IDE setup from scratch](https://hackmd.io/@0V3cv8JJRnuK3jMwbJ-EeA/r1XR_hZL3)

## Test
### Integration Test
```shell=?
cd ${SERVICE_BASE_PATH}/test/acceptance

SYS_BASE_PATH="${PWD}/../../.."  SERVICE_BASE_PATH="${PWD}/../.." \
    SECRET_FILE_PATH="common/data/secret.json" \
    CONFIG_FILE_PATH="settings/test.json" \
    cargo test --test web
```

