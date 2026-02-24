-- GitGov Control Plane Schema v4 - Hotfix for append-only violations
-- ================================================================
-- MIGRATION: Run after supabase_schema_v3.sql
--
-- Problem fixed:
-- - v3 add_violation_decision() updates violations.resolved* fields
-- - v3 also adds a trigger that blocks those updates
-- - Result: function can fail on decision_type='resolved'
--
-- This hotfix keeps violations append-only and records decisions only in
-- violation_decisions (source of truth for status transitions).

CREATE OR REPLACE FUNCTION add_violation_decision(
    p_violation_id UUID,
    p_decision_type TEXT,
    p_decided_by TEXT,
    p_notes TEXT DEFAULT NULL,
    p_evidence JSONB DEFAULT '{}'
) RETURNS UUID AS $$
DECLARE
    decision_id UUID;
BEGIN
    IF p_decision_type NOT IN ('acknowledged', 'false_positive', 'resolved', 'escalated', 'dismissed', 'wont_fix') THEN
        RAISE EXCEPTION 'Invalid decision_type: %. Must be one of: acknowledged, false_positive, resolved, escalated, dismissed, wont_fix', p_decision_type;
    END IF;

    INSERT INTO violation_decisions (
        violation_id, decision_type, decided_by, notes, evidence
    ) VALUES (
        p_violation_id, p_decision_type, p_decided_by, p_notes, COALESCE(p_evidence, '{}'::jsonb)
    )
    ON CONFLICT (violation_id, decision_type) DO UPDATE SET
        decided_by = EXCLUDED.decided_by,
        decided_at = NOW(),
        notes = EXCLUDED.notes,
        evidence = EXCLUDED.evidence
    RETURNING id INTO decision_id;

    -- Do NOT update violations.resolved/resolved_at/resolved_by.
    -- Those fields are deprecated and immutable under append-only policy.
    RETURN decision_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

GRANT EXECUTE ON FUNCTION add_violation_decision(UUID, TEXT, TEXT, TEXT, JSONB) TO gitgov_server;

COMMENT ON FUNCTION add_violation_decision(UUID, TEXT, TEXT, TEXT, JSONB) IS
    'v4 hotfix: stores violation decisions without updating deprecated violations.resolved* fields.';
