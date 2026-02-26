---
title: Connect to the Control Plane
description: Configure the synchronization layer between your local capture agent and the central governance server.
order: 3
---

The Control Plane is the heart of the GitGov ecosystem. It acts as a secure, centralized ingestion point for events captured by all Desktop agents across your organization. Once connected, it enables real-time monitoring, audit logging, and global policy enforcement.

---

## Connection Fundamentals

GitGov Desktop communicates with the Control Plane via a high-performance REST API. For production environments, this connection is typically secured via TLS (HTTPS) and a rolling API Token.

### Standard Endpoint
By default, during development or local evaluation, the Control Plane listens on:
`http://127.0.0.1:3000`

---

## Configuration Workflow

Follow these steps to establish a secure link:

### 1. Verification of Server State
Ensure the Control Plane service is active. If you are running the server manually:
1. Open a terminal in the `gitgov-server` directory.
2. Verify Rust is initialized.
3. Start the service: `cargo run --release`.

### 2. Desktop Authentication
1. Launch **GitGov Desktop**.
2. Navigate to **Settings > Sync & Control Plane**.
3. Input the **Server URL** provided by your DevOps team.
4. Input your **Security Token** (if required by your organization).

### 3. Connection Handshake
Click the **"Test Connection"** button. GitGov will perform a lightweight health check to verify latency and protocol compatibility.

---

## Advanced Sync Settings

You can fine-tune how data is pushed to the server to balance between real-time visibility and network overhead.

| Setting | Recommendation | Description |
|---------|----------------|-------------|
| **Sync Interval** | 5s - 15s | Frequency of data pushes. Lower for high-security environments. |
| **Max Batch Size** | 100 events | Prevents large pushes from saturating local bandwidth. |
| **Offline Buffer** | Enabled | Stores events locally if the server is unreachable. |
| **Retry Logic** | Exponential | Automatically retries failed pushes with increasing delays. |

> [!IMPORTANT]
> **Data Privacy**: GitGov only syncs metadata (hashes, timestamps, branch names, and diff summaries). Your actual source code (the file contents) **never** leaves your workstation unless specifically configured for deep security auditing.

---

## Network Requirements

To ensure stable synchronization, your network environment must allow:
- **Protocol**: HTTP/1.1 or HTTP/2.
- **Port**: Default 3000 (Customizable in `config.toml`).
- **Domain Whitelisting**: Ensure your local firewall allows outbound traffic to the Control Plane domain.

## Next Phase

- [**Configure Governance Policies**](/docs/governance) — Learn how to set the rules of the road.
