-- GitGov Control Plane Schema v5 - Pipeline Events (Jenkins Integration)
-- =====================================================================
-- MIGRATION: Run after supabase_schema_v4.sql
--
-- Adds append-only pipeline event storage for V1.2-A (Jenkins-first MVP).
-- This migration is written to be safe to re-run.

CREATE TABLE IF NOT EXISTS pipeline_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    pipeline_id TEXT NOT NULL,
    job_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failure', 'aborted', 'unstable')),
    commit_sha TEXT,
    branch TEXT,
    repo_full_name TEXT,
    duration_ms BIGINT,
    triggered_by TEXT,
    stages JSONB NOT NULL DEFAULT '[]'::jsonb,
    artifacts JSONB NOT NULL DEFAULT '[]'::jsonb,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION pipeline_events_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'pipeline_events is append-only';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger
        WHERE tgname = 'pipeline_events_immutable'
    ) THEN
        CREATE TRIGGER pipeline_events_immutable
            BEFORE UPDATE OR DELETE ON pipeline_events
            FOR EACH ROW EXECUTE FUNCTION pipeline_events_append_only();
    END IF;
END;
$$;

CREATE INDEX IF NOT EXISTS idx_pipeline_events_commit
    ON pipeline_events(commit_sha);

CREATE INDEX IF NOT EXISTS idx_pipeline_events_org
    ON pipeline_events(org_id, ingested_at DESC);

CREATE INDEX IF NOT EXISTS idx_pipeline_events_branch
    ON pipeline_events(org_id, branch, ingested_at DESC);

CREATE INDEX IF NOT EXISTS idx_pipeline_events_pipeline_id
    ON pipeline_events(pipeline_id, ingested_at DESC);

-- Prep for idempotency strategy in V1.2-A (A3): duplicates by pipeline/job/commit/timestamp.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pipeline_events_dedupe_v1
    ON pipeline_events(pipeline_id, job_name, COALESCE(commit_sha, ''), ingested_at);

GRANT SELECT, INSERT ON pipeline_events TO gitgov_server;
GRANT EXECUTE ON FUNCTION pipeline_events_append_only() TO gitgov_server;

COMMENT ON TABLE pipeline_events IS
    'Append-only CI/CD pipeline events for V1.2 Jenkins integration.';

COMMENT ON FUNCTION pipeline_events_append_only() IS
    'Prevents UPDATE/DELETE on pipeline_events (append-only audit log).';

