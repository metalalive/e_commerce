ARG DST_APP_PATH=/app/media
ARG H2O_CLONE_DST=/app/3pty/h2o
ARG FFM_CLONE_DST=/app/3pty/ffmpeg
ARG H2O_INSTALLED_PATH=/app/3pty/installed/h2o
ARG FFM_INSTALLED_PATH=/app/3pty/installed/ffmpeg
# ---- Stage 1: build + dev ----
FROM gcc:14-trixie as builder0
ARG DST_APP_PATH
ARG H2O_CLONE_DST
ARG FFM_CLONE_DST
ARG H2O_INSTALLED_PATH
ARG FFM_INSTALLED_PATH

# allow non-free packages to be installed
RUN sed -i -E 's/^(Components:.*)$/\1 non-free/'  /etc/apt/sources.list.d/debian.sources

# allow only specific packages from non-free licensing component can be installed,
# this mitigates security vulnerability.
#
# reference : apt configuration in Debian Wiki
# https://wiki.debian.org/AptConfiguration?action=show&redirect=AptPreferences

RUN cat <<EOF > /etc/apt/preferences.d/non-free-default-policy
  Explanation: Disable packages from non-free tree by default
  Package: *
  Pin: release o=Debian,a=stable,l=Debian,c=non-free
  Pin-Priority: -1
EOF

RUN cat <<EOF > /etc/apt/preferences.d/non-free-allowed
  Explanation: Enable packages from non-free tree
  Package: libfdk-aac-dev
  Pin: release o=Debian,a=stable,l=Debian,c=non-free
  Pin-Priority: 600
EOF

# `libbrotli` is required for `libh2o` http library
# `curl`, `nghttp2`, and `cgreen` are required in integration test
# `nettle` and `gnutls` are required in `rhonabwy` (TODO, switch to other JWT library)
# `libelf` and `cmagic` are required in this media application.

RUN apt update && apt install -y --no-install-recommends \
    build-essential make cmake pkg-config git

