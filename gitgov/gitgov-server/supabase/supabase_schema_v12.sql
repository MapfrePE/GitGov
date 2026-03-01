-- GitGov Control Plane Schema v12 — Synthetic login exclusion in active dev metric
-- ============================================================================
-- MIGRATION: Run after supabase_schema_v11.sql
--
-- Problem:
--   Test/synthetic logins can inflate `active_devs_week` in founder/global views.
--
-- Change:
--   Keep audit data append-only, but exclude known synthetic login patterns from
--   `active_devs_week` inside get_audit_stats().

CREATE OR REPLACE FUNCTION get_audit_stats(p_org_id UUID DEFAULT NULL)
RETURNS JSON AS $$
DECLARE
    result JSON;
BEGIN
    SELECT json_build_object(
        'github_events', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'today', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= DATE_TRUNC('day', NOW())),
                'pushes_today', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND event_type = 'push' AND created_at >= DATE_TRUNC('day', NOW())),
                'by_type', COALESCE((SELECT json_object_agg(event_type, cnt) FROM (SELECT event_type, COUNT(*) as cnt FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY event_type) t), '{}'::json)
            )
        ),
        'client_events', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'today', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= DATE_TRUNC('day', NOW())),
                'blocked_today', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND status = 'blocked' AND created_at >= DATE_TRUNC('day', NOW())),
                'by_type', COALESCE((SELECT json_object_agg(event_type, cnt) FROM (SELECT event_type, COUNT(*) as cnt FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY event_type) t), '{}'::json),
                'by_status', COALESCE((SELECT json_object_agg(status, cnt) FROM (SELECT status, COUNT(*) as cnt FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY status) t), '{}'::json)
            )
        ),
        'violations', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'unresolved', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id) AND NOT resolved),
                'critical', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id) AND severity = 'critical' AND NOT resolved)
            )
        ),
        'active_devs_week', (
            SELECT COUNT(DISTINCT user_login)
            FROM client_events
            WHERE (p_org_id IS NULL OR org_id = p_org_id)
              AND created_at >= NOW() - INTERVAL '7 days'
              AND user_login !~* '^(alias_|erase_ok_|hb_user_|user_[0-9a-f]{6,}|test_?user|golden_?test|smoke|manual-check|victim_|dev_team_|e2e_)'
        ),
        'active_repos', (SELECT COUNT(DISTINCT repo_id) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= NOW() - INTERVAL '7 days')
    ) INTO result;

    RETURN result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

GRANT EXECUTE ON FUNCTION get_audit_stats(UUID) TO gitgov_server;
