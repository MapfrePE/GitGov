-- GitGov Control Plane Schema v7 - PR Merges + Admin Audit Log
-- ==============================================================
-- MIGRATION: Run after supabase_schema_v6.sql
--
-- Adds:
--   1. pull_request_merges  — append-only table for merged PR events from GitHub webhooks
--   2. admin_audit_log      — append-only log of administrative actions (revoke key, confirm signal, export)
--
-- Written to be safe to re-run.

-- ============================================================================
-- TABLE: pull_request_merges
-- ============================================================================
-- Stores one row per merged pull_request webhook event.
-- Source: GitHub webhook X-GitHub-Event: pull_request, action=closed, merged=true.
-- Append-only — triggers below prevent UPDATE/DELETE.

CREATE TABLE IF NOT EXISTS pull_request_merges (
    id              UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID    REFERENCES orgs(id),
    repo_id         UUID    REFERENCES repos(id),
    delivery_id     TEXT    NOT NULL UNIQUE,
    pr_number       INT     NOT NULL,
    pr_title        TEXT,
    author_login    TEXT,
    merged_by_login TEXT,
    head_sha        TEXT,
    base_branch     TEXT,
    payload         JSONB   NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pull_request_merges_delivery
    ON pull_request_merges(delivery_id);

CREATE INDEX IF NOT EXISTS idx_pull_request_merges_repo
    ON pull_request_merges(repo_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pull_request_merges_org
    ON pull_request_merges(org_id, created_at DESC);

CREATE OR REPLACE FUNCTION pull_request_merges_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'pull_request_merges is append-only';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS pull_request_merges_append_only_trigger ON pull_request_merges;
CREATE TRIGGER pull_request_merges_append_only_trigger
    BEFORE UPDATE OR DELETE ON pull_request_merges
    FOR EACH ROW EXECUTE FUNCTION pull_request_merges_append_only();

-- ============================================================================
-- TABLE: admin_audit_log
-- ============================================================================
-- Append-only log of administrative actions performed via the GitGov API.
-- Records who (actor_client_id), what (action), on what (target_type/target_id),
-- and optional machine-readable metadata.
--
-- Actions logged:
--   revoke_api_key    — POST /api-keys/:id/revoke
--   confirm_signal    — POST /signals/:id/confirm
--   export_events     — POST /export

CREATE TABLE IF NOT EXISTS admin_audit_log (
    id               UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_client_id  TEXT    NOT NULL,
    action           TEXT    NOT NULL,
    target_type      TEXT,
    target_id        TEXT,
    metadata         JSONB   NOT NULL DEFAULT '{}',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_log_actor
    ON admin_audit_log(actor_client_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_admin_audit_log_action
    ON admin_audit_log(action, created_at DESC);

CREATE OR REPLACE FUNCTION admin_audit_log_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'admin_audit_log is append-only';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS admin_audit_log_append_only_trigger ON admin_audit_log;
CREATE TRIGGER admin_audit_log_append_only_trigger
    BEFORE UPDATE OR DELETE ON admin_audit_log
    FOR EACH ROW EXECUTE FUNCTION admin_audit_log_append_only();
