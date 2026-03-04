# GitGov Desktop + Control Plane Performance and Scalability Audit

Date: 2026-03-04
Author: Codex (static analysis only)
Scope: Tauri desktop (`gitgov`), server (`gitgov-server`), SQL schema/migrations.

## 1) Executive Summary

This app already has a solid functional base, but current architecture has multiple hot paths that will degrade hard with large orgs (many repos, developers, events).

Top blockers found:

1. Outbox flush strategy can create thread pressure and long blocking windows.
2. Ingest path does per-event DB lookups and per-event insert statements (high write overhead).
3. Dashboard refresh pattern pulls heavy payloads every 30s (`logs` up to 500) and re-renders non-virtualized tables.
4. `GET /logs` query plan shape is expensive at scale (`UNION ALL` + global sort + offset + enrichment query).
5. `GET /stats` depends on a function with many full aggregates, called frequently.
6. DB connection pool is capped at 10.
7. Security/config debt can amplify instability (hardcoded fallback API key, plaintext local PIN/API key storage, permissive CORS, default JWT secret fallback).

This is not primarily a "Rust is slow" problem. The main issue is architecture in high-cardinality paths (query shapes, polling cadence, batching policy, payload size, and blocking behavior).

## 2) Scope, Method, and Guardrails

- Analysis only. No runtime behavior was changed.
- Golden Path was not modified in this audit.
- Method used: static code review with file-line evidence.
- No production load test was executed in this pass.

## 3) Evidence-Backed Findings

## 3.1 Critical Performance/Scalability Risks

### F-01 (Critical): Outbox flush can create avoidable blocking/thread pressure

Why it matters:
- Every major git action triggers `trigger_flush`, which spawns a new OS thread and executes blocking network I/O.
- Under server latency/outage, many short-lived threads can stack up and increase CPU/context-switch pressure.

Evidence:
- `gitgov/src-tauri/src/commands/git_commands.rs:27-31` (`trigger_flush` spawns a thread per call).
- Multiple flush trigger call sites: `git_commands.rs:319,347,430,454,543,559,599,641`.
- Flush network timeout is 30s: `gitgov/src-tauri/src/outbox/queue.rs:462-465` and worker path `555-557`.
- Background worker already exists (`start_background_flush(60)`), so immediate per-event thread flush is redundant pressure: `gitgov/src-tauri/src/lib.rs:109`.

### F-02 (Critical): Outbox flush sends all pending events in one big batch

Why it matters:
- A large pending queue produces a large request body, slower serialization/deserialization, and higher timeout risk.

Evidence:
- Flush snapshots all unsent events and sends one batch: `queue.rs:371-413`.
- No chunking/batch-size cap in this path.

### F-03 (Critical): Ingestion path has N+1 style DB work per event

Why it matters:
- In `POST /events`, each event may trigger org/repo lookups and possible repo upsert before insert.
- For large batches, latency and DB load scale linearly with high constant factors.

Evidence:
- Per-event org lookup: `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:63-69`.
- Per-event repo lookup/upsert path: `client_ingest_dashboard.rs:120-168`.
- Batch insert still issues one SQL insert per event inside transaction loop: `gitgov/gitgov-server/src/db.rs:525-563`.

### F-04 (Critical): `/logs` query shape is expensive for large tables

Why it matters:
- Two large event streams are combined with `UNION ALL`, then globally sorted and paginated with offset.
- Additional enrichment query runs after the main query.

Evidence:
- Combined query with `UNION ALL` and global `ORDER BY created_at DESC LIMIT/OFFSET`: `gitgov/gitgov-server/src/db.rs:759-824`.
- Secondary enrichment query by client event IDs: `db.rs:861-937`.
- Ordering only by `created_at` (no deterministic tie-breaker): `db.rs:822`.

### F-05 (Critical): Dashboard refresh cadence + payload size create recurring load spikes

Why it matters:
- Every 30s, admin path loads stats + daily + logs (500), plus heavy modules every 5 minutes.
- This repeats across every admin desktop session.

Evidence:
- Auto-refresh interval 30s: `gitgov/src/components/control_plane/ServerDashboard.tsx:61-63`.
- Refresh pipeline: `gitgov/src/store/useControlPlaneStore.ts:1130-1150`.
- Logs default load up to 500: `useControlPlaneStore.ts:1190-1198`.
- `refreshForCurrentRole` admin path always uses `logLimit: 500`: `useControlPlaneStore.ts:1723-1727`.

### F-06 (Critical): Stats endpoint likely expensive under frequent polling

Why it matters:
- `get_stats` delegates to `get_audit_stats` SQL function with multiple aggregate subqueries over large append-only tables.

