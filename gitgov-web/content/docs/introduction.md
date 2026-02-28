---
title: Introduction to GitGov
description: Master distributed Git governance and establish full operational traceability across your development lifecycle.
order: 1
---

GitGov is an **Enterprise-Grade Distributed Git Governance System**. It is built specifically for security-conscious engineering teams that require immutable operational evidence, deep traceability, and automated compliance enforcement across every commit, push, and deployment.

## The Problem: Fragmented Audit Trails

In modern engineering organizations, the "source of truth" is scattered across a dozen disconnected silos:
- **Git Repositories**: Where development happens.
- **CI/CD Pipelines**: (Jenkins, GitHub Actions) Where builds occur.
- **Ticket Systems**: (Jira) Where requirements are defined.
- **Developer Machines**: Where code is actually authored and manipulated.

When an audit occurs or a security incident is investigated, teams often struggle to answer: *"Who authorized this code to bypass the build server and land in production?"* Traditionally, you piece this together from disparate logs that might be incomplete or already rotated.

## The Solution: Source-Side Governance

GitGov flips the model. Instead of relying on central servers to guess what happened on a developer's machine, GitGov captures high-fidelity metadata **at the point of origin** — the developer workstation.

By correlating local Git operations with upstream build results and ticket data, GitGov builds a **unified chain of custody** for every single byte of code in your organization.

---

## Core Pillars of the Platform

### 1. Immutable Operational Evidence
Every action — commit, push, stage, rebase, merge — is recorded as a discrete, append-only event. Events are deduplicated by a unique UUID and stored in a tamper-evident, append-only audit table. Records are never overwritten or deleted.

### 2. Deep Traceability
GitGov doesn't just see a "commit." It sees a commit linked to a specific Jira ticket, validated by a specific Jenkins build, pushed by a verified developer workstation — all correlated automatically by the Control Plane.

### 3. Progressive Policy Enforcement
- **Branch Protection**: Defined in `gitgov.toml`, prevents unauthorized direct pushes to protected branches (e.g., `main`, `release/*`).
- **Group-Based Access**: Restrict which teams can push to which branches and modify which code paths.
- **CI Advisory Checks**: The `/policy/check` endpoint lets Jenkins and other CI systems query compliance status before executing a build.

---

## Component Architecture

GitGov is composed of four mission-critical components:

| Component | Responsibility | Technology Stack |
|-----------|----------------|------------------|
| **GitGov Desktop** | Local Git event capture and real-time developer feedback | Tauri v2, Rust, React 19 |
| **Control Plane** | Central event ingestion, storage, reporting, and policy engine | Rust, Axum, PostgreSQL |
| **Integrations** | Correlating data from Jenkins, Jira, and GitHub | Webhooks & REST APIs |
| **Web App** | Documentation, marketing, and download portal | Next.js 14, React 18 |

---

## Navigation & Next Steps

Ready to get started? Follow the path below to secure your Git workflow:

1. [**Install GitGov Desktop**](/docs/installation) — Get the capture agent running on your machine.
2. [**Connect to the Control Plane**](/docs/control-plane) — Link your local instance to the central server.
3. [**Configure Policies**](/docs/governance) — Define the rules that keep your codebase clean.
4. [**CI/CD Traceability**](/docs/ci-traces) — Connect your Jenkins pipelines for full build provenance.
