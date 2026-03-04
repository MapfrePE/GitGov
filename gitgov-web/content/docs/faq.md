---
title: FAQ
description: Frequently asked questions about GitGov — what it does, what it doesn't, and how it works.
order: 8
---

## General

### What is GitGov?

GitGov is a **Git governance platform** that captures operational metadata from developer workstations (commits, pushes, branches) and sends it to a central Control Plane for audit, compliance, and policy monitoring. It does not read, analyze, or transmit source code.

### Who is GitGov for?

GitGov is designed for **engineering teams and organizations** that need:

- Audit trails for regulatory compliance (SOC 2, ISO 27001, PCI-DSS).
- Visibility into development operations without inspecting code.
- Policy enforcement awareness (e.g., protected branch rules, working hours).
- CI/CD traceability linking commits to Jenkins pipelines and Jira tickets.
- Team and organization governance across multiple repositories.

### Is GitGov open source?

GitGov is developed by **Yohandry Chirinos**, a Venezuelan software engineer with over 10 years of experience in diverse enterprise environments, banking, fintech, and startups. GitGov is not open source — it emerged as a software product designed to improve operational traceability for organizations.

### What technology stack does GitGov use?

- **Desktop App**: Tauri v2 + React 19 + Tailwind v4 + Zustand v5 — native desktop for Windows, macOS, and Linux.
- **Control Plane Server**: Axum (Rust) — high-performance REST API for event ingestion, policy checks, and the admin dashboard.
- **Database**: PostgreSQL (Supabase-managed or self-hosted) — stores all audit events, organization data, and integrations.
- **Web App**: Next.js 14 — marketing site and documentation at git-gov.vercel.app.

---

## What GitGov Does NOT Do

### Does GitGov read my source code?

**No.** Only metadata is captured: event type, commit SHA, branch name, author, timestamp, file paths (up to 500), and repository name. Source code, file contents, diffs, and commit message bodies are **never transmitted**.

### Does GitGov monitor my screen, keystrokes, or applications?

**No.** GitGov only observes Git operations. It has no access to your screen, clipboard, browser, IDE, or any application outside of Git.

### Does GitGov analyze code quality or run static analysis?

**No.** GitGov captures metadata about *when* and *where* Git events happen, not *what* the code contains.

### Does GitGov replace CI/CD?

**No.** GitGov integrates with CI/CD tools (Jenkins, GitHub Actions) to correlate commits with pipeline results. It does not run builds, tests, or deployments.

### Does GitGov block Git operations?

**No.** GitGov is a **detection and observability** tool. It can flag that a push to a protected branch occurred, but does not prevent the push.

### Does GitGov make HR or disciplinary decisions?

**No.** Signals are **advisory observations**. The deploying organization is fully responsible for any decisions made based on signals.

### Does GitGov profile individual developer productivity?

**No.** There are no "lines of code", "commit frequency scores", or productivity rankings.

### Does GitGov sell or share my data?

**No.** All data belongs to the deploying organization. No data monetization. No third-party sharing.

### Does GitGov store passwords or secrets?

**No.** API keys are hashed (SHA-256). GitGov never collects passwords, tokens, or `.env` values.

### Does GitGov collect information about my computer?

**No.** No hardware specs, network info, installed software, or system telemetry. Only the client app version is captured.

---

## Data & Security

### Where is my data stored?

Event data is stored in a **PostgreSQL database** controlled by the deploying organization (Supabase-managed or self-hosted). The desktop app uses a **JSONL outbox** for offline event queuing and a separate **SQLite database** for local audit logs. See [Security Policy](/docs/security) for full details.

### Is my data encrypted?

**Yes, at multiple layers:**

- **In transit:** TLS (HTTPS) between Desktop and Control Plane.
- **At rest:** AES-256 disk encryption on Supabase-managed databases; API keys stored as SHA-256 hashes.
- **On the workstation:** API keys stored in the OS keyring (Windows DPAPI, macOS Keychain, Linux Secret Service).

### Can audit records be modified or deleted?

**No.** Audit event records are **append-only** by design. PostgreSQL triggers enforce this at the database level. Every data export is logged as an audit event.

### Who can see my events?

- **Developers** see only their own events.
- **Admins** see all events within their organization.
- Data is scoped by organization — admins of one org cannot see another org's data.

### How are API keys protected?

Hashed with SHA-256 before storage. Plaintext shown only once at creation. On desktop, stored in OS keyring. Revocation takes effect immediately.

### How long is my data retained?

Audit data: minimum **1,825 days (5 years)**. Configurable upward. Session data has a shorter, separate retention period.

### Can I export my data?

**Yes.** `POST /export` provides machine-readable JSON. Each export is logged as an audit event (chain of custody).

### Does GitGov support the right to erasure (GDPR)?

**Yes.** Anonymization/deletion endpoint available within org scope. Returns 404 for non-existent users (privacy-preserving).

---

## Desktop App

### What platforms does GitGov Desktop support?

**Windows**, **macOS**, and **Linux** — built with Tauri.

### What happens if I lose internet connectivity?

Events are queued in a local **JSONL outbox** with exponential backoff retries. No events are lost.

### Does GitGov require admin privileges to install?

No. Installs in user space.

### What does the Desktop App capture?

1. **Stage files** — file paths staged for commit (no content).
2. **Commit** — SHA, branch, file count, status.
3. **Push** — target branch, success/failure, block reasons.
4. **Branch operations** — creation, checkout.

Each event carries a unique `event_uuid` for deduplication.

### Can I set a PIN lock on the Desktop App?

