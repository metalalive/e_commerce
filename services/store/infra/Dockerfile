ARG DST_APP_PATH=/app/store
ARG DST_COMMONAPP_PATH=/app/common/python

FROM ubuntu:24.04 AS builder0
ARG DST_APP_PATH
ARG DST_COMMONAPP_PATH

# Install OS-level dependency , python dependency management tool (pipenv).
# and python dependencies for virtual env,

RUN apt update \
    && apt install --no-install-suggests -y software-properties-common  \
    && add-apt-repository ppa:deadsnakes/ppa  \
    && apt update \
    && apt install --no-install-suggests -y python3.13 python3.13-dev \
       python3.13-venv  mariadb-client  libmariadb-dev \
    && apt install --no-install-suggests -y gcc \
    && rm -rf /var/lib/apt/lists/*

# use the pip bound to specific python version , instead of system-level
# `python3-pip` from APT

RUN python3.13 -m ensurepip --default-pip \
    && python3.13 -m pip install --upgrade pip \
    && python3.13 -m pip install pipenv

# switch between working directories
WORKDIR /app/common/data
COPY common/data/app_code.json \
     common/data/nationality_code.json \
     common/data/unit_of_measurement.json \
     .

WORKDIR ${DST_COMMONAPP_PATH}
COPY common/python/Pipfile  common/python/pyproject.toml \
     common/python/README.md .

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

WORKDIR ${DST_COMMONAPP_PATH}/tests
COPY common/python/tests .

WORKDIR ${DST_APP_PATH}
ENV PIPENV_VENV_IN_PROJECT=1

# do NOT use `-m virtualenv` to create virtual environment
RUN python3.13 -m venv ${DST_APP_PATH}/.venv
ENV PATH="${DST_APP_PATH}/.venv/bin:$PATH"
RUN ls -lt ${DST_APP_PATH}/.venv/bin/python3


# the command below does not work, `source` is not included in base image
# source /app/store/.venv/bin/activate

# from here python virtual enviroment should be started , it is required since my
# application is built by non-default python version (3.13) and I try to avoid
# breaking OS-default python (3.12)

WORKDIR ${DST_COMMONAPP_PATH}/c_exts
RUN python3.13 -m pip install build==1.2.2
RUN python3.13 -m build ./

WORKDIR ${DST_APP_PATH}
COPY store/Pipfile  store/Pipfile.lock store/README.md \
     store/pyproject.toml  .

COPY store/src   ./src
COPY store/migrations  ./migrations

RUN pipenv install --dev
RUN pipenv run pip install ${DST_COMMONAPP_PATH}/c_exts/dist/my_c_extension_lib-0.0.2-cp313-cp313-linux_x86_64.whl
RUN find ${DST_APP_PATH}/.venv -depth -type d -name "__pycache__" -exec rm -rf {} +


FROM python:3.13-slim AS final0
ARG DST_APP_PATH
ARG DST_COMMONAPP_PATH

# RUN apt update && apt install -y --no-install-recommends mariadb-client libmariadb-dev \
#    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/log
WORKDIR /app/entry
VOLUME ["/app/log", "/app/entry"]

WORKDIR /app/common/data
COPY --from=builder0 /app/common/data  .

WORKDIR ${DST_COMMONAPP_PATH}/src
COPY --from=builder0 ${DST_COMMONAPP_PATH}/src/ecommerce_common ./ecommerce_common
COPY --from=builder0 ${DST_COMMONAPP_PATH}/src/softdelete ./softdelete

WORKDIR ${DST_APP_PATH}
COPY --from=builder0 ${DST_APP_PATH}/.venv     ./.venv
COPY --from=builder0 ${DST_APP_PATH}/src       ./src
COPY --from=builder0 ${DST_APP_PATH}/migrations  ./migrations

# Ubuntu installs non-system python under `/usr/bin` , however `python:3.13-slim`
# installs default python under `/usr/local/bin` .
# change the symbolic link to resolve path error
RUN unlink ./.venv/bin/python3.13 &&  \
    ln -s /usr/local/bin/python3  ./.venv/bin/python3.13

ENV PATH="${DST_APP_PATH}/.venv/bin:$PATH"
CMD ["/bin/sh", "/app/entry/run_my_app"]
