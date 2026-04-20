FROM --platform=$TARGETOS/$TARGETARCH rust:1.93-bookworm

LABEL author="Rustalink" maintainer="https://github.com/bongodevs/Rustalink"

RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    dnsutils \
    curl \
    iproute2 \
    ffmpeg \
    tini \
    pkg-config \
    build-essential \
    cmake \
    clang \
    libclang-dev \
    libopus-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -d /home/container container \
    && mkdir -p /home/container \
    && chown -R container:container /home/container

ENV USER=container \
    HOME=/home/container \
    CARGO_HOME=/home/container/.cargo \
    CC=clang \
    CXX=clang++

WORKDIR /home/container

STOPSIGNAL SIGINT

COPY --chown=container:container ./entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

USER container

ENTRYPOINT ["/usr/bin/tini", "-g", "--"]
CMD ["/entrypoint.sh"]