**Yes.** Optional 4-6 digit PIN in Settings. Protects local access — separate from server authentication.

### How do I update the Desktop App?

Settings > Updates: select channel (Stable/Beta), check for updates, download, and install. Changelog available per version.

### How do I configure governance policies?

Define a `gitgov.toml` file in your repository root. See [Governance & Policies](/docs/governance).

---

## Organizations & Teams

### How do I create an organization?

Admin creates an org through the Desktop App's Settings panel. All data (events, members, keys, integrations) is scoped to this org.

### How do I add developers to my organization?

Two methods:

1. **Direct provisioning** — Admin enters login, email, and role. Member created immediately.
2. **Invitation** — Admin generates a token with role and expiry. Developer accepts in the Desktop App, which creates their account and issues an API key automatically.

### What roles are available?

| Role | Access |
|------|--------|
| **Admin** | Full: events, stats, dashboard, integrations, team management, API keys, policies |
| **Developer** | Own events only |
| **Architect** | Same as Developer (reserved for future granular permissions) |
| **PM** | Same as Developer (reserved for future reporting access) |

### Can I disable a team member without deleting them?

**Yes.** Set status to **disabled**. API keys stop working immediately. Historical data preserved.

### How do invitation tokens work?

Generated by admins, hashed (SHA-256) before storage, with configurable expiration. Once accepted, consumed and non-reusable. Admins can resend or revoke pending invitations.

### How do I manage API keys for my team?

Settings > API Keys: list, create (with role), revoke (immediate), or issue for a specific member. Plaintext shown once only.

---

## Dashboard & Analytics

### What does the Admin Dashboard show?

- **Metrics Grid** — Total events, success rate, active repos, pushes, blocked events, active developers.
- **Pipeline Health** — Jenkins 7-day build metrics: total, success rate, failures, average duration.
- **Daily Activity** — Commit/push trends (14 days).
- **Ticket Coverage** — Percentage of commits linked to Jira tickets.
- **Event Breakdown** — Distribution by type/status, commits without tickets, tickets without commits.
- **Recent Commits Table** — Paginated events with CI badges, PR badges, Jira ticket links.
- **Governance Chat** — Integrated AI assistant.

### How often does the Dashboard refresh?

Every **30 seconds** by default. Toggle on/off available.

### What do the CI badges mean?

- **Green** — Pipeline passed.
- **Red** — Pipeline failed.
- **Yellow** — Unstable/aborted.
- **No badge** — No Jenkins correlation found.

### What do the ticket badges mean?

Jira ticket references detected in branch names or commit metadata (e.g., `PROJ-123`). Click to see ticket detail: status, assignee, related branches/commits/PRs.

### What does a Developer see?

Their own commits/pushes and an option to accept org invitations. No org-wide stats or team management.

### What timezone does the Dashboard use?

Stored in **UTC**. Display timezone configurable in Settings (12 IANA zones). Display-only — underlying data stays UTC.

---

## Governance Chat (AI Assistant)

### What can I ask the chat?

- **Analytics**: "Who pushed to main without a ticket this week?", "How many commits did alice make?"
- **Configuration**: "How do I set up Jenkins?", "How do I configure branch protection?"
- **Troubleshooting**: "Why am I getting 401?", "Why is my dashboard empty?"
- **Product**: "What integrations exist?", "What roles are available?"

### Does the chat access my source code?

**No.** Only event metadata and product knowledge. Never source code, diffs, or file contents.

### What if the chat doesn't know the answer?

It indicates insufficient data or offers to register a **feature request** for the product team.

---

## Control Plane & Server

### What is the Control Plane?

The central **Axum (Rust) server** — receives events, processes webhooks, runs policy checks, serves the dashboard.

### Can I self-host the Control Plane?

**Yes.** Any server running Rust binaries + PostgreSQL. See [Connect to the Control Plane](/docs/control-plane).

### What integrations are supported?

- **GitHub** — Push/branch webhooks, HMAC validation, audit log streaming.
- **Jenkins** — Pipeline ingestion, commit-pipeline correlation, health widget, policy advisory.
- **Jira** — Ticket ingestion, batch correlation, coverage reports, ticket detail.

### What happens if the server goes down?

Desktop clients keep working. Events queue locally and sync when the server returns.

### How does rate limiting work?

Configurable per endpoint. Defaults: 240 req/min (events), 60 req/min (admin), 120 req/min (Jenkins/Jira). 429 response = increase limits via environment variables.

---

## Compliance

### Does GitGov help with SOC 2?

**Yes.** Append-only audit trails, RBAC, immutable records, export capabilities — key SOC 2 Type II controls.

### Does GitGov help with GDPR?

**Yes.** Data minimization, right of access, portability (export), right to erasure, controller/processor distinction. See [Privacy & Signal Liability](/docs/privacy).

### Does GitGov help with ISO 27001?

**Yes.** Append-only audit trail, RBAC, encrypted storage, exports support Annex A controls.

### What is a "signal"?

An automated flag for a potential policy deviation (e.g., unauthorized push to protected branch, commit without ticket). **Advisory only** — requires human review. See [Privacy & Signal Liability](/docs/privacy).

### Can I review and dismiss signals?

**Yes.** Admins confirm, escalate, or dismiss. Every decision logged with actor, reason, and timestamp.

---

## Related

- [**Security Policy**](/docs/security) — Encryption, storage, access controls.
- [**Privacy & Signal Liability**](/docs/privacy) — Legal boundaries and GDPR.
- [**Privacy Policy**](/privacy) — Legal terms for end-users.
- [**Introduction**](/docs/introduction) — Getting started with GitGov.
