#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "$DIR/server.key" -out "$DIR/server.crt" \
    -days 3650 -nodes -subj "/CN=lector-server"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "$DIR/client.key" -out "$DIR/client.crt" \
    -days 3650 -nodes -subj "/CN=lector-client"
echo "Certificates generated in $DIR"
