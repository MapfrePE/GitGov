---
title: Installing GitGov Desktop
description: Deploy the GitGov capture agent to your local environment and begin tracking engineering operations.
order: 2
---

The GitGov Desktop application is the foundational capture agent for the ecosystem. It runs discreetly in the background, monitoring your local Git directories and providing instantaneous feedback on policy compliance.

## System Prerequisites

Before proceeding with the installation, ensure your workstation meets the following technical requirements:

- **Operating System**: Windows 10 or 11 (64-bit).
- **Git Version**: 2.30 or higher (must be in system PATH).
- **Permissions**: Administrative rights for initial setup and background service installation.
- **Memory**: Minimum 2GB RAM (GitGov uses ~50MB when idle).

---

## Acquisition & Deployment

### 1. Retrieve Installer
Navigate to the [GitGov Download Portal](/download) and select the latest `.exe` package for Windows. 

### 2. Execute Setup
Double-click the downloaded binary. 

> [!IMPORTANT]
> **Security Notice**: During the early access/development phase, the installer may trigger Windows SmartScreen. Click **"More Info"** followed by **"Run Anyway"** to proceed. We are currently working on global EV signing certificates.

### 3. Installation Wizard
Follow the on-screen prompts. GitGov defaults to installing in `%LOCALAPPDATA%/Programs/gitgov`. It is recommended to keep this default path to ensure automatic updates function correctly.

---

## Post-Installation Setup

Once installed, GitGov will launch automatically. Complete the following three steps to initialize the capture agent:

### A. Environment Discovery
The app will attempt to locate your primary `git.exe` and any SSH identities. If Git is installed in a non-standard location, you can manually override this in **Settings > Advanced**.

### B. Control Plane Handshake
You must provide the URL of your organization's Control Plane. If you are testing locally, the default is:
`http://127.0.0.1:3000`

### C. Workspace Indexing
GitGov will request permission to search your drive for Git repositories. You can choose to:
- **Auto-Detect**: Scans common development folders (e.g., `C:/Users/PC/Desktop`, `C:/GitHub`).
- **Manual Select**: Specify individual directories to track.

---

## Operational Verification

To confirm the capture agent is functioning correctly, perform a "Canary Push":

1. Open your terminal of choice (PS, Git Bash, CMD).
2. Navigate to a tracked repository.
3. Create a commit: `git commit -m "chore: test capture"`
4. Switch to the GitGov Desktop UI. You should see a new entry in the **Live Events** feed within milliseconds.

## Troubleshooting Common Issues

| Issue | Potential Cause | Resolution |
|-------|-----------------|------------|
| "Git not found" | Git not in system PATH | Verify `git --version` works in CMD. Adjust PATH vars if necessary. |
| Connection Timeout | VPN or Firewall interference | Ensure port 3000 (or your CP port) is whitelisted for inbound/outbound. |
| High CPU usage | Aggressive drive scanning | Exclude node_modules or large build folders in **Settings > Ignored Paths**. |

## Next Phase

- [**Connect to the Control Plane**](/docs/control-plane) — Finalize the synchronization layer.
