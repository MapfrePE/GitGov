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

### Cryptographic Identity
- **Signed Commits**: Enforce the use of GPG or SSH-signed commits to ensure author authenticity.
- **Verified Workstations**: Only allow pushes from devices registered and authenticated with the Control Plane.

---

## Defining a Policy (Example)

Policies are defined in the Control Plane via YAML or the Management UI. Here is an example of a "Production Ready" policy:

```yaml
name: "Standard Security Policy"
target: "branches/main, branches/release/*"
rules:
  - id: "signed_commits"
    enforcement: "block"
  - id: "commit_message_format"
    pattern: "^\[GITGOV-\d+\]: .+"
    enforcement: "block"
  - id: "max_diff_size"
    limit: "500 lines"
    enforcement: "advisory"
```

> [!NOTE]
> **Policy Inheritance**: Sub-teams can define their own specific policies that inherit from and extend the global organization-wide governance rules.

---

## Best Practices for Rollout

1. **Phase 1 (Observation)**: Deploy the Desktop app with all policies in **Advisory Mode**. Use the Control Plane dashboard to identify frequent violations.
2. **Phase 2 (targeted Enforcement)**: Switch high-risk rules (like commit signing) to **Enforcement Mode**.
3. **Phase 3 (Full Governance)**: Apply full enforcement across all critical repositories.

## Next Phase

- [**CI/CD Traceability**](/docs/ci-traces) — Bridge the gap between commits and deployments.
