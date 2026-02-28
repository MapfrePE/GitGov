---
title: Governance & Policies
description: Define access controls, branch protection, and group-based permissions across your repositories using gitgov.toml.
order: 4
---

GitGov transforms Git from a simple storage tool into a **governed platform**. By defining a `gitgov.toml` file per repository, you can enforce branch protection, group-based access control, and policy advisory checks across every developer workstation.

---

## Policy Operational Modes

GitGov governance operates at two levels:

### 1. Workstation-Level (gitgov.toml)
The `gitgov.toml` file in the repository root configures branch protection and group permissions. When a developer attempts a push to a protected branch without the required group membership, the operation is blocked and an event is logged.

### 2. CI Advisory Check (/policy/check)
The Control Plane exposes a `/policy/check` endpoint that your CI pipeline (e.g. Jenkins) can call before executing a build. It returns an advisory response — `allowed`, `reasons`, and `warnings` — so you can enforce governance at the pipeline level without blocking developer workstations.

---

## Configuring gitgov.toml

Policies are stored per-repository in a `gitgov.toml` file at the repository root. The file supports three configuration sections:

### [branches]
Defines recognized branch patterns and the list of protected branches that block direct pushes.

### [groups.*]
Defines named teams with their members, the branches they are allowed to push to, and the code paths they are authorized to modify.

### admins
A list of usernames with administrative access across all branches and paths.

---

## Configuration Example

```toml
# gitgov.toml — place at the repository root

[branches]
# Recognized naming conventions (informational — used for advisory checks)
patterns  = ["feat/*", "fix/*", "hotfix/*", "release/*", "docs/*"]
# Direct pushes to these branches are blocked for non-admins
protected = ["main", "release/*"]

[groups.backend]
members          = ["alice", "carlos"]
allowed_branches = ["feat/*", "fix/*", "hotfix/*"]
allowed_paths    = ["gitgov-server/", "src/"]

[groups.frontend]
members          = ["bob", "diana"]
allowed_branches = ["feat/*", "fix/*"]
allowed_paths    = ["gitgov/src/", "gitgov-web/"]

admins = ["admin-lead", "devops-ops"]
```

> **Note**: Policy enforcement blocks pushes to `protected` branches for developers not listed as `admins` or in a group with explicit `allowed_branches` access. All blocked operations are recorded as `blocked_push` events in the audit trail.

---

## CI Advisory Check

For Jenkins and other CI systems, the Control Plane provides a `/policy/check` endpoint that evaluates whether a given commit or operation is compliant:

```bash
curl -s -X POST https://your-control-plane/policy/check \
  -H "Authorization: Bearer $GITGOV_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "repo_name": "YourOrg/YourRepo",
    "commit_sha": "a3f8c2e",
    "branch": "main",
    "user_login": "alice"
  }'
```

The response includes:
- `advisory` — `true` if the check is non-blocking
- `allowed` — whether the operation passes current policy
- `reasons` — list of specific violations
- `warnings` — soft advisories (non-blocking)
- `evaluated_rules` — the rules applied to reach this decision

> **Current state**: The `/policy/check` endpoint operates in **advisory mode**. It informs CI pipelines about compliance status but does not automatically halt deployments. Hard enforcement at the CI level is on the roadmap.

---

## Best Practices for Rollout

1. **Phase 1 — Observation**: Deploy `gitgov.toml` with no `protected` branches. Review the advisory data from `/policy/check` to identify frequent violations.
2. **Phase 2 — Branch Protection**: Add critical branches to `protected`. Only admins and explicitly authorized groups can push directly.
3. **Phase 3 — Full Governance**: Apply group-based `allowed_paths` restrictions and integrate `/policy/check` into your Jenkins pipeline as a gating step.

## Next Phase

- [**CI/CD Traceability**](/docs/ci-traces) — Bridge the gap between commits and deployments.