RUN apt install -y --no-install-recommends \
    libmariadb-dev librabbitmq-dev libssl-dev libjansson-dev libbrotli-dev \
    libuv1-dev librhonabwy-dev nettle-dev  libgnutls28-dev libcurl4-openssl-dev \
    yasm  libfdk-aac-dev  libx264-dev  libx265-dev  libcgreen1-dev \
    libelf-dev libmagic-dev uuid-dev libnghttp2-dev \
    && rm -rf /var/lib/apt/lists/*

# Add script to clone h2o repository
WORKDIR  ${H2O_CLONE_DST}
RUN if [ -z "$(ls -A ${H2O_CLONE_DST})" ]; then \
        echo "Cloning h2o repository into ${H2O_CLONE_DST}"; \
        git init; \
        git remote add -t master origin https://github.com/h2o/h2o.git; \
        git fetch --depth 1 origin b49276113a92d6866d0e09d8dafb11341647bebe; \
        git checkout FETCH_HEAD; \
        git branch master; \
        git checkout master; \
    else \
        echo "${H2O_CLONE_DST} is not empty, skipping git clone."; \
    fi

# Build and install libh2o
WORKDIR  ${H2O_CLONE_DST}/build
RUN cmake -DCMAKE_INSTALL_PREFIX=${H2O_INSTALLED_PATH} -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_BUILD_TYPE=Release -DWITH_MRUBY=OFF  -DWITH_FUSION=OFF  ${H2O_CLONE_DST} && \
    make libh2o && \
    rm -rf ${H2O_INSTALLED_PATH} && mkdir -p ${H2O_INSTALLED_PATH} && \
    make install;


WORKDIR  ${FFM_CLONE_DST}
RUN git init && \
    git remote add -t master origin123 https://github.com/FFmpeg/FFmpeg.git && \
    git fetch --depth 1 origin123 tag n4.4.6 && \
    git checkout FETCH_HEAD && \
    git branch master && \
    git checkout master

RUN ./configure --prefix=${FFM_INSTALLED_PATH} --enable-libx264 --enable-libx265 \
    --enable-libfdk-aac  --enable-openssl --enable-gpl --enable-nonfree --enable-shared \
    --disable-static --disable-programs  --disable-ffplay --disable-ffmpeg --disable-doc  \
    --disable-htmlpages  --disable-manpages  --disable-podpages --disable-txtpages \
    --disable-large-tests && \
    make && \
    rm -rf ${FFM_INSTALLED_PATH} && mkdir -p ${FFM_INSTALLED_PATH} && \
    make install


WORKDIR  ${DST_APP_PATH}
COPY media/CMakeLists.txt  media/py_venv_requirement.txt  .
COPY media/include   ./include
COPY media/src       ./src

WORKDIR  ${DST_APP_PATH}/test
COPY media/test/unit ./unit

WORKDIR  ${DST_APP_PATH}/test/integration
COPY media/test/integration/*.[ch]   .
COPY media/test/integration/api      ./api

WORKDIR ${DST_APP_PATH}/build

ENV PKG_CONFIG_PATH="${FFM_INSTALLED_PATH}/lib/pkgconfig:${H2O_INSTALLED_PATH}/lib/pkgconfig:/usr/local/lib/pkgconfig:/usr/lib/pkgconfig"

ENV LD_LIBRARY_PATH="/usr/lib:/usr/libexec:/usr/local/lib"

RUN cmake -DCMAKE_BUILD_TYPE=Debug  -DCMAKE_EXPORT_COMPILE_COMMANDS=1  ${DST_APP_PATH}

RUN make app_server.out && make rpc_consumer.out
RUN make unit_test.out && make itest_app_server.out && make itest_rpc_consumer.out

# Keep this as a named image for running unit tests
# docker build --target build -t myapp:build .

# ---- Stage 2: runtime ----
FROM python:3.13-slim as final0
ARG DST_APP_PATH
ARG H2O_INSTALLED_PATH
ARG FFM_INSTALLED_PATH

RUN sed -i -E 's/^(Components:.*)$/\1 non-free/'  /etc/apt/sources.list.d/debian.sources

RUN cat <<EOF > /etc/apt/preferences.d/non-free-default-policy
  Explanation: Disable packages from non-free tree by default
  Package: *
  Pin: release o=Debian,a=stable,l=Debian,c=non-free
  Pin-Priority: -1
EOF

RUN cat <<EOF > /etc/apt/preferences.d/non-free-allowed
  Explanation: Enable packages from non-free tree
  Package: libfdk-aac2
  Pin: release o=Debian,a=stable,l=Debian,c=non-free
  Pin-Priority: 501
EOF

RUN apt update && apt install -y --no-install-recommends gdb valgrind

# libhogweed6t64  libgmp-dev  liborcania-dev, libyder-dev  zlib1g-dev 
# libgnutls-dane0t64  libgnutls-openssl27t64  libidn2-dev, libp11-kit-dev, libtasn1-6-dev, 
# yasm  zlib1g-dev, libzstd-dev

RUN apt install -y --no-install-recommends \
  libmariadb3  librabbitmq4  libjansson4  libbrotli1  libuv1t64 \
  librhonabwy1.1t64  libnettle8t64  libgnutls30t64  libcurl4t64 \
  libfdk-aac2  libx264-164  libx265-215  libcgreen1 libelf1t64 \
  libmagic1t64  libuuid1  libnghttp2-14 && \
  rm -rf /var/lib/apt/lists/*

WORKDIR ${H2O_INSTALLED_PATH}
COPY --from=builder0 ${H2O_INSTALLED_PATH}  .

WORKDIR ${FFM_INSTALLED_PATH}
COPY --from=builder0 ${FFM_INSTALLED_PATH}  .

WORKDIR  ${DST_APP_PATH}
COPY --from=builder0  ${DST_APP_PATH}/py_venv_requirement.txt  .
RUN pip3 install -r ./py_venv_requirement.txt

WORKDIR  ${DST_APP_PATH}/build
COPY --from=builder0 ${DST_APP_PATH}/build/app_server.out \
    ${DST_APP_PATH}/build/rpc_consumer.out \
    ${DST_APP_PATH}/build/unit_test.out          \
    ${DST_APP_PATH}/build/itest_app_server.out   \
    ${DST_APP_PATH}/build/itest_rpc_consumer.out \
    .

ENV LD_LIBRARY_PATH="${FFM_INSTALLED_PATH}/lib:${H2O_INSTALLED_PATH}/lib:/usr/lib:/usr/libexec:/usr/local/lib"

WORKDIR /app/log
VOLUME ["/app/log"]

CMD ["/bin/bash", "/app/entry/run_my_app"]
