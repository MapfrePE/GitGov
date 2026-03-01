-- GitGov Control Plane Schema v10 — Org Invitations (Admin Onboarding)
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v9.sql
--
-- Adds:
--   1. org_invitations — invite lifecycle for admin onboarding.

CREATE TABLE IF NOT EXISTS org_invitations (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id         UUID NOT NULL REFERENCES orgs(id),
    invite_email   TEXT,
    invite_login   TEXT,
    role           TEXT NOT NULL DEFAULT 'Developer',
    token_hash     TEXT NOT NULL UNIQUE,
    status         TEXT NOT NULL DEFAULT 'pending',
    invited_by     TEXT NOT NULL,
    accepted_by    TEXT,
    accepted_at    TIMESTAMPTZ,
    revoked_by     TEXT,
    revoked_at     TIMESTAMPTZ,
    expires_at     TIMESTAMPTZ NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT org_invitations_target_check
        CHECK (invite_email IS NOT NULL OR invite_login IS NOT NULL),
    CONSTRAINT org_invitations_role_check
        CHECK (role IN ('Admin', 'Architect', 'Developer', 'PM')),
    CONSTRAINT org_invitations_status_check
        CHECK (status IN ('pending', 'accepted', 'revoked'))
);

CREATE INDEX IF NOT EXISTS idx_org_invitations_org_status_created
    ON org_invitations(org_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_org_invitations_org_expires
    ON org_invitations(org_id, expires_at DESC);

CREATE OR REPLACE FUNCTION org_invitations_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS org_invitations_set_updated_at_trigger ON org_invitations;
CREATE TRIGGER org_invitations_set_updated_at_trigger
    BEFORE UPDATE ON org_invitations
    FOR EACH ROW EXECUTE FUNCTION org_invitations_set_updated_at();

GRANT SELECT, INSERT, UPDATE ON org_invitations TO gitgov_server;

COMMENT ON TABLE org_invitations IS
    'Organization invitation lifecycle for admin onboarding.';
