-- GitGov Control Plane Schema v2 - Production Hardening
-- =======================================================
-- MIGRATION: Run after supabase_schema.sql
-- This adds: ingested_at for cursor safety, job queue hardening, metrics

-- ============================================================================
-- PART 1: INGESTED_AT FOR SAFE CURSOR PROCESSING
-- ============================================================================
-- WHY: created_at reflects event time, but events can arrive late.
--      If we use created_at as cursor, late-arriving events get skipped.
--      ingested_at is set at INSERT time, guaranteeing order.

-- Add ingested_at to github_events (if not exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'github_events' AND column_name = 'ingested_at'
    ) THEN
        ALTER TABLE github_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();
        COMMENT ON COLUMN github_events.ingested_at IS 
            'Server-side ingestion time. Used for cursor-based processing. NEVER modified.';
    END IF;
END $$;

-- Add ingested_at to client_events (if not exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'client_events' AND column_name = 'ingested_at'
    ) THEN
        ALTER TABLE client_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();
        COMMENT ON COLUMN client_events.ingested_at IS 
            'Server-side ingestion time. Used for cursor-based processing. NEVER modified.';
    END IF;
END $$;

-- Backfill existing rows (set to created_at for historical data)
UPDATE github_events SET ingested_at = created_at WHERE ingested_at IS NULL;
UPDATE client_events SET ingested_at = created_at WHERE ingested_at IS NULL;

-- Index for INGESTED_AT cursor (replaces created_at cursor)
DROP INDEX IF EXISTS idx_github_events_org_type_ingested;
CREATE INDEX idx_github_events_org_type_ingested 
    ON github_events(org_id, event_type, ingested_at, id);

-- Index for client_events correlation with ingested_at
DROP INDEX IF EXISTS idx_client_events_org_user_commit_ingested;
CREATE INDEX idx_client_events_org_user_commit_ingested 
    ON client_events(org_id, user_login, commit_sha, ingested_at);

-- ============================================================================
-- PART 2: UPDATE ORG_PROCESSING_STATE FOR INGESTED_AT CURSOR
-- ============================================================================

-- Add last_ingested_at column if not exists
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'org_processing_state' AND column_name = 'last_ingested_at'
    ) THEN
        ALTER TABLE org_processing_state ADD COLUMN last_ingested_at TIMESTAMPTZ;
    END IF;
END $$;

-- ============================================================================
-- PART 3: JOB QUEUE HARDENING
-- ============================================================================

-- Update jobs table for production readiness
-- Add 'dead' status for jobs that exceeded max_attempts
-- Increase default max_attempts to 10 for exponential backoff

ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_status_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_status_check 
    CHECK (status IN ('pending', 'running', 'completed', 'failed', 'dead'));

-- Update default max_attempts to 10 for better retry behavior
ALTER TABLE jobs ALTER COLUMN max_attempts SET DEFAULT 10;

-- Add job metrics columns
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'jobs' AND column_name = 'started_at'
    ) THEN
        ALTER TABLE jobs ADD COLUMN started_at TIMESTAMPTZ;
    END IF;
    
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'jobs' AND column_name = 'duration_ms'
    ) THEN
        ALTER TABLE jobs ADD COLUMN duration_ms BIGINT;
    END IF;
END $$;

-- ============================================================================
-- PART 4: REPLACE DETECT_NONCOMPLIANCE_SIGNALS WITH INGESTED_AT CURSOR
-- ============================================================================

CREATE OR REPLACE FUNCTION detect_noncompliance_signals(
    p_org_id UUID,
    p_window_minutes INTEGER DEFAULT 15,
    p_tolerance_minutes INTEGER DEFAULT 30
) RETURNS INTEGER AS $$
DECLARE
    signal_count INTEGER := 0;
    rec RECORD;
    config RECORD;
    cursor_id UUID := NULL;
    cursor_ingested_at TIMESTAMPTZ := NULL;
    processed_count INTEGER := 0;
    last_event_id UUID;
    last_ingested_at TIMESTAMPTZ;
