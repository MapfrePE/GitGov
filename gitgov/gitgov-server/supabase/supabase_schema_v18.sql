-- ===================================================================
-- GitGov schema migration v18
-- Date: 2026-03-15
-- Purpose:
--   Close prod/repo drift after EC2 single-node hardening.
--   1) Add composite indexes for /logs fast path.
--   2) Replace get_audit_stats with runtime-optimized implementation.
-- ===================================================================

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'client_events'
    ) THEN
        CREATE INDEX IF NOT EXISTS idx_client_events_user_login_created
            ON client_events(user_login, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_client_events_branch_created
            ON client_events(branch, created_at DESC);
    END IF;
END $$;

CREATE OR REPLACE FUNCTION get_audit_stats(p_org_id UUID DEFAULT NULL)
RETURNS JSON
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    result JSON;
    ce_total bigint;
    ce_today bigint;
    ce_blocked bigint;
    ce_by_type json;
    ce_by_status json;
    v_total bigint;
    v_unresolved bigint;
    v_critical bigint;
    active_devs bigint;
BEGIN
    SELECT
        COUNT(*),
        COUNT(*) FILTER (WHERE created_at >= DATE_TRUNC('day', NOW())),
        COUNT(*) FILTER (WHERE status = 'blocked' AND created_at >= DATE_TRUNC('day', NOW()))
    INTO ce_total, ce_today, ce_blocked
    FROM client_events
    WHERE (p_org_id IS NULL OR org_id = p_org_id);

    SELECT COALESCE(json_object_agg(event_type, cnt), '{}'::json)
    INTO ce_by_type
    FROM (
        SELECT event_type, COUNT(*) AS cnt
        FROM client_events
        WHERE (p_org_id IS NULL OR org_id = p_org_id)
        GROUP BY event_type
    ) t;

    SELECT COALESCE(json_object_agg(status, cnt), '{}'::json)
    INTO ce_by_status
    FROM (
        SELECT status, COUNT(*) AS cnt
        FROM client_events
        WHERE (p_org_id IS NULL OR org_id = p_org_id)
        GROUP BY status
    ) t;

    SELECT COUNT(DISTINCT user_login)
    INTO active_devs
    FROM client_events
    WHERE (p_org_id IS NULL OR org_id = p_org_id)
      AND created_at >= NOW() - INTERVAL '7 days';

    SELECT
        COUNT(*),
        COUNT(*) FILTER (WHERE NOT resolved),
        COUNT(*) FILTER (WHERE severity = 'critical' AND NOT resolved)
    INTO v_total, v_unresolved, v_critical
    FROM violations
    WHERE (p_org_id IS NULL OR org_id = p_org_id);

    SELECT json_build_object(
        'github_events', json_build_object(
            'total', 0,
            'today', 0,
            'pushes_today', 0,
            'by_type', '{}'::json
        ),
        'client_events', json_build_object(
            'total', ce_total,
            'today', ce_today,
            'blocked_today', ce_blocked,
            'by_type', ce_by_type,
            'by_status', ce_by_status
        ),
        'violations', json_build_object(
            'total', v_total,
            'unresolved', v_unresolved,
            'critical', v_critical
        ),
        'active_devs_week', active_devs,
        'active_repos', 0
    ) INTO result;

    RETURN result;
END;
$$;

