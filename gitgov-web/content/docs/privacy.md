---
title: Privacy & Signal Liability
description: What data GitGov captures, what signals mean legally, and the responsibility boundaries for deploying organizations.
order: 6
---

GitGov is built on a foundational principle: **metadata only, never source code**. This page explains exactly what is collected, what signals represent, and the explicit limits of GitGov's liability when signal data is used to inform organizational decisions.

---

## What GitGov Captures

GitGov Desktop captures the following fields per Git event. Nothing more.

| Field | Example | Description |
|-------|---------|-------------|
| `event_type` | `commit`, `push` | The Git operation that occurred |
| `commit_sha` | `a3f8c2e` | Identifier linking the event to a specific commit |
| `branch` | `feat/auth` | Target branch of the operation |
| `user_login` | `alice` | Git author identifier (`git config user.name`) |
| `timestamp` | ISO 8601 | When the operation occurred on the developer's machine |
| `file_count` | `12` | Count of files staged — **not their names, not their content** |
| `repo_name` | `org/repo` | Repository identifier |
| `client_version` | `0.1.0` | Desktop app version for protocol compatibility |

> **Absolute guarantee:** Source code content, file contents, diff contents, commit message bodies, passwords, secrets, and `.env` values are **never transmitted** and never leave the developer workstation.

---

## Non-Compliance Signals

The Control Plane can generate **signals** — automated flags that indicate a potential deviation from configured governance policies. Examples:

- A `successful_push` to a branch listed as `protected` in `gitgov.toml` by a user not in `admins` or an authorized group.
- A commit outside configured operational hours (if a time-window policy is defined).
- A push without a corresponding Jira ticket reference (if ticket-coverage policy is active).

### Critical: Advisory Nature

> [!IMPORTANT]
> **Signals are computational observations. They are not legal conclusions, HR determinations, or findings of misconduct.**

A signal means: _"a configured rule was triggered based on available metadata."_ It does not establish:

- **Intent** — the developer may have had a legitimate reason.
- **Negligence** — configuration errors or clock skew can produce false positives.
- **Fault** — the signal has no knowledge of context beyond the captured metadata.

GitGov provides **no warranty** — express or implied — as to the accuracy, completeness, or fitness of signal data for any employment, disciplinary, or legal purpose.

---

## Liability Boundaries

### GitGov's responsibility ends at the signal.

| Boundary | GitGov's Role | Deploying Organization's Role |
|----------|--------------|-------------------------------|
| **Data capture** | Captures metadata per the schema above | Must inform developers that monitoring is active |
| **Signal generation** | Flags policy deviations based on configured rules | Responsible for policy configuration accuracy |
| **Signal interpretation** | None — signals carry no judgement | Responsible for human review before any action |
| **Decisions based on signals** | None — GitGov makes no decisions | Assumes full legal responsibility for HR/disciplinary actions |
| **False positives** | Provides tooling to review and dismiss signals | Must not act on unreviewed signals |
| **Data subject rights** | Provides export endpoint (`POST /export`) | Acts as data controller; handles individual DSAR requests |

### Deploying organizations must:

1. **Inform employees** that Git operational metadata is captured before deployment.
2. **Establish legal basis** (legitimate interests, legal obligation, or contract) for processing under applicable data protection law (GDPR Art. 6).
3. **Configure policies accurately** — a misconfigured `gitgov.toml` will produce incorrect signals.
4. **Require human review** before taking any action based on a signal.
5. **Comply with local labor law** — in many jurisdictions, employee monitoring requires works council consultation, individual notice, or regulatory approval.

---

## Data Retention and Immutability

Audit event records are **append-only**. The system is designed to prevent tampering with historical records — a core requirement for compliance frameworks like SOC 2 and ISO 27001.

- Records cannot be modified or deleted via the standard API.
- Retention period is configured by the deploying organization.
- GitGov does not impose a maximum retention period; organizations must define their own retention policies in line with GDPR data minimization principles.

---

## GDPR Reference

For EU-based deployments:

| GDPR Concept | Implementation |
|-------------|---------------|
| **Data controller** | The deploying organization |
| **Data processor** | GitGov (software + operators) |
| **Legal basis** | Art. 6(1)(b) contract, 6(1)(c) legal obligation, or 6(1)(f) legitimate interests |
| **Data minimization** | Only operational metadata — no content, no diffs |
| **Right of access** | `GET /logs?user_login={user}` scoped to own data for Developer role |
| **Portability** | `POST /export` — full JSON export of event records |
| **Erasure** | Subject to the organization's audit trail retention obligations |

---

## Summary

- GitGov captures metadata, not code.
- Signals flag policy deviations — they are advisory inputs, not conclusions.
- The deploying organization is the data controller and bears full responsibility for how signals are used.
- Always apply human judgment before acting on a signal.
- Review your jurisdiction's employee monitoring requirements before deployment.

## Related

- [**Privacy Policy**](/privacy) — Legal terms for end-users and organizations.
- [**Governance & Policies**](/docs/governance) — How to configure `gitgov.toml`.
- [**Connect to the Control Plane**](/docs/control-plane) — Authentication and data flow architecture.
