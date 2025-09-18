FROM python:3.13-slim AS builder0

RUN python3.13 -m ensurepip --default-pip \
    && python3.13 -m pip install --upgrade pip \
    && python3.13 -m pip install cffi setuptools \
    && python3.13 -m pip install poetry==2.1.4

# switch between working directories
WORKDIR /app/common/data
COPY services/common/data/app_code.json \
     services/common/data/nationality_code.json \
     services/common/data/unit_of_measurement.json \
     .

WORKDIR /app/common/python
COPY services/common/python/pyproject.toml \
     services/common/python/README.md .

# according to dockerdoc, If the source is a directory, the contents of the directory are
# copied, including filesystem metadata. The directory itself isn't copied, only its contents
WORKDIR /app/common/python/src
COPY services/common/python/src/ecommerce_common ./ecommerce_common
COPY services/common/python/src/softdelete ./softdelete

WORKDIR /app/common/python/src/ecommerce_common/util
RUN rm  celerybeatconfig.py celeryconfig.py

WORKDIR /app/common/python
RUN poetry config virtualenvs.in-project true && poetry install


FROM python:3.13-slim AS final0

WORKDIR /app/common/data
COPY --from=builder0 /app/common/data .
WORKDIR /app/common/python/src
COPY --from=builder0 /app/common/python/src .

# Copy the virtual environment with installed packages
WORKDIR /app/common/python
COPY --from=builder0 /app/common/python/.venv ./.venv

# Activate the virtual environment by setting the PATH
ENV PATH="/app/common/python/.venv/bin:$PATH"

WORKDIR /app/log
WORKDIR /app/entry
VOLUME ["/app/log", "/app/entry"]

CMD ["/bin/sh", "/app/entry/run_my_app"]
