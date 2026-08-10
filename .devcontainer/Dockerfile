# The skpr package repository path couples the Alpine release to the PHP version,
# so both are build args and must be moved together. Keep the defaults matching
# the version the compose image ships (see docker/compose/php-fpm/Dockerfile).
ARG ALPINE_VERSION=3.21

FROM alpine:${ALPINE_VERSION}

# Re-declared: args from before the first FROM are only visible to FROM itself.
ARG ALPINE_VERSION=3.21
ARG PHP_VERSION=8.4

USER root

RUN apk add --no-cache curl && \
    curl -sSL https://packages.skpr.io/php-alpine/skpr.rsa.pub -o /etc/apk/keys/skpr.rsa.pub && \
    echo "https://packages.skpr.io/php-alpine/${ALPINE_VERSION}/php${PHP_VERSION}" >> /etc/apk/repositories

# pdo and pdo_sqlite are needed to exercise the database probes: the extension
# hooks PDOStatement::execute, PDO::query and PDO::exec, so without them
# scripts/fixture.php cannot reach those code paths.
RUN apk add alpine-sdk \
            clang \
            clang-dev \
            git \
            php${PHP_VERSION} \
            php${PHP_VERSION}-dev \
            php${PHP_VERSION}-pdo \
            php${PHP_VERSION}-pdo_sqlite

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
