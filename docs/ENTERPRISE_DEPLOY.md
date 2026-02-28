# GitGov — Enterprise Deployment Guide

## Prerequisites

- Network access to GitGov Control Plane server (HTTP/HTTPS on configured port, default 3000)
- API key issued by your GitGov admin
- Platform-specific runtime requirements:
  - Windows 10/11 x64 (+ .NET Framework 4.7.2+)
  - macOS 12+ (Apple Silicon / Intel)
  - Linux x64 (glibc-based distro for AppImage/DEB)

---

## 1. Installer Options

GitGov ships two Windows installer formats per release:

| Format | File | Use case |
|--------|------|----------|
| NSIS (`.exe`) | `GitGov_x.x.x_x64-setup.exe` | Silent install via GPO / Intune / SCCM |
| MSI (`.msi`) | `GitGov_x.x.x_x64_en-US.msi` | Group Policy Software Installation |

Both installers are code-signed. Verify SHA256 hashes from the release page before deploying.

---

## 2. Silent Installation (NSIS)

The NSIS installer supports fully unattended deployment:

```
GitGov_x.x.x_x64-setup.exe /S /D=C:\Program Files\GitGov
```

| Flag | Description |
|------|-------------|
| `/S` | Silent mode — no UI |
| `/D=<path>` | Installation directory (must be last argument, no quotes) |

**Uninstall silently:**
```
"C:\Program Files\GitGov\Uninstall GitGov.exe" /S
```

---

## 3. MSI Deployment via Group Policy

```
msiexec /i GitGov_x.x.x_x64_en-US.msi /quiet /norestart INSTALLDIR="C:\Program Files\GitGov"
```

Assign to a GPO Computer Configuration > Software Settings > Software Installation.

---

## 4. Microsoft Intune Deployment

**Win32 App packaging:**

1. Download `IntuneWinAppUtil.exe` from Microsoft
2. Package the NSIS installer:
   ```
   IntuneWinAppUtil.exe -c . -s GitGov_x.x.x_x64-setup.exe -o ./output
   ```
3. In Intune > Apps > Windows > Add Win32:
   - **Install command:** `GitGov_x.x.x_x64-setup.exe /S`
   - **Uninstall command:** `"C:\Program Files\GitGov\Uninstall GitGov.exe" /S`
   - **Detection rule:** File exists `C:\Program Files\GitGov\GitGov.exe`
   - **Return codes:** 0 = success, 1641 = success (reboot initiated), 3010 = success (reboot required)

---

## 5. Pre-configuring Server Connection via Environment Variables

Set these machine-wide environment variables before or during installation so the app connects to your Control Plane automatically on first launch:

| Variable | Example | Description |
|----------|---------|-------------|
| `GITGOV_SERVER_URL` | `http://192.168.1.50:3000` | URL of the GitGov Control Plane server |
| `GITGOV_API_KEY` | `57f1ed59-371d-46ef-...` | API key provisioned by admin |

**Via Group Policy (Machine environment variables):**
```
Computer Configuration > Preferences > Windows Settings > Environment
```

**Via PowerShell (for Intune remediation scripts):**
```powershell
[System.Environment]::SetEnvironmentVariable("GITGOV_SERVER_URL", "http://192.168.1.50:3000", "Machine")
[System.Environment]::SetEnvironmentVariable("GITGOV_API_KEY", "your-api-key-here", "Machine")
```

**Via SCCM Task Sequence:**
```
cmd.exe /c setx GITGOV_SERVER_URL "http://192.168.1.50:3000" /M
cmd.exe /c setx GITGOV_API_KEY "your-api-key-here" /M
```

> **Note:** The app also reads from a local `.env` file at `%APPDATA%\..\Local\gitgov\.env` as a fallback. This is useful for per-user configuration or development environments.

---

## 6. Verifying Installation

After deployment, verify using PowerShell:

```powershell
# Check binary exists
Test-Path "C:\Program Files\GitGov\GitGov.exe"

# Check version
(Get-Item "C:\Program Files\GitGov\GitGov.exe").VersionInfo.ProductVersion

# Check env vars are set
[System.Environment]::GetEnvironmentVariable("GITGOV_SERVER_URL", "Machine")
```

---

## 7. SHA256 Hash Verification

Before deploying to machines, verify the installer hash matches the published release:

```powershell
# Windows PowerShell
Get-FileHash .\GitGov_x.x.x_x64-setup.exe -Algorithm SHA256
```

Compare with the `.sha256` file published alongside each release in GitHub Releases.

### Generating a .sha256 file (release managers)

After every build, generate the hash file using the included script:

```powershell
# From the repo root
.\scripts\generate_sha256.ps1 -InstallerPath ".\gitgov\src-tauri\target\release\bundle\nsis\GitGov_x.x.x_x64-setup.exe"
```

