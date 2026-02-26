---
title: Introduction to GitGov
description: Master distributed Git governance and establish full operational traceability across your development lifecycle.
order: 1
---

GitGov is an **Enterprise-Grade Distributed Git Governance System**. It is built specifically for security-conscious engineering teams that require immutable operational evidence, cryptographic traceability, and automated compliance enforcement across every commit, push, and deployment.

## The Problem: Fragmented Audit Trails

In modern engineering organizations, the "source of truth" is scattered across a dozen disconnected silos:
- **Git Repositories**: Where development happens.
- **CI/CD Pipelines**: (Jenkins, GitHub Actions) Where builds occur.
- **Ticket Systems**: (Jira) Where requirements are defined.
- **Developer Machines**: Where code is actually authored and manipulated.

When an audit occurs or a security incident is investigated, teams often struggle to answer: *"Who authorized this code to bypass the build server and land in production?"* Traditionally, you piece this together from disparate logs that might be incomplete or already rotated.

## The Solution: Source-Side Governance

GitGov flips the model. Instead of relying on central servers to guess what happened on a developer's machine, GitGov captures high-fidelity metadata **at the point of origin**.

By correlating local Git operations with upstream build results and ticket data, GitGov builds a **unified chain of custody** for every single byte of code in your organization.

---

## Core Pillars of the Platform

### 1. Immutable Operational Evidence
Every action (commit, rebase, merge, push) is recorded as a discrete, immutable event. These events are cryptographically hashed and synced to a central Control Plane, creating a tamper-evident audit trail.

### 2. Deep Traceability
GitGov doesn't just see a "commit." It sees a commit linked to a specific Jira ticket, validated by a specific Jenkins build, and pushed by a verified developer workstation. 

### 3. Progressive Policy Enforcement
- **Advisory Mode**: Warns developers about unconventional branch names or commit messages in real-time.
- **Enforcement Mode**: Blocks operations that don't meet organizational standards (e.g., unsigned commits or missing ticket IDs).

---

## Component Architecture

GitGov is composed of three mission-critical layers:

| Component | Responsibility | Technology Stack |
|-----------|----------------|------------------|
| **GitGov Desktop** | Local capture & real-time developer feedback | Tauri, Rust, React |
| **Control Plane** | Central event ingestion, storage, and reporting | Rust, Axum, PostgreSQL |
| **Integrations** | Correlating data from Jenkins, Jira, and GitHub | Webhooks & REST APIs |

> [!TIP]
> **Pro Tip**: You can run GitGov in "Silent Mode" during initial rollout to gather baseline compliance data without interrupting developer workflows.

## Navigation & Next Steps

Ready to get started? Follow the path below to secure your Git workflow:

1. [**Install GitGov Desktop**](/docs/installation) — Get the capture agent running on your machine.
2. [**Connect to the Control Plane**](/docs/control-plane) — Link your local instance to the central server.
3. [**Configure Policies**](/docs/governance) — Define the rules that keep your codebase clean.