BEGIN
    -- Get org config
    SELECT * INTO config FROM correlation_config WHERE org_id = p_org_id;
    IF NOT FOUND THEN
        config := ROW(p_org_id, p_window_minutes, p_tolerance_minutes, 60, FALSE, NOW(), NOW());
    END IF;

    -- Get cursor from processing state (now using ingested_at)
    SELECT last_processed_event_id, last_ingested_at 
    INTO cursor_id, cursor_ingested_at
    FROM org_processing_state 
    WHERE org_id = p_org_id;
    
    -- If no cursor, start from 24 hours ago (not infinite lookback)
    IF cursor_ingested_at IS NULL THEN
        cursor_ingested_at := NOW() - INTERVAL '24 hours';
        cursor_id := NULL;
    END IF;

    -- Find github push events AFTER cursor (incremental processing)
    -- CRITICAL: Uses ingested_at (server-side time) not created_at (event time)
    -- This ensures late-arriving events are NOT skipped
    FOR rec IN 
        SELECT ge.id, ge.actor_login, ge.repo_id, ge.ref_name, ge.after_sha, 
               ge.created_at as event_time, ge.ingested_at
        FROM github_events ge
        WHERE ge.org_id = p_org_id
          AND ge.event_type = 'push'
          AND (cursor_id IS NULL OR (ge.ingested_at, ge.id) > (cursor_ingested_at, cursor_id))
          AND NOT EXISTS (
              SELECT 1 FROM client_events ce
              WHERE ce.org_id = p_org_id
                AND ce.user_login = ge.actor_login
                AND ce.commit_sha = ge.after_sha
                AND ce.ingested_at BETWEEN 
                    ge.ingested_at - (config.clock_skew_seconds || ' seconds')::INTERVAL AND
                    ge.ingested_at + (config.clock_skew_seconds || ' seconds')::INTERVAL
          )
          AND NOT EXISTS (
              SELECT 1 FROM noncompliance_signals ns
              WHERE ns.github_event_id = ge.id
          )
        ORDER BY ge.ingested_at ASC, ge.id ASC
        LIMIT 100
    LOOP
        -- Track last processed event for cursor update
        last_event_id := rec.id;
        last_ingested_at := rec.ingested_at;
        processed_count := processed_count + 1;
        
        -- Check if there are pending client events (outbox not flushed)
        DECLARE
            pending_events INTEGER;
        BEGIN
            SELECT COUNT(*) INTO pending_events
            FROM client_events ce
            WHERE ce.org_id = p_org_id
              AND ce.user_login = rec.actor_login
              AND ce.ingested_at >= NOW() - (config.bypass_tolerance_minutes || ' minutes')::INTERVAL;

            -- Determine confidence based on pending events and timing
            IF pending_events > 0 THEN
                INSERT INTO noncompliance_signals (
                    org_id, repo_id, github_event_id, signal_type, confidence,
                    actor_login, branch, commit_sha, evidence
                ) VALUES (
                    p_org_id, rec.repo_id, rec.id, 'missing_telemetry', 'low',
                    rec.actor_login, rec.ref_name, rec.after_sha,
                    jsonb_build_object(
                        'github_event_time', rec.event_time,
                        'ingested_at', rec.ingested_at,
                        'pending_client_events', pending_events,
                        'note', 'Client events exist but no matching push correlation'
                    )
                );
            ELSE
                INSERT INTO noncompliance_signals (
                    org_id, repo_id, github_event_id, signal_type, confidence,
                    actor_login, branch, commit_sha, evidence
                ) VALUES (
                    p_org_id, rec.repo_id, rec.id, 'untrusted_path', 'high',
                    rec.actor_login, rec.ref_name, rec.after_sha,
                    jsonb_build_object(
                        'github_event_time', rec.event_time,
                        'ingested_at', rec.ingested_at,
                        'pending_client_events', 0,
                        'note', 'Push detected without GitGov client event'
                    )
                );
            END IF;
            
            signal_count := signal_count + 1;
        END;
    END LOOP;
    
    -- Update cursor in processing state
    IF last_event_id IS NOT NULL THEN
        INSERT INTO org_processing_state (
            org_id, last_processed_event_id, last_processed_created_at, last_ingested_at,
            last_run_at, events_processed, signals_created
        )
        VALUES (
            p_org_id, last_event_id, last_ingested_at, last_ingested_at,
            NOW(), processed_count, signal_count
        )
        ON CONFLICT (org_id) DO UPDATE SET
            last_processed_event_id = EXCLUDED.last_processed_event_id,
            last_processed_created_at = EXCLUDED.last_processed_created_at,
            last_ingested_at = EXCLUDED.last_ingested_at,
            last_run_at = NOW(),
            events_processed = org_processing_state.events_processed + processed_count,
            signals_created = org_processing_state.signals_created + signal_count,
            updated_at = NOW();
    END IF;
    
    RETURN signal_count;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