Evidence:
- Server call: `gitgov/gitgov-server/src/db.rs:1688-1691`.
- SQL function aggregates with many `COUNT(*)`/`json_object_agg` scans: `gitgov/gitgov-server/supabase_schema.sql:423-449` and `supabase/supabase_schema_v12.sql:18-50`.

## 3.2 High Risks

### F-07 (High): DB pool max_connections=10 is low for target scale

Why it matters:
- Concurrent admin dashboards + ingest + chat + integrations can saturate 10 connections quickly.

Evidence:
- `gitgov/gitgov-server/src/db.rs:64-66`.

### F-08 (High): Team overview/repos queries are heavy CTE pipelines

Why it matters:
- Aggregation across windowed event data with nested JSON aggregation per user/repo is expensive for big orgs.

Evidence:
- Team overview query with CTEs and correlated `jsonb_agg`: `db.rs:3917-4007`.
- Team repos query over `window_events` + group + count distinct: `db.rs:4065-4118`.

### F-09 (High): Daily activity query uses expression on `created_at` that can weaken index usage

Why it matters:
- Joining by `(created_at AT TIME ZONE 'UTC')::date` can force broader scans.

Evidence:
- `gitgov/gitgov-server/src/db.rs:1743-1745`.

### F-10 (High): Outbox persistence rewrites full file on every add

Why it matters:
- `add()` persists full snapshot for each event, which increases disk I/O and lock contention with bigger queues.

Evidence:
- `add()` calls `persist()` every event: `queue.rs:313-336`.
- `persist()` writes full snapshot and atomic rename: `queue.rs:223-309`.

### F-11 (High): Outbox response reconciliation is O(n^2)

Why it matters:
- Repeated `Vec::contains` over accepted/duplicate UUID lists for each pending event scales poorly with large batches.

Evidence:
- Flush path: `queue.rs:426-434`.
- Worker path: `queue.rs:593-601`.

### F-12 (High): Event ingestion can reject full batch on first policy mismatch

Why it matters:
- First invalid event returns early in handler loop, potentially causing repeated retries/backlog growth.

Evidence:
- Early return inside loop for strict actor mismatch: `client_ingest_dashboard.rs:13-35`.
- Similar early return for synthetic login and org/repo mismatch: `client_ingest_dashboard.rs:43-61`, `73-97`, `129-149`.

### F-13 (High): Rate limit defaults are low for large multi-team load

Why it matters:
- For high traffic orgs, default limits can become artificial bottlenecks and increase retry storms.

Evidence:
- `/events` rate default 240/min: `gitgov/gitgov-server/src/main.rs:518-521`.
- Admin endpoints default 60/min: `main.rs:538-541`.
- Chat default 40/min: `main.rs:543-546`.

## 3.3 UI/Frontend Bottlenecks

### F-14 (High): Non-virtualized heavy tables + frequent data replacement

Why it matters:
- Large arrays are fully re-rendered when `serverLogs` updates.

Evidence:
- Full rows built from logs each update: `gitgov/src/components/control_plane/RecentCommitsTable.tsx:57-62`.
- Table maps rows directly (no virtualization): `RecentCommitsTable.tsx:152-204`.
- Team tables also map full arrays: `gitgov/src/components/control_plane/TeamManagementPanel.tsx:127-149` and `173-181`.

### F-15 (Medium): CPU overhead in fuzzy lookup per row

Why it matters:
- For each commit row, fallback loops over full map entries for SHA prefix matching.

Evidence:
- Pipeline fallback scan: `RecentCommitsTable.tsx:84-87`.
- PR fallback scan: `RecentCommitsTable.tsx:96-99`.

### F-16 (High): Chat history persistence can silently fail and appear as "history not saved"

Why it matters:
- Large localStorage payloads are serialized synchronously; persistence errors are swallowed.
- If quota or storage errors occur, user gets no visible failure.

Evidence:
- Sync serialization before async write: `gitgov/src/store/useControlPlaneStore.ts:787-820`.
- Silent failure on setItem: `useControlPlaneStore.ts:821-825`.

### F-17 (Medium): Dashboard intentionally uses a 500-event window, not full history

Why it matters:
- Users can perceive missing history as data loss when it is a frontend windowing decision.

Evidence:
- Logs load limit 500: `useControlPlaneStore.ts:1190-1198`.
- UI explicitly states recent window only: `RecentCommitsTable.tsx:122`.

## 3.4 Backend Logic/Query Risks

### F-18 (High): Optional filter pattern (`$x IS NULL OR column = $x`) repeated heavily

