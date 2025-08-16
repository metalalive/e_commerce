FROM ubuntu:24.04 AS builder0

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
     common/data/dsl_clean_useraction_log.json \
     common/data/dsl_clean_xpack_monitoring_log.json \
     common/data/nationality_code.json \
     common/data/unit_of_measurement.json \
     .

WORKDIR /app/common/python
COPY common/python/Pipfile  common/python/pyproject.toml \
     common/python/README.md .

# according to dockerdoc, If the source is a directory, the contents of the directory are
# copied, including filesystem metadata. The directory itself isn't copied, only its contents
WORKDIR /app/common/python/src
COPY common/python/src/ecommerce_common ./ecommerce_common
COPY common/python/src/softdelete ./softdelete

WORKDIR /app/common/python/c_exts
COPY common/python/c_exts/pyproject.toml \
     common/python/c_exts/*.c \
     common/python/c_exts/*.py \
     .

WORKDIR  /app/common/python/tests
COPY common/python/tests .

WORKDIR /app/usermgt
RUN python3.13 -m venv /app/usermgt/.venv
ENV PATH="/app/usermgt/.venv/bin:$PATH"

# the command below does not work, `source` is not included in base image
# source /app/usermgt/.venv/bin/activate

# from here python virtual enviroment should be started , it is required since my
# application is built by non-default python version (3.13) and I try to avoid
# breaking OS-default python (3.12)

WORKDIR /app/common/python/c_exts
RUN python3.13 -m pip install build==1.2.2
RUN python3.13 -m build ./

WORKDIR /app/usermgt
COPY user_management/Pipfile  user_management/Pipfile.lock \
     user_management/README.md  user_management/devtools.py \
     user_management/pyproject.toml  user_management/manage.py \
     .

COPY user_management/data  ./data
COPY user_management/src   ./src
COPY user_management/infra/check_db_ready ./infra/

RUN pipenv install
RUN pipenv run pip install /app/common/python/c_exts/dist/my_c_extension_lib-0.0.2-cp313-cp313-linux_x86_64.whl
RUN find /app/usermgt/.venv -depth -type d -name "__pycache__" -exec rm -rf {} +


FROM python:3.13-slim AS final0
RUN apt update && apt install -y --no-install-recommends mariadb-client libmariadb-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/log
WORKDIR /app/entry
VOLUME ["/app/log", "/app/entry"]

WORKDIR /app/common/data
COPY --from=builder0 /app/common/data  .

WORKDIR /app/common/python/src
COPY --from=builder0 /app/common/python/src/ecommerce_common ./ecommerce_common
COPY --from=builder0 /app/common/python/src/softdelete ./softdelete

WORKDIR /app/usermgt
COPY --from=builder0 /app/usermgt/.venv ./.venv
COPY --from=builder0 /app/usermgt/data  ./data
COPY --from=builder0 /app/usermgt/src   ./src
COPY --from=builder0 /app/usermgt/infra/check_db_ready ./infra/
COPY --from=builder0 /app/usermgt/devtools.py  /app/usermgt/manage.py ./

# Ubuntu installs non-system python under `/usr/bin` , however `python:3.13-slim`
# installs default python under `/usr/local/bin` .
# change the symbolic link to resolve path error
RUN unlink ./.venv/bin/python3.13 &&  \
    ln -s /usr/local/bin/python3  ./.venv/bin/python3.13

ENV PATH="/app/usermgt/.venv/bin:$PATH"
CMD ["/bin/sh", "/app/entry/run_my_app"]
