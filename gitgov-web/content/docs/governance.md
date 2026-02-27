---
title: Governance & Policies
description: Define and enforce computational policies across your entire software supply chain.
order: 4
---

GitGov transforms Git from a simple storage tool into a **governed platform**. By defining policies at the Control Plane, you can ensure that every developer in your organization follows identical standards for security, quality, and traceability.

---

## Policy Operational Modes

GitGov policies operate in two distinct modes, allowing for a progressive rollout within your organization:

### 1. Advisory Mode (Non-Blocking)
The capture agent monitors developer actions and provides real-time feedback in the Desktop UI without blocking Git commands. This is ideal for teaching best practices and gathering baseline compliance metrics.

### 2. Enforcement Mode (Blocking)
Operations that violate a defined policy are actively blocked at the workstation. Developers receive a detailed explanation of the violation and instructions on how to remediate it.

---

## Core Governance Domains

### Commit Message Standards
Ensure every commit is linked to a purpose.
- **Regex Enforcement**: Force messages to follow patterns like `[JIRA-123]: Short description`.
- **Length Constraints**: Prevent cryptic or overly verbose descriptions.
- **Keyword Requirements**: Mandate the presence of critical keywords (e.g., `fix`, `feat`, `chore`).

### Branch Naming Conventions
Maintain a clean and searchable repository structure.
- **Prefix Requirements**: `feature/*`, `bugfix/*`, `hotfix/*`.
- **Owner Tags**: Include developer or team identifiers in branch names.

---

## Defining a Policy (Example)

Policies are stored per-repository in a `gitgov.toml` file. Here is an example policy for a production branch:

```toml
[policy]
name = "Standard Security Policy"
target_branches = ["main", "release/*"]

[[policy.rules]]
id = "commit_message_format"
pattern = "^(feat|fix|refactor|docs|test|chore): .+"
enforcement = "advisory"

[[policy.rules]]
id = "branch_naming"
pattern = "^(feat|fix|hotfix|release)/.+"
enforcement = "advisory"

[[policy.rules]]
id = "max_diff_size"
limit_lines = 500
enforcement = "advisory"
```

> [!NOTE]
> **Advisory-first**: All rules currently operate in advisory mode. Enforcement mode (blocking at the workstation) is on the roadmap. Use advisory mode now to collect baseline compliance metrics before tightening policy.

---

## Best Practices for Rollout

1. **Phase 1 (Observation)**: Deploy the Desktop app with all policies in **Advisory Mode**. Use the Control Plane dashboard to identify frequent violations.
2. **Phase 2 (targeted Enforcement)**: Switch high-risk rules (like commit signing) to **Enforcement Mode**.
3. **Phase 3 (Full Governance)**: Apply full enforcement across all critical repositories.

## Next Phase

- [**CI/CD Traceability**](/docs/ci-traces) — Bridge the gap between commits and deployments.