Why it matters:
- This pattern can reduce planner selectivity and lead to broader scans.

Evidence:
- `/logs` query has many OR-optional filters: `db.rs:781-820`.

### F-19 (High): Chat deterministic queries can become scan-heavy at global scope

Why it matters:
- Global admin can query with no org filter, which scans all-org data.

Evidence:
- Chat scope allows `None` org when global and org not provided: `gitgov/gitgov-server/src/handlers/chat_handler.rs:200-206`.
- Scope resolver explicitly returns `Ok(None)` in that case: `gitgov/gitgov-server/src/handlers/gdpr_clients_identities_scope.rs:346-347`.

### F-20 (High): Chat no-ticket push query uses JSON array expansion per row

Why it matters:
- `jsonb_array_elements_text` inside `NOT EXISTS` can be expensive on large `github_events`.

Evidence:
- `db.rs:4657-4661`, `4698-4702`, `4870-4874`.

### F-21 (Medium): `ILIKE` matching in user queries can hurt index use

Why it matters:
- Case-insensitive text matching in hot analytical queries can be slower than normalized equality paths.

Evidence:
- `db.rs:4805`, `4834`, `4867`, `4907`, `4952`, `4992`, `5038`.

## 3.5 Security / Business Rule Risks

### F-22 (Critical): Hardcoded legacy API key fallback in frontend

Why it matters:
- Hidden fallback key can mask auth/config problems and is a clear security risk.

Evidence:
- Constant defined: `gitgov/src/store/useControlPlaneStore.ts:477`.
- Used as last-resort in config resolution: `useControlPlaneStore.ts:847-853`.

### F-23 (High): Control Plane API key stored in localStorage plaintext

Why it matters:
- Local compromise or browser-context access can expose key.

Evidence:
- Read/write config in localStorage: `useControlPlaneStore.ts:551-571`.

### F-24 (High): Local unlock PIN stored plaintext in localStorage

Why it matters:
- No hashing/encryption for PIN at rest.

Evidence:
- Key definition and storage: `gitgov/src/store/useAuthStore.ts:6`, `39`, `205`, `227`.

### F-25 (High): Server uses fallback JWT secret string if env missing

Why it matters:
- Insecure default in production-like deployments.

Evidence:
- `gitgov/gitgov-server/src/main.rs:239-240`.

### F-26 (Medium): Permissive CORS on all origins/methods/headers

Why it matters:
- Broad CORS increases attack surface if keys leak into browser contexts.

Evidence:
- `gitgov/gitgov-server/src/main.rs:689`.

### F-27 (Medium): Repo auto-upsert from client metadata can inflate/poison repo catalog

Why it matters:
- A scoped client can create repo records from payload metadata without remote verification.

Evidence:
- Auto-upsert path from event metadata repo name: `client_ingest_dashboard.rs:154-168`.

## 3.6 Crash Analysis: "Component is not a function"

Observed symptom:
- React Router "Unexpected Application Error" with `TypeError: Component is not a function`.

Evidence and likely causes:

1. Dynamic component rendering exists in code and can fail if runtime value is invalid.
- `gitgov/src/components/shared/Toast.tsx:52` (`const Icon = iconMap[t.type]`) and `Toast.tsx:74` (`<Icon .../>`).
- `gitgov/src/pages/HelpPage.tsx:136-143` uses `const Icon = section.icon` + render.

2. Router has no explicit route `errorElement`, so route-render exceptions surface as default React Router error page.
- `gitgov/src/router.tsx:26-75`.

3. Top-level app has ErrorBoundary, but route-level errors can still render Router's fallback page.
- `gitgov/src/App.tsx:56-59`.

Conclusion:
- Exact throw site is NOT VERIFIED from static review only, but current codebase has dynamic component render points and missing route `errorElement`, matching the symptom pattern.

## 4) Root Cause Pattern for "No responde" / Freezes

Most plausible combined chain:

1. Frequent heavy polling + large payloads (`/logs` up to 500) repeatedly cross Rust<->JS boundary.
2. Expensive SQL query shapes and low DB pool increase tail latency.
3. Outbox strategy adds extra blocking pressure under unstable network/server conditions.
4. UI re-renders full non-virtualized tables and performs extra per-row matching work.
5. Chat persistence and large JSON serialization can add main-thread hitches.

This is consistent with temporary app unresponsiveness without removing functionality.

## 5) Prioritized Remediation Plan (No Golden Path Removal)

Phase P0 (safe, low-risk, high ROI)

