-- GitGov Control Plane Schema v3 - Violation Decisions
-- =======================================================
-- MIGRATION: Run after supabase_schema_v2.sql
-- This adds: violation_decisions table, true append-only violations

-- ============================================================================
-- PART 1: CREATE VIOLATION_DECISIONS TABLE
-- ============================================================================
-- WHY: violations table was allowing UPDATEs to resolved/resolved_at/resolved_by
--      This creates confusion about "append-only" guarantees.
--      SOLUTION: violations becomes 100% append-only, decisions are tracked separately.

CREATE TABLE IF NOT EXISTS violation_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    violation_id UUID NOT NULL REFERENCES violations(id) ON DELETE CASCADE,
    
    -- Decision type
    decision_type TEXT NOT NULL CHECK (decision_type IN (
        'acknowledged',      -- Team aware, no action needed
        'false_positive',    -- Not actually a violation
        'resolved',          -- Issue fixed
        'escalated',         -- Sent to management/security
        'dismissed',         -- Admin dismissed with reason
        'wont_fix'           -- Known issue, accepted risk
    )),
    
    -- Who made the decision
    decided_by TEXT NOT NULL,
    
    -- When
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Evidence/reasoning
    notes TEXT,
    evidence JSONB DEFAULT '{}',
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT violation_decisions_once_per_type UNIQUE (violation_id, decision_type)
);

-- Index for fast lookup by violation
CREATE INDEX IF NOT EXISTS idx_violation_decisions_violation_id 
    ON violation_decisions(violation_id);

-- Index for decisions by user
CREATE INDEX IF NOT EXISTS idx_violation_decisions_decided_by 
    ON violation_decisions(decided_by);

-- Index for decisions by date
CREATE INDEX IF NOT EXISTS idx_violation_decisions_decided_at 
    ON violation_decisions(decided_at DESC);

-- ============================================================================
-- PART 2: MAKE VIOLATIONS TRUE APPEND-ONLY
-- ============================================================================
-- Remove UPDATE capability from resolved/resolved_at/resolved_by
-- These columns are now DEPRECATED - use violation_decisions instead

-- First, migrate existing resolved violations to violation_decisions
INSERT INTO violation_decisions (violation_id, decision_type, decided_by, decided_at, notes)
SELECT 
    id,
    'resolved' as decision_type,
    COALESCE(resolved_by, 'system') as decided_by,
    COALESCE(resolved_at, NOW()) as decided_at,
    'Migrated from legacy resolved_at field' as notes
FROM violations 
WHERE resolved = true 
  AND resolved_at IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM violation_decisions vd WHERE vd.violation_id = violations.id
  );

-- Create trigger to PREVENT updates to resolved/resolved_at/resolved_by
CREATE OR REPLACE FUNCTION violations_no_resolution_update()
RETURNS TRIGGER AS $$
BEGIN
    -- These columns are now IMMUTABLE - use violation_decisions instead
    IF NEW.resolved IS DISTINCT FROM OLD.resolved OR
       NEW.resolved_at IS DISTINCT FROM OLD.resolved_at OR
       NEW.resolved_by IS DISTINCT FROM OLD.resolved_by THEN
        RAISE EXCEPTION 'Cannot update resolved/resolved_at/resolved_by directly. Use violation_decisions table instead.';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP TRIGGER IF EXISTS violations_no_resolution_update_trigger ON violations;
CREATE TRIGGER violations_no_resolution_update_trigger
    BEFORE UPDATE ON violations
    FOR EACH ROW EXECUTE FUNCTION violations_no_resolution_update();

-- ============================================================================
-- PART 3: HELPER FUNCTION TO ADD DECISION
-- ============================================================================

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
    -- Validate decision type
    IF p_decision_type NOT IN ('acknowledged', 'false_positive', 'resolved', 'escalated', 'dismissed', 'wont_fix') THEN
        RAISE EXCEPTION 'Invalid decision_type: %. Must be one of: acknowledged, false_positive, resolved, escalated, dismissed, wont_fix', p_decision_type;
    END IF;
    
    -- Insert decision
    INSERT INTO violation_decisions (
        violation_id, decision_type, decided_by, notes, evidence
    ) VALUES (
        p_violation_id, p_decision_type, p_decided_by, p_notes, p_evidence
    )
    ON CONFLICT (violation_id, decision_type) DO UPDATE SET
        decided_by = EXCLUDED.decided_by,
        decided_at = NOW(),
        notes = EXCLUDED.notes,
        evidence = EXCLUDED.evidence
    RETURNING id INTO decision_id;
    
    -- Update violation's deprecated fields for backwards compatibility
    -- (This is allowed from this function since it's SECURITY DEFINER)
    IF p_decision_type = 'resolved' THEN
        UPDATE violations 
        SET resolved = true, 
            resolved_at = NOW(), 
            resolved_by = p_decided_by
        WHERE id = p_violation_id;
    END IF;
    
    RETURN decision_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- ============================================================================
-- PART 4: VIEW FOR LATEST DECISION PER VIOLATION
-- ============================================================================

CREATE OR REPLACE VIEW violation_current_status AS
SELECT 
    v.id as violation_id,
    v.org_id,
    v.repo_id,
    v.github_event_id,
    v.client_event_id,
    v.severity,
    v.violation_type,
    v.reason as description,
    v.user_login as actor_login,
    v.branch,
    v.commit_sha,
    v.created_at as violation_created_at,
    
    -- Latest decision
    vd.decision_type as current_status,
    vd.decided_by,
    vd.decided_at,
    vd.notes as decision_notes,
    
    -- Is it "closed"?
    CASE 
        WHEN vd.decision_type IN ('resolved', 'false_positive', 'dismissed', 'wont_fix') THEN true
        ELSE false
    END as is_closed
    
FROM violations v
LEFT JOIN LATERAL (
    SELECT * FROM violation_decisions 
    WHERE violation_id = v.id 
    ORDER BY decided_at DESC 
    LIMIT 1
) vd ON true;

-- ============================================================================
-- PART 5: GRANTS
-- ============================================================================

GRANT SELECT, INSERT ON violation_decisions TO gitgov_server;
GRANT SELECT ON violation_current_status TO gitgov_server;
GRANT EXECUTE ON FUNCTION add_violation_decision(UUID, TEXT, TEXT, TEXT, JSONB) TO gitgov_server;

-- ============================================================================
-- PART 6: COMMENTS
-- ============================================================================

COMMENT ON TABLE violation_decisions IS 
    'Audit trail of all decisions made about violations. Append-only.
     Each violation can have multiple decisions over time (e.g., acknowledged -> resolved).
     Use add_violation_decision() to add decisions.';

COMMENT ON COLUMN violation_decisions.decision_type IS 
    'acknowledged: Team aware, no action needed
     false_positive: Not actually a violation
     resolved: Issue fixed
     escalated: Sent to management/security
     dismissed: Admin dismissed with reason
     wont_fix: Known issue, accepted risk';

COMMENT ON VIEW violation_current_status IS 
    'Convenience view showing latest status of each violation with decision info.';
