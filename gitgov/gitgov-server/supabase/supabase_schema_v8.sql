-- GitGov Control Plane Schema v8 — GDPR + Client Sessions + Identity Aliases
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v7.sql
--
-- Adds:
--   1. user_pseudonyms   — tracks GDPR erasure requests (art. 17 right to erasure)
--   2. client_sessions   — last_seen per device/user (T3.A heartbeat baseline)
--   3. identity_aliases  — links multiple logins to one canonical identity (T3.B)

-- ============================================================================
-- TABLE: user_pseudonyms
-- ============================================================================
-- Records which user_logins have been erased under GDPR art. 17.
-- The actual erasure replaces user_login in client_events / github_events
-- with '[ERASED]'. This table is the audit trail of erasure requests.
--
-- NOT append-only: erased_at can be set multiple times (re-erasure is idempotent).

CREATE TABLE IF NOT EXISTS user_pseudonyms (
    user_login    TEXT PRIMARY KEY,
    pseudonym_id  UUID NOT NULL DEFAULT gen_random_uuid(),
    erased_at     TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_pseudonyms_erased
    ON user_pseudonyms(erased_at)
    WHERE erased_at IS NOT NULL;

-- ============================================================================
-- TABLE: client_sessions
-- ============================================================================
-- Upserted on every inbound event (including heartbeat).
-- Allows GET /clients to return last_seen_at per developer device.

CREATE TABLE IF NOT EXISTS client_sessions (
    client_id        TEXT PRIMARY KEY,
    org_id           UUID REFERENCES orgs(id),
    last_seen_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    device_metadata  JSONB NOT NULL DEFAULT '{}',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_client_sessions_org
    ON client_sessions(org_id, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_client_sessions_last_seen
    ON client_sessions(last_seen_at DESC);

-- ============================================================================
-- TABLE: identity_aliases
-- ============================================================================
-- Maps alias_login → canonical_login so that multiple GitHub accounts
-- belonging to the same developer are aggregated in the dashboard.
-- One alias can only map to one canonical identity (UNIQUE on alias_login).

CREATE TABLE IF NOT EXISTS identity_aliases (
    canonical_login  TEXT NOT NULL,
    alias_login      TEXT NOT NULL,
    org_id           UUID REFERENCES orgs(id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (canonical_login, alias_login),
    UNIQUE (alias_login)
);

CREATE INDEX IF NOT EXISTS idx_identity_aliases_canonical
    ON identity_aliases(canonical_login);
