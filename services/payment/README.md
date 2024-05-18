# Payment service
## Features
## Essential Environment Variables
## Build
### Pre-requisite
| type | name | version required |
|------|------|------------------|
| Rust toolchain | [rust](https://github.com/rust-lang/rust), including Cargo, Analyzer | `>= 1.75.0` |

### Optional features
### Commands for build
## Run
### Development API server
```bash
cargo build --bin web

SYS_BASE_PATH="${PWD}/../"  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/development.json"  cargo run --bin web
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
### Integration Test
```bash
SYS_BASE_PATH="${PWD}/.."  SERVICE_BASE_PATH="${PWD}" \
    CONFIG_FILE_PATH="settings/test.json" cargo test --test integration \
    -- <specific-test-case-path>  --test-threads=1
```
### Reference
- [Web API documentation (OpenAPI v3.0 specification)](./doc/api/openapi.yaml)
