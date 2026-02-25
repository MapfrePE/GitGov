#!/bin/sh
set -eu

echo "[gitgov-db-init] Applying supabase_schema.sql (local Docker mode, skipping Supabase auth policies)..."

awk '
  /-- Policies for authenticated users \(using Supabase auth\)/ { skip=1; next }
  /-- UTILITY FUNCTIONS/ { skip=0 }
  skip != 1 { print }
' /workspace/supabase_schema.sql | psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB"

