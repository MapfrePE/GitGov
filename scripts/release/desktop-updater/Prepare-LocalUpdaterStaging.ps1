param(
  [Parameter(Mandatory = $false)]
  [string]$Version = "0.1.1",

  [Parameter(Mandatory = $false)]
  [string]$Port = "3005",

  [Parameter(Mandatory = $false)]
  [string]$ListenHost = "127.0.0.1",

  [Parameter(Mandatory = $false)]
  [string]$RepoRoot = (Resolve-Path ".").Path,

  [Parameter(Mandatory = $false)]
  [string]$ExePath = "gitgov/src-tauri/target/release/bundle/nsis/GitGov_0.1.0_x64-setup.exe",

  [Parameter(Mandatory = $false)]
  [string]$PrivateKeyPath = "$env:USERPROFILE\.gitgov-secrets\desktop-updater\tauri-updater.key",

  [Parameter(Mandatory = $false)]
  [string]$PrivateKeyPassword = "",

  [Parameter(Mandatory = $false)]
  [switch]$StartServer
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRootResolved = (Resolve-Path $RepoRoot).Path
$exeFullPath = [System.IO.Path]::GetFullPath((Join-Path $repoRootResolved $ExePath))
$keyFullPath = [System.IO.Path]::GetFullPath($PrivateKeyPath)

if (-not (Test-Path $exeFullPath)) {
  throw "No existe instalador para staging local: $exeFullPath"
}
if (-not (Test-Path $keyFullPath)) {
  throw "No existe clave privada del updater: $keyFullPath"
}

$stagingDir = Join-Path $repoRootResolved "release\desktop\staging-local"
New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null

$exeName = Split-Path -Leaf $exeFullPath
$stagedExePath = Join-Path $stagingDir $exeName
$sigPath = "$stagedExePath.sig"
$manifestPath = Join-Path $stagingDir "latest.json"
$updateUrl = "http://$ListenHost`:$Port/$exeName"

Copy-Item -Force $exeFullPath $stagedExePath

$signCmd = @("tauri", "signer", "sign", "-f", $keyFullPath)
if (-not [string]::IsNullOrWhiteSpace($PrivateKeyPassword)) {
  $signCmd += @("-p", $PrivateKeyPassword)
}
$signCmd += $stagedExePath

Push-Location (Join-Path $repoRootResolved "gitgov")
try {
  $signature = (& npx @signCmd).Trim()
  if ([string]::IsNullOrWhiteSpace($signature)) {
    throw "No se obtuvo firma del comando tauri signer."
  }
}
finally {
  Pop-Location
}

$signature | Set-Content -Path $sigPath -Encoding ASCII

& (Join-Path $repoRootResolved "scripts\release\desktop-updater\New-TauriUpdaterManifest.ps1") `
  -Version $Version `
  -Url $updateUrl `
  -Signature $signature `
  -Notes "Local staging update simulation ($Version)" `
  -OutputPath $manifestPath | Out-Null

Write-Host "Staging local listo:" -ForegroundColor Green
Write-Host "  Carpeta: $stagingDir"
Write-Host "  Manifest: $manifestPath"
Write-Host "  URL manifest esperada: http://$ListenHost`:$Port/latest.json"
Write-Host ""
Write-Host "Para probar con Tauri (staging local):" -ForegroundColor Cyan
Write-Host "  cd gitgov"
Write-Host "  npm run tauri dev -- --config src-tauri/tauri.updater-staging.conf.json"

if ($StartServer) {
  Write-Host ""
  Write-Host "Iniciando servidor HTTP local en http://$ListenHost`:$Port ..." -ForegroundColor Yellow
  if (Get-Command python -ErrorAction SilentlyContinue) {
    python -m http.server $Port --bind $ListenHost --directory $stagingDir
  } elseif (Get-Command py -ErrorAction SilentlyContinue) {
    py -m http.server $Port --bind $ListenHost --directory $stagingDir
  } else {
    throw "No se encontró Python (python/py) para iniciar servidor local. Sirve la carpeta manualmente: $stagingDir"
  }
}
