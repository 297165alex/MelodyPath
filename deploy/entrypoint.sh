#!/bin/sh
set -eu
umask 077
# The platform's mounted disk may initially be root-owned. Only prepare this
# application's directory, then permanently drop root for the Rust process.
install -d -m 700 -o melody -g melody "${MELODYPATH_DATA_DIR:?}"
exec gosu melody /app/melody-path-api
