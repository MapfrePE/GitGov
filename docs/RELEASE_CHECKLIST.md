# GitGov Release Checklist

Use this checklist for every public release. Complete every step in order before announcing.

---

## 1. Build

- [ ] Pull latest `main` and confirm CI is green
- [ ] Bump version in `gitgov/src-tauri/tauri.conf.json` (`version` field)
- [ ] Bump `version` and `downloadFileName` in `gitgov-web/lib/config/site.ts`
- [ ] Run local build:
  ```
  cd gitgov && npm run tauri build
  ```
- [ ] Confirm build artifacts exist:
  - `src-tauri/target/release/bundle/nsis/GitGov_X.X.X_x64-setup.exe`
  - `src-tauri/target/release/bundle/msi/GitGov_X.X.X_x64_en-US.msi`

---

## 2. Generate & Record SHA256 Hash

- [ ] Run the hash generation script from the repo root:
  ```powershell
  .\scripts\generate_sha256.ps1 -InstallerPath ".\gitgov\src-tauri\target\release\bundle\nsis\GitGov_X.X.X_x64-setup.exe"
  ```
- [ ] Confirm `.sha256` file was created next to the installer
- [ ] Copy the `sha256:<hex>` value — you will need it in step 5

---

## 3. Upload Release Assets to GitHub Releases

- [ ] Create a new GitHub Release at https://github.com/MapfrePE/GitGov/releases/new
  - Tag: `vX.X.X`
  - Title: `GitGov vX.X.X`
  - Body: changelog summary
- [ ] Upload the following assets:
  - `GitGov_X.X.X_x64-setup.exe` (NSIS installer)
  - `GitGov_X.X.X_x64-setup.exe.sha256` (SHA256 hash file)
  - `GitGov_X.X.X_x64_en-US.msi` (MSI installer, optional)
  - `GitGov_X.X.X_x64_en-US.msi.sha256` (MSI hash, optional)
- [ ] Copy the direct download URL for the `.exe` asset

---

## 4. Update Vercel Environment Variables

In the Vercel dashboard for `git-gov.vercel.app`, update:

| Variable | Value |
|----------|-------|
| `NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL` | Direct URL to `.exe` on GitHub Releases |
| `NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM` | `sha256:<hex>` from step 2 |
| `NEXT_PUBLIC_DESKTOP_DOWNLOAD_MSI_URL` | Direct URL to `.msi` (optional, leave blank if not used) |

- [ ] Variables updated in Vercel dashboard
- [ ] Trigger a new Vercel deployment (re-deploy latest commit)

---

## 5. Smoke Test the Download Page

- [ ] Open https://git-gov.vercel.app/download
- [ ] Confirm the download button links to the new GitHub Releases URL
- [ ] Confirm the checksum shown matches the `.sha256` file
- [ ] Click the download button and verify the file downloads correctly
- [ ] Verify SHA256 locally:
  ```powershell
  Get-FileHash .\GitGov_X.X.X_x64-setup.exe -Algorithm SHA256
  ```
- [ ] Check the API endpoint: `https://git-gov.vercel.app/api/release-metadata`
  - Confirm `version`, `downloadUrl`, `checksum` are correct
- [ ] Run the e2e smoke test (with the server running locally):
  ```
  NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL=<url> node gitgov-web/tests/e2e/download-url.mjs
  ```

---

## 6. Post-Release

- [ ] Update `docs/PROGRESS.md` with release notes
- [ ] Announce in team channel
- [ ] Tag the commit in git: `git tag vX.X.X && git push origin vX.X.X`

---

## Risk Notes

- **SmartScreen warning**: Until Authenticode signing is implemented, users will see a SmartScreen prompt. The download page includes a neutral notice explaining how to proceed.
- **Checksum mismatch**: If `NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM` is not updated in Vercel, the page will show `sha256:pending-build`. Always update this variable before redeploying.
