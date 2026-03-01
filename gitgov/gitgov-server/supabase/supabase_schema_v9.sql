-- GitGov Control Plane Schema v9 — Org Users (Admin Provisioning)
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v8.sql
--
-- Adds:
--   1. org_users — admin-managed users per organization (identity + role + status)

-- ============================================================================
-- TABLE: org_users
-- ============================================================================
-- Purpose:
-- - Allow org admins to pre-provision users in GitGov before issuing API keys.
-- - Track role/status lifecycle at org scope.
--
-- Notes:
-- - NOT append-only: status and profile fields are expected to change.
-- - API keys remain in api_keys; this table is identity/admin metadata.

CREATE TABLE IF NOT EXISTS org_users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id       UUID NOT NULL REFERENCES orgs(id),
    login        TEXT NOT NULL,
    display_name TEXT,
    email        TEXT,
    role         TEXT NOT NULL DEFAULT 'Developer',
    status       TEXT NOT NULL DEFAULT 'active',
    created_by   TEXT,
    updated_by   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT org_users_role_check
        CHECK (role IN ('Admin', 'Architect', 'Developer', 'PM')),
    CONSTRAINT org_users_status_check
        CHECK (status IN ('active', 'disabled')),
    CONSTRAINT org_users_org_login_unique
        UNIQUE (org_id, login)
);

CREATE INDEX IF NOT EXISTS idx_org_users_org_status
    ON org_users(org_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_org_users_org_login
    ON org_users(org_id, login);

CREATE OR REPLACE FUNCTION org_users_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS org_users_set_updated_at_trigger ON org_users;
CREATE TRIGGER org_users_set_updated_at_trigger
    BEFORE UPDATE ON org_users
    FOR EACH ROW EXECUTE FUNCTION org_users_set_updated_at();

GRANT SELECT, INSERT, UPDATE ON org_users TO gitgov_server;

COMMENT ON TABLE org_users IS
    'Admin-provisioned users per org (identity/role/status) used to issue scoped API keys.';