COMMENT ON FUNCTION detect_noncompliance_signals IS 
'Incremental signal detection using ingested_at cursor. 
Processes events in order of server ingestion, not event creation time.
This prevents late-arriving events from being skipped.';

-- ============================================================================
-- PART 5: JOB QUEUE HELPER FUNCTIONS
-- ============================================================================

-- Calculate exponential backoff: 2^attempts * base_seconds, capped at max_seconds
-- IMPORTANT: p_attempts should be the NEW attempt count (after increment)
CREATE OR REPLACE FUNCTION job_backoff_seconds(p_attempts INTEGER)
RETURNS INTEGER AS $$
DECLARE
    base_seconds INTEGER := 30;  -- Start at 30 seconds
    max_seconds INTEGER := 3600; -- Cap at 1 hour
    backoff INTEGER;
BEGIN
    backoff := (2 ^ p_attempts) * base_seconds;
    RETURN LEAST(backoff, max_seconds);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Get job queue metrics for observability
CREATE OR REPLACE FUNCTION get_job_metrics()
RETURNS JSON AS $$
DECLARE
    result JSON;
BEGIN
    SELECT json_build_object(
        'pending', (SELECT COUNT(*) FROM jobs WHERE status = 'pending'),
        'running', (SELECT COUNT(*) FROM jobs WHERE status = 'running'),
        'completed_today', (SELECT COUNT(*) FROM jobs WHERE status = 'completed' AND completed_at >= DATE_TRUNC('day', NOW())),
        'failed_today', (SELECT COUNT(*) FROM jobs WHERE status = 'failed' AND completed_at >= DATE_TRUNC('day', NOW())),
        'dead', (SELECT COUNT(*) FROM jobs WHERE status = 'dead'),
        'stale_running', (SELECT COUNT(*) FROM jobs WHERE status = 'running' AND locked_at < NOW() - INTERVAL '5 minutes'),
        'avg_duration_ms', (SELECT AVG(duration_ms)::BIGINT FROM jobs WHERE status = 'completed' AND duration_ms IS NOT NULL),
        'oldest_pending_seconds', (
            SELECT EXTRACT(EPOCH FROM (NOW() - created_at))::BIGINT 
            FROM jobs WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1
        )
    ) INTO result;
    
    RETURN result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Safely reset stale jobs with proper backoff and dead-letter handling
-- FIX: Use attempts_new for backoff calculation, check max_attempts for dead-letter
CREATE OR REPLACE FUNCTION reset_stale_jobs_safe(p_ttl_minutes INTEGER DEFAULT 5)
RETURNS INTEGER AS $$
DECLARE
    reset_count INTEGER;
