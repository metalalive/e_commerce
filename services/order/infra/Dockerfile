ARG DST_APP_PATH=/app/order
ARG DST_COMMONAPP_PATH=/app/common/rust

FROM rust:1.86-slim AS builder0
ARG DST_APP_PATH
ARG DST_COMMONAPP_PATH

RUN apt update && apt install -y pkg-config libssl-dev  && \
    rm -rf /var/lib/apt/lists/*

COPY common/data/app_code.json  /app/common/data/

WORKDIR ${DST_COMMONAPP_PATH}
COPY common/rust/Cargo.toml common/rust/Cargo.lock common/rust/README.md  . 
COPY common/rust/src  ./src

WORKDIR  ${DST_APP_PATH}
COPY order/Cargo.toml order/Cargo.lock order/README.md  .
COPY order/src         ./src
COPY order/tests/unit  ./tests/unit

ENV SYS_BASE_PATH="${DST_APP_PATH}/../"
ENV SERVICE_BASE_PATH="${DST_APP_PATH}"

# the command below will build executable images for this application
# - the following options can be applied along with the cargo command
#   - `--features mariadb` , to enable MariaDB database in this service.
#   - `--features amqprs` , to enable publish / subscribe operations to AMQP broker in this service.
#   - or enable all options , such as `--features "mariadb amqprs"`

RUN cargo test --release --test unittest --features "mariadb amqprs" -- \
    api::rpc  model usecase auth network adapter::datastore::in_mem \
    repository::in_mem --test-threads=1

WORKDIR  ${DST_APP_PATH}/tests/integration
COPY order/tests/integration/*.rs   order/tests/integration/Cargo.lock \
     order/tests/integration/Cargo.toml  .

RUN cargo build --release --test web --features "mariadb"

FROM debian:trixie-slim AS final0
ARG DST_APP_PATH

RUN apt update && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates

ENV SYS_BASE_PATH="${DST_APP_PATH}/../"
ENV SERVICE_BASE_PATH="${DST_APP_PATH}"
ENV CONFIG_FILE_PATH="settings/your-app-setup.json"

COPY --from=builder0 /app/common/data/app_code.json  /app/common/data/

WORKDIR  ${DST_APP_PATH}
WORKDIR /app/log
WORKDIR /app/entry
VOLUME ["/app/log", "/app/entry"]

COPY --from=builder0 ${DST_APP_PATH}/target/release/rpc_consumer  .
COPY --from=builder0 ${DST_APP_PATH}/target/release/web  .

CMD ["/bin/sh", "/app/entry/run_my_app"]
