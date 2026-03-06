# Forensic Pre-Prod Report (2026-03-06)

## Scope
- Secret exposure scan in working tree.
- Secret marker scan in recent git history (`last 60 commits`).
- Incident marker scan in local runtime/test logs.
- Runtime contract checks on `http://127.0.0.1:3000`:
  - `/health`
  - `/stats` (Bearer)
  - `/logs` (Bearer)
  - `/events` dedup (`accepted` then `duplicates`).

## Evidence
- Raw forensic artifact:
  - [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json)
- Forensic runner:
  - [forensic_preprod_scan.py](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/forensic_preprod_scan.py)

## Findings
1. `High` Active secret-like material in working tree.
Evidence:
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:4)
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:7)
Details:
- `working_tree.total_findings=10`
- Includes API-key/token assignments in local env files and token-like patterns in test content.
Action:
- Rotate exposed credentials.
- Move prod secrets to secret manager/CI protected variables.
- Keep `.env` out of shared channels and artifacts.

2. `Medium` Secret markers present in recent git history.
Evidence:
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:15)
Details:
- `git_history.total_findings=54` in last 60 commits.
- Majority are marker/assignment references; includes `2` token-like hits.
Action:
- Run full-history secret scan (not only last 60).
- If confirmed real secrets in history, perform rotation and history remediation policy.

3. `Medium` Historical DB saturation signatures exist in logs.
Evidence:
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:25)
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:31)
Details:
- `max_clients_session_mode=195` hits in historical tuning logs.
- No new `db_error_fail_open`, no unexpected `401`, no auth backend outage markers in scanned logs.
Action:
- Keep `GITGOV_DB_MAX_CONNECTIONS` tuned and monitor `/outbox/lease/metrics` continuously.

4. `Pass` Runtime contract and dedup are healthy.
Evidence:
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:40)
- [forensic_preprod_scan_2026-03-06_runtime.json](/C:/Users/PC/Desktop/GitGov/gitgov/gitgov-server/tests/artifacts/forensic_preprod_scan_2026-03-06_runtime.json:49)
Details:
- `health_ok=true`, `stats_ok=true`, `logs_ok=true`.
- Dedup check passed: first `/events` accepted, second marked as duplicate.
- `runtime_ok=true`.

## Risk Decision
- Technical runtime path is stable for release.
- Main residual risk is credential hygiene (active and historical secret-like data).

## Pre-Prod Go/No-Go
- `Conditional GO` if:
  - credential rotation is completed,
  - secrets are managed outside plain local files for production,
  - runtime monitoring remains enabled for lease/db error markers.

## NO VERIFICADO
- Full git history forensic scan beyond last 60 commits.
- External systems forensic scope (CI vault, deployment platform, external log sinks).
