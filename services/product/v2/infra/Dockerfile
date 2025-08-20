ARG DST_APP_PATH=/app/productmgt/v2.x
ARG DST_COMMONAPP_PATH=/app/common/python

FROM ubuntu:24.04 AS builder0
ARG DST_APP_PATH
ARG DST_COMMONAPP_PATH

# Install OS-level dependency , python dependency management tool (poetry).
# and python dependencies for virtual env,

RUN apt update \
    && apt install --no-install-suggests -y software-properties-common libssl-dev \
    && add-apt-repository ppa:deadsnakes/ppa  \
    && apt update \
    && apt install --no-install-suggests -y python3.13 python3.13-dev python3.13-venv  \
    && apt install --no-install-suggests -y gcc \
    && rm -rf /var/lib/apt/lists/*

RUN python3.13 -m ensurepip --default-pip \
    && python3.13 -m pip install --upgrade pip cffi setuptools \
    && python3.13 -m pip install poetry==2.1.4

COPY common/data/app_code.json \
     common/data/nationality_code.json \
     common/data/unit_of_measurement.json \
     /app/common/data/

COPY common/python/pyproject.toml common/python/README.md  ${DST_COMMONAPP_PATH}/

# according to dockerdoc, If the source is a directory, the contents of the directory are
# copied, including filesystem metadata. The directory itself isn't copied, only its contents
WORKDIR ${DST_COMMONAPP_PATH}/src
COPY common/python/src/ecommerce_common ./ecommerce_common

WORKDIR ${DST_COMMONAPP_PATH}/src/softdelete
RUN touch ./__init__.py

WORKDIR ${DST_COMMONAPP_PATH}/c_exts
COPY common/python/c_exts/pyproject.toml \
     common/python/c_exts/*.c \
     common/python/c_exts/*.py \
     .

WORKDIR  ${DST_COMMONAPP_PATH}/tests
COPY common/python/tests .

WORKDIR  ${DST_APP_PATH}
COPY product/v2/pyproject.toml product/v2/poetry.lock product/v2/poetry.toml \
     product/v2/README.md .

COPY product/v2/src   ./src
COPY product/v2/settings/common.py  ./settings/
COPY product/v2/tests/__init__.py   ./tests/

RUN python3.13 -m venv ./.venv
# enter virtual environment by setting PATH env var
ENV PATH="${DST_APP_PATH}/.venv/bin:$PATH"

# from here python virtual enviroment should be started , it is required since my
# application is built by non-default python version (3.13) and I try to avoid
# breaking OS-default python (3.12)
RUN poetry install
RUN find ${DST_APP_PATH}/.venv -depth -type d -name "__pycache__" -exec rm -rf {} +


FROM python:3.13-slim AS final0
ARG DST_APP_PATH
ARG DST_COMMONAPP_PATH

# for nslookup
#RUN apt update && apt install -y dnsutils

WORKDIR /app/log
WORKDIR /app/entry
VOLUME ["/app/log", "/app/entry"]

WORKDIR /app/common/data
COPY --from=builder0 /app/common/data  .

WORKDIR ${DST_APP_PATH}
COPY --from=builder0 ${DST_APP_PATH}/.venv    ./.venv
COPY --from=builder0 ${DST_APP_PATH}/src      ./src
COPY --from=builder0 ${DST_APP_PATH}/settings ./settings

# Ubuntu installs non-system python under `/usr/bin` , however `python:3.13-slim`
# installs default python under `/usr/local/bin` .
# change the symbolic link to resolve path error
RUN unlink ./.venv/bin/python3.13 &&  \
    ln -s /usr/local/bin/python3  ./.venv/bin/python3.13

ENV PATH="${DST_APP_PATH}/.venv/bin:$PATH"
CMD ["/bin/bash", "/app/entry/run_my_app"]

