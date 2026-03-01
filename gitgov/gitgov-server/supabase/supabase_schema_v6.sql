-- GitGov Control Plane Schema v6 - Jira Ticket Coverage Groundwork (V1.2-B)
-- ========================================================================
-- MIGRATION: Run after supabase_schema_v5.sql
--
-- Adds Jira ticket entities and commit<->ticket correlation tables.
-- Written to be safe to re-run.

CREATE TABLE IF NOT EXISTS project_tickets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    ticket_id TEXT NOT NULL,
    ticket_url TEXT,
    title TEXT,
    status TEXT,
    assignee TEXT,
    reporter TEXT,
    priority TEXT,
    ticket_type TEXT,
    related_commits TEXT[] NOT NULL DEFAULT '{}',
    related_prs TEXT[] NOT NULL DEFAULT '{}',
    related_branches TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_project_tickets_org_ticket
    ON project_tickets(org_id, ticket_id);

CREATE INDEX IF NOT EXISTS idx_project_tickets_status
    ON project_tickets(org_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS commit_ticket_correlations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    commit_sha TEXT NOT NULL,
    ticket_id TEXT NOT NULL,
    correlation_source TEXT NOT NULL CHECK (
        correlation_source IN ('branch_name', 'commit_message', 'pr_title', 'manual')
    ),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_commit_ticket_unique
    ON commit_ticket_correlations(commit_sha, ticket_id);

CREATE INDEX IF NOT EXISTS idx_commit_ticket_by_ticket
    ON commit_ticket_correlations(org_id, ticket_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_commit_ticket_by_commit
    ON commit_ticket_correlations(org_id, commit_sha, created_at DESC);

CREATE OR REPLACE FUNCTION commit_ticket_correlations_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'commit_ticket_correlations is append-only';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger WHERE tgname = 'commit_ticket_correlations_immutable'
    ) THEN
        CREATE TRIGGER commit_ticket_correlations_immutable
            BEFORE UPDATE OR DELETE ON commit_ticket_correlations
            FOR EACH ROW EXECUTE FUNCTION commit_ticket_correlations_append_only();
    END IF;
END;
$$;

GRANT SELECT, INSERT, UPDATE ON project_tickets TO gitgov_server;
GRANT SELECT, INSERT ON commit_ticket_correlations TO gitgov_server;
GRANT EXECUTE ON FUNCTION commit_ticket_correlations_append_only() TO gitgov_server;

COMMENT ON TABLE project_tickets IS
    'Jira/PM ticket snapshots for V1.2-B ticket coverage.';

COMMENT ON TABLE commit_ticket_correlations IS
    'Append-only correlations between commits and work items (Jira/GitHub issues).';

