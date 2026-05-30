#!/bin/sh
set -e

# The app does not run migrations itself, so seed the persistent volume from the
# migrated template baked into the image on first start only. This avoids
# re-running the non-idempotent ALTER TABLE migration on every boot.
DB_FILE="/data/database.sqlite"
if [ ! -f "$DB_FILE" ]; then
    echo "Seeding $DB_FILE from migrated template..."
    mkdir -p /data
    cp /app/template.sqlite "$DB_FILE"
fi

exec "$@"
