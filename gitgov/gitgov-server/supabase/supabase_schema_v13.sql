-- GitGov Control Plane Schema v13 — CLI Command Audit Trail
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v12.sql
--
-- Adds:
--   1. cli_commands — audit trail for embedded terminal commands

CREATE TABLE IF NOT EXISTS cli_commands (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id           UUID REFERENCES orgs(id),
    user_login       TEXT NOT NULL,
    command          TEXT NOT NULL,
    origin           TEXT NOT NULL DEFAULT 'manual_input',
    branch           TEXT NOT NULL DEFAULT '',
    repo_name        TEXT,
    exit_code        INTEGER,
    duration_ms      BIGINT,
    metadata         JSONB NOT NULL DEFAULT '{}',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT cli_commands_origin_check
        CHECK (origin IN ('button_click', 'manual_input'))
);

CREATE INDEX IF NOT EXISTS idx_cli_commands_org_created
    ON cli_commands(org_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_cli_commands_user_created
    ON cli_commands(user_login, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_cli_commands_repo
    ON cli_commands(repo_name, created_at DESC);
