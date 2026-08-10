# The USDT probe contract for the `compass` provider.
#
# This is the single source of truth. scripts/validate-probes.sh checks that the
# built .so, bpftrace/compass.bt and README.md all agree with it.
#
# Format: <probe name> <argument count>
#
# Argument *registers* are deliberately not part of the contract. The previous
# spec (noteutils.yml, deleted in 95483ca) pinned them exactly - "-8@%rcx" and a
# parallel arm64 block - so it broke on any codegen change and went stale. The
# name and arity are what a consumer actually depends on.

# System. Every other probe is gated on canary, so consumers must attach to it.
canary                                          0

# FPM
fpm_request_init                                3
fpm_request_shutdown                            1
fpm_function                                    4

# CLI
cli_request_init                                2
cli_request_shutdown                            1
cli_function                                    4

# Database
db_query_text                                   2
fpm_db_query                                    3
cli_db_query                                    3

# Drupal
drupal_cacheablemetadata_createfromobject       6
drupal_cacheablemetadata_createfromrenderarray  5
