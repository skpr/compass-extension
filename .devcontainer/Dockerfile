# 3.23 is the oldest Alpine release with every PHP version we build against.
FROM alpine:3.23

# Override to build against another PHP, e.g. --build-arg PHP_VERSION=8.6.
ARG PHP_VERSION=8.4

USER root

RUN apk add --no-cache curl && \
    curl -sSL https://packages.skpr.io/php-alpine/skpr.rsa.pub -o /etc/apk/keys/skpr.rsa.pub && \
    echo "https://packages.skpr.io/php-alpine/3.23/php${PHP_VERSION}" >> /etc/apk/repositories

RUN apk add alpine-sdk \
            clang \
            clang-dev \
            git \
            php${PHP_VERSION} \
            php${PHP_VERSION}-dev

ENV MISE_DATA_DIR="/mise"
ENV MISE_CONFIG_DIR="/mise"
ENV MISE_CACHE_DIR="/mise/cache"
ENV MISE_INSTALL_PATH="/usr/local/bin/mise"
ENV PATH="/mise/shims:$PATH"

RUN curl https://mise.run | sh

# Say yes to all the prompts (e.g. trust)
ENV MISE_YES=1

ENV RUST_BACKTRACE=full

WORKDIR /data