This writes `GitGov_x.x.x_x64-setup.exe.sha256` next to the installer and prints the hash.
Upload both the `.exe` and the `.sha256` file as GitHub Release assets.

Then set the hash as an environment variable in Vercel before redeploying the web app:
```
NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM=sha256:<hex>
```

The download page at https://git-gov.vercel.app/download will display the checksum automatically.

---

## 8. Code Signing

GitGov installers are signed with an EV Code Signing certificate. Windows SmartScreen and enterprise EDR solutions will recognize the signature automatically.

To verify the signature manually:
```powershell
Get-AuthenticodeSignature .\GitGov_x.x.x_x64-setup.exe | Select-Object -Property Status, SignerCertificate
```

Expected output: `Status: Valid`

### CI secrets required for signed Windows releases

The `build-signed.yml` workflow enforces signature validity and fails if installers are not properly signed.

Required GitHub Actions secrets:

| Secret | Description |
|--------|-------------|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` certificate blob |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for the `.pfx` certificate |
| `WINDOWS_CERTIFICATE_THUMBPRINT` | Thumbprint of the cert to use for Tauri Windows signing |
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater signing private key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for updater signing private key |

If any of these are missing or invalid, release jobs on `v*` tags fail by design.

### CI for all three platforms (Windows + macOS + Linux)

The `build-signed.yml` workflow builds all desktop targets on git tag pushes (`v*`):

- Windows: NSIS (`.exe`) + MSI (`.msi`) + `.sha256`
- macOS: DMG (`.dmg`) + `.sha256`
- Linux: AppImage (`.AppImage`) + DEB (`.deb`) + `.sha256`

Notes:

- Windows Authenticode signing requires `WINDOWS_CERTIFICATE*` secrets.
- Tauri updater signing (`TAURI_SIGNING_PRIVATE_KEY*`) should be set for all platforms.
- macOS notarization is not yet wired in this repository; for production distribution via Gatekeeper, add Apple Developer signing/notarization secrets and notarization steps.

### Local signed build (Windows)

For local/rehearsal signed builds outside CI, use:

```powershell
.\scripts\build_signed_windows.ps1 -RepoRoot . -PfxPath "C:\secrets\gitgov-codesign.pfx" -PfxPassword "<password>"
```

Options:

- `-Thumbprint "<thumbprint>"`: use an already-installed cert from `Cert:\CurrentUser\My` without importing PFX.
- `-PfxBase64 "<base64>"`: import a base64-encoded PFX blob (same format as CI secret).

The script:

1. Imports/selects the code-signing cert
2. Injects `certificateThumbprint` into `src-tauri/tauri.conf.json` temporarily
3. Runs `npm run tauri build`
4. Verifies Authenticode status (`Valid`) for MSI/NSIS
5. Emits `.sha256` files next to installers
6. Restores `tauri.conf.json`

---

## 9. Firewall / Proxy Requirements

GitGov Desktop only makes outbound connections:

| Destination | Port | Protocol | Purpose |
|-------------|------|----------|---------|
| Control Plane server | 3000 (or configured) | HTTP/HTTPS | Event ingestion + dashboard |
| `downloads.gitgov.com` | 443 | HTTPS | Auto-update checks (optional) |

If using a proxy, set the standard `HTTP_PROXY` / `HTTPS_PROXY` environment variables.

---

## 10. Automatic Updates

GitGov checks for updates at launch from:
```
https://downloads.gitgov.com/desktop/stable/latest.json
```

To disable auto-updates in an air-gapped environment, block the `downloads.gitgov.com` domain at the firewall level. The app will continue to function normally; only the update notification will be suppressed.

To host your own update server, see `docs/DESKTOP_UPDATES.md`.

---

## 11. Offboarding a Developer

When a developer leaves the organization:

1. **Revoke their API key** from the GitGov dashboard (Admin → API Key Manager → Revoke)
   Revocation takes effect immediately — the developer's Desktop app will receive 401 errors on next sync.

2. **Uninstall the app** via Intune/SCCM/GPO using the silent uninstall command above.

3. The audit history remains intact and immutable in the Control Plane database. All events logged under the developer's `client_id` are permanently retained for compliance purposes.

---

## 12. Compliance Export

Admins can export the full audit history from the GitGov dashboard:

1. Open **Control Plane** tab in GitGov Desktop
2. Connect to your server with an Admin API key
3. Scroll to **Export Historial de Auditoría**
4. Select date range and click **Exportar JSON**
5. The exported file is saved locally in JSON format with all event metadata

The export also creates an immutable log entry in the server (`export_logs` table) recording who exported, when, and how many records.

---

## 13. Support

- Documentation: `docs/` directory in the GitGov repository
- Issues: https://github.com/MapfrePE/GitGov/issues
- Control Plane health check: `GET http://<server>:3000/health`
