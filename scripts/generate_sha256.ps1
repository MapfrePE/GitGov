<#
.SYNOPSIS
    Computes the SHA256 hash of a GitGov installer and writes a .sha256 file next to it.

.DESCRIPTION
    Accepts a path to a .exe or .msi installer, computes SHA256 using Get-FileHash,
    writes the hash as "sha256:<hex>" into a <installer>.sha256 file in the same directory,
    and prints the hash to stdout.

    Use this script after every build before uploading to GitHub Releases.

.PARAMETER InstallerPath
    Full or relative path to the installer file (e.g. GitGov_0.1.0_x64-setup.exe).

.EXAMPLE
    .\scripts\generate_sha256.ps1 -InstallerPath ".\src-tauri\target\release\bundle\nsis\GitGov_0.1.0_x64-setup.exe"

.EXAMPLE
    .\scripts\generate_sha256.ps1 ".\src-tauri\target\release\bundle\msi\GitGov_0.1.0_x64_en-US.msi"
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$InstallerPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Resolve to absolute path
$resolved = Resolve-Path -Path $InstallerPath -ErrorAction Stop
$absolutePath = $resolved.Path

if (-not (Test-Path -LiteralPath $absolutePath -PathType Leaf)) {
    Write-Error "File not found: $absolutePath"
    exit 1
}

Write-Host "Computing SHA256 for: $absolutePath"

$hashResult = Get-FileHash -Path $absolutePath -Algorithm SHA256
$hex = $hashResult.Hash.ToLower()
$formattedHash = "sha256:$hex"

$sha256FilePath = "$absolutePath.sha256"
Set-Content -Path $sha256FilePath -Value $formattedHash -Encoding UTF8 -NoNewline

Write-Host ""
Write-Host "  Hash  : $formattedHash"
Write-Host "  Output: $sha256FilePath"
Write-Host ""
Write-Host "Done. Set NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM=$formattedHash in Vercel before deploying."