1. Outbox flush control:
- Replace per-action thread spawn flush with debounced signal to existing worker.
- Add batch chunking (e.g., 100 events max/request).
- Replace UUID list `contains` checks with `HashSet`.

2. Dashboard polling/load-shedding:
- Keep 30s refresh but avoid re-fetching unchanged heavy panels every cycle.
- Pull logs incrementally (cursor/keyset) instead of full 500 replace.

3. `/logs` query optimization:
- Add deterministic order (`created_at DESC, id DESC`).
- Move to keyset pagination for high offsets.
- Minimize details payload for default dashboard view.

4. Security debt cleanup:
- Remove hardcoded fallback API key.
- Stop storing API key/PIN plaintext in localStorage (or at least hash PIN, move API key to secure storage).

Phase P1 (schema/query level)

1. Introduce pre-aggregated/stat cache path for `/stats`.
2. Rework daily activity query to keep indexable range predicates on `created_at`.
3. Refactor ingest path to pre-resolve org/repo per batch and multi-row insert.
4. Add/validate composite indexes by hottest predicates (`org_id + created_at + event_type`).

Phase P2 (scale hardening)

1. Raise/tune DB pool and rate limits by environment profile.
2. Add server response compression for large JSON endpoints.
3. Add load tests for org-scale traffic with realistic event distributions.

## 6) Business Rule/Contract Notes

- Golden Path components are coupled to event flow and auth header contracts; improvements should be implementation-level, not behavior removal.
- Existing anti split-brain normalization to `127.0.0.1` is present and good:
  - Frontend normalize: `gitgov/src/store/useControlPlaneStore.ts:527-542`
  - Tauri normalize: `gitgov/src-tauri/src/control_plane/server.rs:640-652`

## 7) NO VERIFICADO Items

NO VERIFICADO: Exact runtime root cause for the specific `Component is not a function` crash in your screenshot.
- Missing: runtime stack with source maps and component name.
- Needed next: capture browser console stack + route/component state at crash moment.

NO VERIFICADO: Real p95/p99 latency and CPU/memory under production-like load.
- Missing: benchmark environment and representative dataset volume.
- Needed next: controlled load test with org-scale event tables.

NO VERIFICADO: Actual DB execution plans for hot queries in your production dataset.
- Missing: `EXPLAIN (ANALYZE, BUFFERS)` against real data.
- Needed next: run explain on `/logs`, `/stats`, team queries, chat no-ticket queries.

## 8) Files Reviewed (Audit Evidence Set)

Desktop frontend/store:
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/store/useRepoStore.ts`
- `gitgov/src/store/useAuthStore.ts`
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`
- `gitgov/src/components/control_plane/dashboard-helpers.ts`
- `gitgov/src/components/control_plane/ConversationalChatPanel.tsx`
- `gitgov/src/components/control_plane/TeamManagementPanel.tsx`
- `gitgov/src/components/shared/Toast.tsx`
- `gitgov/src/components/shared/ErrorBoundary.tsx`
- `gitgov/src/pages/ControlPlanePage.tsx`
- `gitgov/src/pages/DashboardPage.tsx`
- `gitgov/src/pages/HelpPage.tsx`
- `gitgov/src/components/layout/Header.tsx`
- `gitgov/src/components/layout/MainLayout.tsx`
- `gitgov/src/router.tsx`
- `gitgov/src/App.tsx`

Tauri Rust desktop:
- `gitgov/src-tauri/src/lib.rs`
- `gitgov/src-tauri/src/commands/git_commands.rs`
- `gitgov/src-tauri/src/commands/server_commands.rs`
- `gitgov/src-tauri/src/control_plane/server.rs`
- `gitgov/src-tauri/src/outbox/queue.rs`
- `gitgov/src-tauri/src/git/repository.rs`
- `gitgov/src-tauri/src/git/branch.rs`
- `gitgov/src-tauri/src/commands/auth_commands.rs`

Server + DB:
- `gitgov/gitgov-server/src/main.rs`
- `gitgov/gitgov-server/src/auth.rs`
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs`
- `gitgov/gitgov-server/src/handlers/chat_handler.rs`
- `gitgov/gitgov-server/src/handlers/gdpr_clients_identities_scope.rs`
- `gitgov/gitgov-server/src/handlers/prelude_health.rs`
- `gitgov/gitgov-server/src/handlers/conversational/core.rs`
- `gitgov/gitgov-server/src/handlers/conversational/engine.rs`
- `gitgov/gitgov-server/src/handlers/conversational/query.rs`
- `gitgov/gitgov-server/supabase_schema.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v6.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v8.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v9.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v12.sql`

---

Status: analysis complete. No functional code path was changed in this audit report.
