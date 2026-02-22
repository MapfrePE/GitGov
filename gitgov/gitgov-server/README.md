# GitGov Control Plane Server

Centralized audit and policy server for GitGov desktop clients. Uses Supabase (PostgreSQL) as the database.

## Features

### Core
- **GitHub Webhooks**: Receives push/create events as source of truth
- **Client Events**: Batch telemetry from desktop clients with idempotency
- **Append-Only Audit**: Events cannot be modified or deleted
- **Row Level Security**: Supabase RLS policies for multi-tenant access

### Compliance (V1.0)
- **Correlation Engine**: Correlates client events with GitHub events by commit_sha
- **Bypass Detection**: Detects pushes without GitGov client events
- **Confidence Scoring**: NOT binary - uses `high`, `medium`, `low` confidence levels
- **Noncompliance Signals**: Evidence-based signals, not accusations
- **Policy Versioning**: Automatic history of all policy changes
- **Export with Hash**: PDF/Excel/JSON exports with SHA256 content hash

## Quick Start

### 1. Setup Supabase

1. Create a project at [supabase.com](https://supabase.com)
2. Go to **SQL Editor** and run the contents of `supabase_schema.sql`
3. Run the contents of `supabase_schema_v2.sql` for production hardening
4. Get your database URL from **Project Settings > Database > Connection string (URI)**

#### 1.1 Create a Limited Database User (CRITICAL for Security)

**IMPORTANT:** The default `postgres` user has `service_role` privileges which **bypasses all Row Level Security (RLS)**. You MUST create a limited user for the control plane server.

**Step-by-step in Supabase Dashboard:**

1. Go to **Database → Roles**
2. Click **Create a new role**
3. Name it `gitgov_server`
4. Set a strong password
5. **Do NOT** check `superuser`, `createrole`, or `createdb`
6. Save

**Then run this SQL in SQL Editor to grant minimal permissions:**

```sql
-- Grant connect privilege
GRANT CONNECT ON DATABASE postgres TO gitgov_server;

-- Grant usage on schema
GRANT USAGE ON SCHEMA public TO gitgov_server;

-- GRANT PERMISSIONS BY TABLE (NOT ALL TABLES)

-- Tables that need INSERT and SELECT (audit data)
GRANT SELECT, INSERT ON github_events TO gitgov_server;
GRANT SELECT, INSERT ON client_events TO gitgov_server;
GRANT SELECT, INSERT ON noncompliance_signals TO gitgov_server;
GRANT SELECT, INSERT ON violations TO gitgov_server;
GRANT SELECT, INSERT ON governance_events TO gitgov_server;
GRANT SELECT, INSERT ON export_logs TO gitgov_server;
GRANT SELECT, INSERT ON webhook_events TO gitgov_server;

-- Tables that need SELECT, INSERT, UPDATE (config/metadata)
GRANT SELECT, INSERT, UPDATE ON orgs TO gitgov_server;
GRANT SELECT, INSERT, UPDATE ON repos TO gitgov_server;
GRANT SELECT, INSERT, UPDATE ON members TO gitgov_server;
GRANT SELECT, INSERT, UPDATE ON policies TO gitgov_server;
GRANT SELECT, INSERT, UPDATE ON correlation_config TO gitgov_server;

-- API keys table - needs UPDATE for last_used_at
GRANT SELECT, INSERT, UPDATE ON api_keys TO gitgov_server;

-- Policy history and signal decisions - INSERT only (append-only)
GRANT SELECT, INSERT ON policy_history TO gitgov_server;
GRANT SELECT, INSERT ON signal_decisions TO gitgov_server;

-- Job queue - needs UPDATE for state transitions
GRANT SELECT, INSERT, UPDATE ON jobs TO gitgov_server;

-- Org processing state
GRANT SELECT, INSERT, UPDATE ON org_processing_state TO gitgov_server;

-- Grant usage on sequences (for UUID generation)
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO gitgov_server;

-- Grant execute on functions
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO gitgov_server;
```

**Update your `.env`:**

```env
# Use the limited gitgov_server user, NOT postgres
DATABASE_URL=postgresql://gitgov_server:YOUR_PASSWORD@db.YOUR_PROJECT_REF.supabase.co:5432/postgres
```

### 2. Configure Environment

```bash
cp .env.example .env
```

Edit `.env`:

```env
DATABASE_URL=postgresql://postgres:YOUR_PASSWORD@db.YOUR_PROJECT_REF.supabase.co:5432/postgres
GITGOV_JWT_SECRET=your-secure-secret-key
GITGOV_SERVER_ADDR=0.0.0.0:3000
GITHUB_WEBHOOK_SECRET=your-webhook-secret
```

### 3. Run Server

```bash
cargo run
```

## Job Queue Architecture

### Overview

The job queue provides **backpressure control** for webhook processing. Instead of processing detection synchronously, webhooks enqueue jobs that are processed by a background worker.

### Key Features

| Feature | Implementation | Purpose |
|---------|---------------|---------|
| **Atomic Claim** | `FOR UPDATE SKIP LOCKED` | No race conditions between workers |
| **Deduplication** | Partial unique index on `(org_id, job_type)` | Only 1 pending/running job per org+type |
| **Exponential Backoff** | `30s * 2^attempts`, capped at 1 hour | Prevents retry storms |
| **Dead-Letter Queue** | `status='dead'` after max_attempts | Failed jobs for manual inspection |
| **Stale Job Reset** | TTL of 5 minutes, safe reset | Recover from worker crashes |
| **Structured Logging** | `job_id`, `org_id`, `duration_ms` | Observability |

### Job States

```
pending -> running -> completed
                  \-> failed (with retry)
                  \-> dead (max attempts exceeded)
```

### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `max_attempts` | 10 | Max retries before dead-letter |
| `TTL` | 5 minutes | Time before job considered stale |
| `Poll Interval` | 5 seconds | How often worker checks for jobs |
| `Backoff Base` | 30 seconds | Base for exponential backoff |
| `Backoff Max` | 1 hour | Maximum backoff delay |

### Monitoring Jobs

```bash
# Get job metrics
curl -H "Authorization: Bearer $API_KEY" \
  http://localhost:3000/jobs/metrics

# Response:
{
  "worker_id": "worker-12345",
  "metrics": {
    "pending": 2,
    "running": 1,
    "completed_today": 45,
    "failed_today": 0,
    "dead": 1,
    "stale_running": 0,
    "avg_duration_ms": 1523,
    "oldest_pending_seconds": 12
  }
}

# List dead jobs
curl -H "Authorization: Bearer $API_KEY" \
  http://localhost:3000/jobs/dead

# Retry a dead job
curl -X POST -H "Authorization: Bearer $API_KEY" \
  http://localhost:3000/jobs/{job_id}/retry
```

### Troubleshooting

#### Stuck Jobs

If jobs are stuck in `running` state:

1. Check if worker process is alive
2. Wait for TTL (5 minutes) for auto-recovery
3. Manually inspect: `SELECT * FROM jobs WHERE status = 'running';`

#### Dead Jobs

Jobs that exceeded max attempts:

1. Check `last_error` column for failure reason
2. Fix underlying issue
3. Retry via API: `POST /jobs/{id}/retry`

#### High Pending Count

Many pending jobs:

1. Check worker logs for errors
2. Verify database connectivity
3. Check if jobs are being claimed: `SELECT COUNT(*) FROM jobs WHERE status = 'running';`

### SQL Queries for Debugging

```sql
-- View all jobs for an org
SELECT id, job_type, status, attempts, last_error, created_at
FROM jobs WHERE org_id = '...' ORDER BY created_at DESC;

-- Check for orphaned jobs
SELECT * FROM jobs WHERE status = 'running' AND locked_at < NOW() - INTERVAL '10 minutes';

-- Manual stale reset
SELECT reset_stale_jobs_safe(5);

-- View dead-letter queue
SELECT id, job_type, attempts, last_error, created_at
FROM jobs WHERE status = 'dead' ORDER BY created_at DESC;

-- Job throughput
SELECT 
    status,
    COUNT(*) as count,
    AVG(duration_ms) as avg_duration_ms
FROM jobs
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY status;
```

## Cursor-Based Incremental Processing

### Why `ingested_at` Instead of `created_at`

The `detect_noncompliance_signals` function uses **server-side ingestion time** (`ingested_at`) as the cursor, not event creation time (`created_at`).

**Problem with `created_at`:**
- Events can arrive late (retries, backlogs, network delays)
- `created_at` reflects when the event happened in GitHub
- A cursor on `created_at` would skip late-arriving events

**Solution with `ingested_at`:**
- Set at INSERT time by the database
- Never modified
- Guarantees processing order

```
GitHub Event Time:     10:00  (created_at)
Arrives at Server:     10:05  (ingested_at)
Cursor processes:      10:05  (using ingested_at)
```

### Schema

```sql
-- Added in supabase_schema_v2.sql
ALTER TABLE github_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE client_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();

-- Cursor stored in org_processing_state
last_ingested_at TIMESTAMPTZ,
last_processed_event_id UUID
```

## Append-Only Guarantee

### Tables with Append-Only Triggers

| Table | Trigger | Allowed Updates |
|-------|---------|-----------------|
| `github_events` | `BEFORE UPDATE OR DELETE` | None |
| `client_events` | `BEFORE UPDATE OR DELETE` | None |
| `violations` | Limited UPDATE | Only `resolved` fields |
| `noncompliance_signals` | `BEFORE UPDATE OR DELETE` | None |
| `governance_events` | `BEFORE UPDATE OR DELETE` | None |
| `signal_decisions` | `BEFORE UPDATE OR DELETE` | None |
| `policy_history` | `BEFORE UPDATE OR DELETE` | None |

### Jobs Table (NOT Append-Only)

The `jobs` table allows state transitions but restricts which columns can be updated:

- **Immutable**: `id`, `org_id`, `job_type`, `created_at`, `payload`
- **Mutable**: `status`, `locked_at`, `locked_by`, `attempts`, `last_error`, `next_run_at`

## Endpoints

### Health

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Simple health check |
| GET | `/health/detailed` | Detailed health with DB latency, uptime, pending events |

### Webhooks & Events

| Method | Path | Description |
|--------|------|-------------|
| POST | `/webhooks/github` | GitHub webhook receiver (push, create) |
| POST | `/events` | Client events (batch with idempotency) |

### Queries

| Method | Path | Description |
|--------|------|-------------|
| GET | `/logs` | Query combined events (github + client) |
| GET | `/stats` | Statistics |
| GET | `/dashboard` | Dashboard data |

### Compliance

| Method | Path | Description |
|--------|------|-------------|
| GET | `/compliance/:org` | Compliance dashboard (signals, correlation rate) |
| GET | `/signals` | List noncompliance signals with filters |
| POST | `/signals/:id` | Update signal status (investigate/dismiss) |
| POST | `/signals/detect/:org` | Trigger bypass detection |

### Policy

| Method | Path | Description |
|--------|------|-------------|
| GET | `/policy/:repo` | Get policy for repo |
| PUT | `/policy/:repo` | Save policy for repo |
| GET | `/policy/:repo/history` | Policy change history |

### Export

| Method | Path | Description |
|--------|------|-------------|
| POST | `/export` | Export events (JSON/PDF/Excel) with SHA256 hash |

### Admin

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api-keys` | Create API key |
| GET | `/jobs/metrics` | Job queue metrics |
| GET | `/jobs/dead` | List dead-letter jobs |
| POST | `/jobs/:id/retry` | Retry dead job |

## Correlation & Bypass Detection

### How It Works

1. Desktop sends `client_event` with `event_uuid` before/during push
2. GitHub sends webhook with `delivery_id` after push
3. Server correlates by `commit_sha + actor_login + branch`
4. If no client_event found → creates noncompliance signal

### Confidence Levels (NOT Binary)

| Level | Condition | Signal Type |
|-------|-----------|-------------|
| **High** | GitHub push with NO client event, empty outbox | `untrusted_path` |
| **Low** | GitHub push with NO client event, pending outbox events | `missing_telemetry` |

### Language (Evidence-Based)

The system uses evidence-based language, NOT accusations:

- ✓ `untrusted_path` - Direct push detected
- ✓ `missing_telemetry` - Incomplete data, outbox pending
- ✓ `noncompliance signal` - Generic term
- ✗ `bypass detected` - Too accusatory for automated detection
- ✗ `violation` - Requires manual confirmation

## Security

- **HMAC Signature**: Webhooks are validated using `X-Hub-Signature-256`
- **API Keys**: Desktop clients authenticate with API keys
- **RLS**: Row Level Security restricts data access by user/role
- **Append-Only**: Audit events cannot be tampered with
- **Export Hash**: Every export has SHA256 for verification
- **Bootstrap Security**: API keys printed only with explicit flag or TTY

### Bootstrap Key Security

The bootstrap admin key is only printed when:
1. Running with `--print-bootstrap-key` flag, OR
2. Running in an interactive terminal (TTY)

In Docker/Kubernetes environments (no TTY), the key is never printed to logs:

```bash
# Interactive (TTY) - key printed to console
cargo run

# Docker (no TTY) - key NOT printed
docker run gitgov-server

# Explicit flag - key always printed
docker run gitgov-server --print-bootstrap-key
```

## Testing

### Stress Tests

```bash
# Run stress test suite
cd tests
chmod +x stress_test.sh
./stress_test.sh

# With API key for full testing
SERVER_URL=http://localhost:3000 API_KEY=your-key ./stress_test.sh
```

### Manual Testing

```bash
# Health check
curl http://localhost:3000/health

# Simulate webhook
curl -X POST http://localhost:3000/webhooks/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: test-123" \
  -d '{
    "ref": "refs/heads/main",
    "before": "abc123",
    "after": "def456",
    "repository": {
      "id": 123,
      "name": "repo",
      "full_name": "org/repo",
      "private": false,
      "owner": {"id": 1, "login": "org"},
      "organization": {"id": 1, "login": "org"}
    },
    "sender": {"id": 1, "login": "developer"},
    "commits": [{"id": "def456", "message": "test"}]
  }'

# Check job metrics
curl -H "Authorization: Bearer $API_KEY" \
  http://localhost:3000/jobs/metrics
```

## Architecture

```
┌─────────────────┐     ┌─────────────────┐
│   GitHub        │────▶│  Webhook        │
│   (Webhooks)    │     │  POST /webhooks │
└─────────────────┘     └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │   Job Queue     │
                        │   (Postgres)    │
                        └────────┬────────┘
                                 │
┌─────────────────┐              │
│   Desktop App   │──────┐       │
│   (Outbox)      │      │       │
└─────────────────┘      │       │
                         ▼       ▼
                  ┌─────────────────┐
                  │  POST /events   │
                  │  (Batch)        │
                  └────────┬────────┘
                           │
                           ▼
                  ┌─────────────────┐
                  │   Supabase      │
                  │   PostgreSQL    │
                  │   (Append-only) │
                  └────────┬────────┘
                           │
                           ▼
                  ┌─────────────────┐
                  │ Job Worker      │
                  │ (background)    │
                  │                 │
                  │ ▶ claim_job     │
                  │ ▶ detect_signals│
                  │ ▶ complete_job  │
                  └─────────────────┘
```

## License

MIT
