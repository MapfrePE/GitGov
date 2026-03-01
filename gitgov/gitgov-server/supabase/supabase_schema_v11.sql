-- GitGov Control Plane Schema v11 — Conversational Chat (Feature Requests)
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v10.sql
--
-- Adds:
--   1. feature_requests — tracks capabilities requested by users via chat

CREATE TABLE IF NOT EXISTS feature_requests (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id           UUID REFERENCES orgs(id),
    requested_by     TEXT NOT NULL,
    question         TEXT NOT NULL,
    missing_capability TEXT,
    status           TEXT NOT NULL DEFAULT 'new',
    metadata         JSONB NOT NULL DEFAULT '{}',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT feature_requests_status_check
        CHECK (status IN ('new', 'reviewing', 'planned', 'rejected'))
);

CREATE INDEX IF NOT EXISTS idx_feature_requests_org_created
    ON feature_requests(org_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_feature_requests_requested_by
    ON feature_requests(requested_by, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_feature_requests_status
    ON feature_requests(status, created_at DESC);
