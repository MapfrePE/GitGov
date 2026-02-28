---
title: Connect to the Control Plane
description: Configure the synchronization layer between your local capture agent and the central governance server.
order: 3
---

The Control Plane is the heart of the GitGov ecosystem. It acts as a secure, centralized ingestion point for events captured by all Desktop agents across your organization. Once connected, it enables real-time monitoring, audit logging, and global policy enforcement.

---

## Authentication

GitGov Desktop communicates with the Control Plane via a REST API authenticated with a **Bearer token**. The token is derived from an API key that your administrator generates via the Control Plane's `/api-keys` endpoint.

```
Authorization: Bearer <api-key>
```

> [!IMPORTANT]
> Always use the `Authorization: Bearer` header format. The `X-API-Key` header is **not** supported and will result in a `401 Unauthorized` response.

---

## Connection Fundamentals

GitGov Desktop communicates via a high-performance REST API. For production environments, this connection should be secured with TLS (HTTPS).

The Desktop app connects automatically on launch using the server URL configured by your administrator. Your DevOps team will provide the correct server address for your organization.

---

## Configuration Workflow

### 1. Verify Server State
Ensure the Control Plane service is active and reachable. Your administrator can confirm via the health endpoint:

```bash
curl http://your-control-plane/health
# Expected: {"status":"ok", ...}
```

### 2. Desktop Authentication
1. Launch **GitGov Desktop**.
2. Navigate to **Settings > Sync & Control Plane**.
3. Enter the **Server URL** provided by your DevOps team.
4. Enter your **API Token**. The app will verify the connection immediately.

### 3. Connection Handshake
GitGov performs a lightweight health check to verify latency and protocol compatibility. A green status indicator confirms a successful connection.

---

## Sync Behavior

| Behaviour | Details |
|-----------|---------|
| **Event Dispatch** | Events are dispatched to the Control Plane as they occur, in batches via the `/events` endpoint. |
| **Dashboard Refresh** | The Control Plane dashboard auto-refreshes every **30 seconds**. |
| **Offline Buffer** | When the server is unreachable, the local outbox queues events in a JSONL file on disk. |
| **Retry Logic** | Failed dispatches use exponential backoff, capped at **32×** the base interval. Events are never lost. |
| **Rate Limit** | Default: **240 events/minute** per API key. Configurable via `GITGOV_RATE_LIMIT_EVENTS_PER_MIN`. |

---

## Role-Based Access

The Control Plane enforces role-based access on all authenticated endpoints:

| Role | Access |
|------|--------|
| **Admin** | Full access — stats, dashboard, integrations, policy management, all events |
| **Developer** | Scoped access — only sees their own events on `/logs` |
| **Architect** | Reserved for future role restrictions |
| **PM** | Reserved for future role restrictions |

API keys carry a role assignment. Ensure your developers are issued keys with the `Developer` role, and your DevOps/security team with the `Admin` role.

---

## Data Privacy

GitGov only syncs metadata: event type, commit SHA, branch name, author login, timestamp, and file counts. **Source code content never leaves the developer workstation.** Diff contents and file contents are not transmitted.

---

## Network Requirements

- **Protocol**: HTTP/1.1 or HTTP/2.
- **Port**: Default `3000` (configurable via `GITGOV_SERVER_ADDR` on the server side).
- **Firewall**: Allow outbound traffic from developer workstations to the Control Plane host on the configured port.
- **Production**: TLS (HTTPS) is strongly recommended. HTTP is supported for local evaluation only.

## Next Phase

- [**Configure Governance Policies**](/docs/governance) — Learn how to set the rules of the road.
