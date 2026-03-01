---
title: Security Policy
description: What GitGov captures, where it is stored, who can access it, and how your data is protected at every layer.
order: 7
---

GitGov is designed with the principle of **least data, maximum protection**. This page details the full security posture of the platform — what enters the system, how it is stored, who can access it, and what technical controls are in place.

---

## What GitGov Captures

GitGov Desktop captures **operational metadata only** — never source code, file contents, diffs, commit messages, secrets, or `.env` values.

| Data Point | Example | Captured? |
|------------|---------|-----------|
| Event type | `commit`, `push` | Yes |
| Commit SHA | `a3f8c2e` | Yes |
| Branch name | `feat/auth` | Yes |
| Git author | `alice` | Yes |
| Timestamp | ISO 8601 | Yes |
| File count | `12` | Yes |
| Repo name | `org/repo` | Yes |
| Client version | `0.1.0` | Yes |
| **Source code** | — | **Never** |
| **File contents** | — | **Never** |
| **Diff content** | — | **Never** |
| **Commit message body** | — | **Never** |
| **Passwords / secrets** | — | **Never** |
| **.env values** | — | **Never** |

> No source code ever leaves the developer workstation. This is an architectural guarantee, not a configuration option.

---

## Where Data Is Stored

### Control Plane Server

Event records are stored in a **PostgreSQL database** (hosted on Supabase in our managed deployment). The deploying organization controls the database instance and its geographic location.

| Layer | Technology | Details |
|-------|-----------|---------|
| **Database** | PostgreSQL 15+ | Supabase-managed or self-hosted |
| **Connection** | TLS-encrypted pooler | Connection string uses `sslmode=require` |
| **Backups** | Supabase daily backups | Or organization-managed backup policy |
| **Region** | Configurable | Organization selects region at deployment |

### Desktop Client

The desktop app stores a **local SQLite outbox** for offline resilience. Events are queued locally and synced to the server when connectivity is restored.

| Local Storage | Purpose | Protection |
|--------------|---------|------------|
| Outbox (SQLite) | Queued events pending sync | Local filesystem, cleared after successful delivery |
| API key | Server authentication | Stored in OS keyring (never plaintext files) |
| `gitgov.toml` | Policy configuration | Checked into the repository — no secrets |

---

## Encryption

### In Transit

All communication between GitGov Desktop and the Control Plane is protected by **TLS (HTTPS)** in production environments.

- HTTP is only supported for **local development** (`127.0.0.1:3000`).
- Production deployments **must** use HTTPS with a valid certificate.
- Webhook payloads from GitHub, Jenkins, and Jira are validated with **HMAC signatures** or dedicated webhook secrets before processing.

### At Rest

| Component | Encryption at Rest |
|-----------|-------------------|
| PostgreSQL (Supabase) | AES-256 disk encryption (Supabase default) |
| Database backups | Encrypted at rest per Supabase/provider policy |
| API keys in database | Stored as **SHA-256 hashes** — plaintext never persisted after initial issuance |
| Local outbox (SQLite) | Protected by OS filesystem permissions |
| OS keyring (API key) | Protected by OS credential storage (Windows DPAPI, macOS Keychain, Linux Secret Service) |

---

## Who Can Access What

GitGov enforces **role-based access control (RBAC)** at the API level. Every request requires a valid `Authorization: Bearer` token.

| Role | Own Events | All Events | Stats/Dashboard | Integrations | API Key Management |
|------|-----------|------------|----------------|-------------|-------------------|
| **Developer** | Read | — | — | — | — |
| **Architect** | Read | — | — | — | — |
| **PM** | Read | — | — | — | — |
| **Admin** | Read | Read | Read | Read/Write | Create |

### Access Control Details

- **Developers** can only see their own event records (`GET /logs` is scoped by `user_login`).
- **Admins** have full visibility: all events, statistics, dashboard, integrations, compliance signals, and API key creation.
- There is **no superuser bypass** — the Admin role is the highest privilege level, and it is still subject to authentication.
- API keys are hashed (SHA-256) before storage. The server never stores or logs plaintext keys.

---

## Audit Trail Integrity

Event records are **append-only**. The system is architecturally designed to prevent tampering:

- **No UPDATE** — audit event records cannot be modified via the API.
- **No DELETE** — audit event records cannot be deleted via the API.
- **Deduplication** — each event carries a unique `event_uuid`; duplicates are rejected.
- **Export logging** — every data export (`POST /export`) is itself logged as an audit event, creating an immutable chain of custody.

This design supports compliance frameworks including **SOC 2**, **ISO 27001**, and **PCI-DSS** audit trail requirements.

---

## Authentication Security

| Mechanism | Details |
|-----------|---------|
| **API Key hashing** | SHA-256 — server computes hash of the bearer token and looks up by `key_hash` |
| **Key storage** | OS keyring on Desktop; hashed column in PostgreSQL on server |
| **JWT signing** | `GITGOV_JWT_SECRET` — must be a strong, unique secret in production (`openssl rand -hex 32`) |
| **Webhook validation** | GitHub: HMAC-SHA256; Jenkins/Jira: dedicated secrets via headers |
| **Rate limiting** | Per-route configurable rate limits (events: 240/min, admin: 60/min) |

---

## Network Security

- The Control Plane listens on `0.0.0.0:3000` by default (configurable via `GITGOV_SERVER_ADDR`).
- Local development uses `127.0.0.1:3000` (loopback only) to prevent accidental exposure.
- Production deployments should be placed behind a **reverse proxy** (e.g., Nginx, Caddy) with TLS termination.
- CORS and request body size limits are enforced per integration endpoint.

---

## What GitGov Does NOT Do

This section is critical for setting accurate expectations:

| GitGov Does NOT... | Explanation |
|-------------------|-------------|
| **Read your source code** | Only metadata (SHA, branch, author, timestamp, file count) is captured. |
| **Analyze code quality** | No static analysis, linting, or code review functionality. |
| **Monitor keystrokes or screen** | GitGov only observes Git operations, not developer behavior. |
| **Make HR decisions** | Signals are advisory observations, not disciplinary determinations. |
| **Replace CI/CD** | GitGov traces CI pipelines but does not run builds, tests, or deployments. |
| **Enforce branch protection** | GitGov detects policy violations; it does not block Git operations. |
| **Store passwords or secrets** | API keys are hashed; no passwords are ever collected. |
| **Access private repositories** | GitGov does not clone, fetch, or read repository content. |
| **Profile individual productivity** | There are no "lines of code" or "commit frequency" performance scores. |
| **Sell or share your data** | Data belongs to the deploying organization. GitGov has no data monetization. |

---

## Incident Response

If a security vulnerability is discovered in GitGov:

1. Report it to **security@gitgov.io** with a detailed description.
2. Do not disclose publicly until a fix is available.
3. We aim to acknowledge reports within **48 hours** and provide a remediation timeline within **7 business days**.

---

## Related

- [**Privacy & Signal Liability**](/docs/privacy) — Legal boundaries and GDPR compliance.
- [**Privacy Policy**](/privacy) — Full legal terms for end-users.
- [**Governance & Policies**](/docs/governance) — Configuring `gitgov.toml` rules.