BEGIN
    -- Reset stale jobs with proper backoff calculation
    -- CRITICAL: 
    -- 1. Calculate attempts_new BEFORE using it for backoff
    -- 2. If attempts_new >= max_attempts, mark as dead (not pending)
    -- 3. Only set next_run_at for jobs that will retry (not dead)
    WITH stale_jobs AS (
        SELECT id, attempts, max_attempts FROM jobs 
        WHERE status = 'running' 
          AND locked_at < NOW() - (p_ttl_minutes || ' minutes')::INTERVAL
        FOR UPDATE SKIP LOCKED
    )
    UPDATE jobs 
    SET status = CASE 
        WHEN (attempts + 1) >= max_attempts THEN 'dead'
        ELSE 'pending'
    END,
    locked_at = NULL,
    locked_by = NULL,
    started_at = NULL,
    attempts = attempts + 1,
    last_error = CASE 
        WHEN (attempts + 1) >= max_attempts THEN 'Job exceeded max_attempts after timeout'
        ELSE 'Job timed out after ' || p_ttl_minutes || ' minutes'
    END,
    next_run_at = CASE 
        WHEN (attempts + 1) < max_attempts THEN NOW() + (job_backoff_seconds(attempts + 1) || ' seconds')::INTERVAL
        ELSE NULL
    END,
    completed_at = CASE 
        WHEN (attempts + 1) >= max_attempts THEN NOW()
        ELSE completed_at
    END
    WHERE id IN (SELECT id FROM stale_jobs);
    
    GET DIAGNOSTICS reset_count = ROW_COUNT;
    
    IF reset_count > 0 THEN
        RAISE NOTICE 'Reset % stale jobs (TTL=% minutes)', reset_count, p_ttl_minutes;
    END IF;
    
    RETURN reset_count;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Mark job as dead (dead-letter queue)
CREATE OR REPLACE FUNCTION mark_job_dead(p_job_id UUID, p_error TEXT DEFAULT NULL)
RETURNS VOID AS $$
BEGIN
    UPDATE jobs 
    SET status = 'dead',
        last_error = COALESCE(p_error, 'Exceeded max attempts'),
        completed_at = NOW()
    WHERE id = p_job_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- ============================================================================
-- PART 6: GRANTS FOR NEW FUNCTIONS
-- ============================================================================

GRANT EXECUTE ON FUNCTION job_backoff_seconds(INTEGER) TO gitgov_server;
GRANT EXECUTE ON FUNCTION get_job_metrics() TO gitgov_server;
GRANT EXECUTE ON FUNCTION reset_stale_jobs_safe(INTEGER) TO gitgov_server;
GRANT EXECUTE ON FUNCTION mark_job_dead(UUID, TEXT) TO gitgov_server;

-- ============================================================================
-- PART 7: VERIFY APPEND-ONLY TRIGGERS
-- ============================================================================

-- Ensure all audit tables have append-only triggers
-- These should already exist from supabase_schema.sql, but verify:

DO $$
DECLARE
    tables TEXT[] := ARRAY['github_events', 'client_events', 'violations', 
                           'noncompliance_signals', 'governance_events', 
                           'signal_decisions', 'policy_history'];
    t TEXT;
    trigger_exists BOOLEAN;
BEGIN
    FOREACH t IN ARRAY tables LOOP
        SELECT EXISTS(
            SELECT 1 FROM pg_trigger 
            WHERE tgname = t || '_append_only'
               OR tgname = t || '_no_delete'
        ) INTO trigger_exists;
        
        IF NOT trigger_exists THEN
            RAISE WARNING 'Append-only trigger missing for table: %', t;
        END IF;
    END LOOP;
END $$;

-- ============================================================================
-- PART 8: JOBS TABLE CONSTRAINTS (Allow state updates only)
-- ============================================================================

-- Jobs table is NOT append-only - it needs state transitions
-- But we restrict which columns can be updated

