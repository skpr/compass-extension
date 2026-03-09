FROM ghcr.io/skpr/php-cli:8.3-v2-stable AS build

  USER root

  RUN apk add --no-cache curl && \
      curl -sSL https://packages.skpr.io/php-alpine/skpr.rsa.pub -o /etc/apk/keys/skpr.rsa.pub && \
      echo "https://packages.skpr.io/php-alpine/3.20/php8.3" >> /etc/apk/repositories

  RUN apk add alpine-sdk clang clang-dev php8.3 php8.3-dev

  ENV MISE_DATA_DIR="/mise"
  ENV MISE_CONFIG_DIR="/mise"
  ENV MISE_CACHE_DIR="/mise/cache"
  ENV MISE_INSTALL_PATH="/usr/local/bin/mise"
  ENV PATH="/mise/shims:$PATH"

  RUN curl https://mise.run | sh

  ENV RUSTFLAGS="-C target-feature=-crt-static"
  ENV RUST_BACKTRACE=full

  WORKDIR /data

  ADD --chown=skpr:skpr . /data
  RUN mise trust .

  RUN mise run lint
  RUN mise run build

FROM ghcr.io/skpr/php-fpm:8.3-v2-stable

  USER root

  RUN apk add binutils bpftrace bash

  USER skpr

  COPY --from=docker.io/composer:2 --chown=skpr:skpr /usr/bin/composer /usr/local/bin/

  # Copy the Drupal application code in.
  COPY --from=docker.io/drupal:10 --chown=skpr:skpr /opt/drupal /data

  # Make this Drupal application compatible with our images.
  RUN ln -sf /data/web /data/app

  # Override settings.php
  ADD --chown=skpr:skpr docker/compose/php-fpm/settings.php /data/app/sites/default/settings.php
  ADD --chown=skpr:skpr docker/compose/php-fpm/services.yml /data/app/sites/default/services.yml

  # Add drush
  RUN composer require drush/drush
  COPY docker/compose/php-fpm/drush /etc/drush

  RUN PHP_MEMORY_LIMIT=-1 vendor/bin/drush si demo_umami --db-url=sqlite://sites/default/files/.ht.sqlite

  COPY 00_compass.ini /etc/php/conf.d/00_compass.ini
  COPY --from=build /data/target/release/libcompass_extension.so /usr/lib/php/modules/compass.so
