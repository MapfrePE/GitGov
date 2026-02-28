---
title: Installing GitGov Desktop
description: Deploy the GitGov capture agent to your local environment and begin tracking engineering operations.
order: 2
---

The GitGov Desktop application is the foundational capture agent for the ecosystem. It runs in the background, monitoring your active Git operations and providing real-time feedback on policy compliance.

## System Prerequisites

Before proceeding with the installation, ensure your workstation meets the following technical requirements:

- **Operating System**: Windows 10 or 11 (64-bit).
- **Git Version**: 2.30 or higher (must be in system PATH).
- **Permissions**: Administrative rights for initial setup.
- **Memory**: Minimum 2 GB RAM (GitGov uses ~50 MB when idle).

---

## Acquisition & Deployment

### 1. Retrieve Installer
Navigate to the [GitGov Download Portal](/download) and select the latest `.exe` package for Windows.

### 2. Execute Setup
Double-click the downloaded binary.

> [!IMPORTANT]
> **Security Notice**: During the early access phase, the installer may trigger Windows SmartScreen. Click **"More Info"** followed by **"Run Anyway"** to proceed. Code signing certificates are in progress.

### 3. Installation Wizard
Follow the on-screen prompts. GitGov defaults to installing in `%LOCALAPPDATA%\Programs\GitGov`. Keep this default path to ensure automatic updates function correctly.

---

## Post-Installation Setup

Once installed, GitGov will launch automatically. Complete the following steps to initialize the capture agent:

### A. Git Detection
The app locates your `git.exe` from the system PATH. If Git is installed but not detected, verify that `git --version` works in a CMD or PowerShell terminal and that Git is present in your `PATH` environment variable.

### B. Control Plane Connection
Provide the URL of your organization's Control Plane server. Your DevOps team will supply this address. The app connects automatically on launch — no manual action required once the URL is saved.

---

## What GitGov Captures

Once connected, GitGov captures the following Git events automatically:

| Event | Trigger |
|-------|---------|
| `stage_files` | Files added to the Git index (`git add`) |
| `commit` | A new commit is created (includes SHA, message, author, branch) |
| `attempt_push` | A push is initiated |
| `successful_push` | Push completes successfully |
| `blocked_push` | Push rejected by branch protection policy |
| `push_failed` | Push fails (network, remote rejection, etc.) |

> **Note**: For repositories with a very large number of staged files, the `stage_files` event captures a maximum of 500 file entries. A `truncated` flag is set in the event metadata when this limit is reached.

---

## Operational Verification

To confirm the capture agent is functioning correctly, perform a "Canary Push":

1. Open a terminal (PowerShell, Git Bash, or CMD).
2. Navigate to a Git repository.
3. Stage and commit a change: `git add . && git commit -m "chore: test capture"`
4. Switch to the GitGov Desktop UI. You should see `stage_files` and `commit` events appear in the **Live Events** feed within milliseconds.

---

## Troubleshooting Common Issues

| Issue | Potential Cause | Resolution |
|-------|-----------------|------------|
| "Git not found" | Git not in system PATH | Verify `git --version` works in CMD/PS. Adjust `PATH` if needed. |
| Connection timeout | Firewall or VPN blocking port 3000 | Ensure outbound traffic to the Control Plane host on port 3000 is allowed. |
| Events not appearing | Control Plane not running | Verify the server is active. Check the connection status indicator in the Desktop app. |
| SmartScreen warning | No code signing certificate | Click "More Info" → "Run Anyway". This is expected during early access. |

## Next Phase

- [**Connect to the Control Plane**](/docs/control-plane) — Finalize the synchronization layer.