CREATE OR REPLACE FUNCTION jobs_allowed_update()
RETURNS TRIGGER AS $$
BEGIN
    -- id, org_id, job_type, created_at, payload are IMMUTABLE
    IF NEW.id != OLD.id OR 
       (NEW.org_id IS DISTINCT FROM OLD.org_id) OR
       NEW.job_type != OLD.job_type OR
       NEW.created_at != OLD.created_at OR
       (NEW.payload IS DISTINCT FROM OLD.payload) THEN
        RAISE EXCEPTION 'Cannot modify immutable job columns (id, org_id, job_type, created_at, payload)';
    END IF;
    
    -- Only these columns can change: status, priority, locked_at, locked_by,
    -- attempts, max_attempts, last_error, next_run_at, started_at, completed_at, duration_ms
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP TRIGGER IF EXISTS jobs_update_restriction ON jobs;
CREATE TRIGGER jobs_update_restriction
    BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION jobs_allowed_update();

-- Allow DELETE only for completed/failed/dead jobs (cleanup)
CREATE OR REPLACE FUNCTION jobs_delete_restriction()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status NOT IN ('completed', 'failed', 'dead') THEN
        RAISE EXCEPTION 'Cannot delete pending or running jobs';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP TRIGGER IF EXISTS jobs_delete_restriction_trigger ON jobs;
CREATE TRIGGER jobs_delete_restriction_trigger
    BEFORE DELETE ON jobs
    FOR EACH ROW EXECUTE FUNCTION jobs_delete_restriction();

-- ============================================================================
-- PART 9: PARTITIONING PREPARATION (Optional, for high-volume orgs)
-- ============================================================================

-- Note: For very high volume, consider partitioning github_events by org_id
-- This is commented out as it requires careful planning

-- CREATE TABLE github_events_partitioned (LIKE github_events INCLUDING ALL)
-- PARTITION BY LIST (org_id);

-- ============================================================================
-- PART 10: INDEXES FOR PERFORMANCE
-- ============================================================================

-- Index for job claiming (most important for queue performance)
DROP INDEX IF EXISTS idx_jobs_claim;
CREATE INDEX idx_jobs_claim ON jobs(status, next_run_at, priority, created_at)
    WHERE status = 'pending';

-- Index for stale job detection
DROP INDEX IF EXISTS idx_jobs_stale;
CREATE INDEX idx_jobs_stale ON jobs(locked_at)
    WHERE status = 'running';

-- Partial index for unique pending/running jobs (already exists, ensuring)
DROP INDEX IF EXISTS idx_jobs_unique_pending;
CREATE UNIQUE INDEX idx_jobs_unique_pending 
    ON jobs(org_id, job_type) 
    WHERE status IN ('pending', 'running');

-- ============================================================================
-- COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON COLUMN github_events.ingested_at IS 
    'Server-side ingestion timestamp. Used for cursor-based incremental processing. 
     Unlike created_at (event time), this is set at INSERT and never modified.
     This prevents late-arriving events from being skipped during processing.';

COMMENT ON COLUMN jobs.status IS 
    'Job states: pending -> running -> completed | failed | dead.
     dead = exceeded max_attempts, in dead-letter queue.
     failed = marked failed but may have retries left.';

COMMENT ON COLUMN jobs.attempts IS 
    'Number of processing attempts. Incremented on each claim.';

COMMENT ON COLUMN jobs.next_run_at IS 
    'When the job should next be processed. Set with exponential backoff on failure.';

COMMENT ON COLUMN jobs.max_attempts IS 
    'Maximum attempts before marking as dead. Default 10 with exponential backoff.';

COMMENT ON FUNCTION job_backoff_seconds IS 
    'Exponential backoff calculator: 30s * 2^attempts, capped at 1 hour. 
     IMPORTANT: Pass the NEW attempt count (after increment).';

COMMENT ON FUNCTION reset_stale_jobs_safe IS 
    'Reset stale jobs with proper backoff calculation using NEW attempt count.
     Jobs exceeding max_attempts are marked as dead (not pending).
     Uses FOR UPDATE SKIP LOCKED for safety.';
