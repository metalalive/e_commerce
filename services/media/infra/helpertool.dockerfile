ARG DST_APP_PATH=/app/media

FROM python:3.13-slim
ARG DST_APP_PATH

WORKDIR  ${DST_APP_PATH}
COPY media/py_venv_requirement.txt  .

RUN pip3 install -r ./py_venv_requirement.txt

