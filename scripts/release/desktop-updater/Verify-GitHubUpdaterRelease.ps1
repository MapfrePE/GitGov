param(
  [string]$LatestJsonUrl = "https://github.com/MapfrePE/GitGov/releases/latest/download/latest.json",
  [string]$RepoRoot = "."
)

$ErrorActionPreference = "Stop"

function Fail([string]$Message) {
  Write-Host "[FAIL] $Message" -ForegroundColor Red
  exit 1
}

function Ok([string]$Message) {
  Write-Host "[OK]   $Message" -ForegroundColor Green
}

function Info([string]$Message) {
  Write-Host "[INFO] $Message" -ForegroundColor Cyan
}

try {
  $repoPath = (Resolve-Path $RepoRoot).Path
} catch {
  Fail "No se pudo resolver RepoRoot: $RepoRoot"
}

$tauriConfigPath = Join-Path $repoPath "gitgov\src-tauri\tauri.conf.json"
$pubKeyPath = Join-Path $repoPath "secrets\tauri-updater.key.pub"

if (-not (Test-Path $tauriConfigPath)) {
  Fail "No existe tauri.conf.json: $tauriConfigPath"
}
if (-not (Test-Path $pubKeyPath)) {
  Fail "No existe clave publica local: $pubKeyPath"
}

# 1) Cargar pubkey del repo (base64 string)
$tauriCfg = Get-Content $tauriConfigPath -Raw | ConvertFrom-Json
$cfgPubB64 = [string]$tauriCfg.plugins.updater.pubkey
if ([string]::IsNullOrWhiteSpace($cfgPubB64)) {
  Fail "tauri.conf.json no tiene plugins.updater.pubkey"
}

# 2) Cargar clave publica local y compararla con la embebida
$pubRaw = (Get-Content $pubKeyPath -Raw).Trim()
$pubB64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($pubRaw))
if ($pubB64 -ne $cfgPubB64) {
  Fail "Mismatch pubkey: la clave en tauri.conf.json no coincide con secrets\\tauri-updater.key.pub"
}
Ok "Pubkey del repo coincide con secrets/tauri-updater.key.pub"

# 3) Verificar latest.json remoto (status 200 + JSON válido)
$resp = Invoke-WebRequest -Uri $LatestJsonUrl -MaximumRedirection 10 -UseBasicParsing
if ([int]$resp.StatusCode -ne 200) {
  Fail "latest.json no responde 200 (status=$($resp.StatusCode))"
}
Ok "latest.json responde 200"

# Manejar BOM/byte array en Windows PowerShell 5.1
$content = $resp.Content
if ($content -is [byte[]]) {
  $jsonText = [Text.Encoding]::UTF8.GetString($content)
} else {
  $jsonText = [string]$content
  # En PS 5.1, algunos responses llegan como lista textual de bytes (una línea por byte).
  if ($jsonText -notmatch '^\s*\{' -and $jsonText -match '^\s*\d+\s*$') {
    $byteLines = $jsonText -split "(`r`n|`n|`r)" | Where-Object { $_ -match '^\s*\d+\s*$' }
    if ($byteLines.Count -gt 0) {
      [byte[]]$bytes = @()
      foreach ($line in $byteLines) {
        $bytes += [byte]([int]$line.Trim())
      }
      $jsonText = [Text.Encoding]::UTF8.GetString($bytes)
    }
  }
}

try {
  # Strip UTF-8 BOM if present (PS 5.1 ConvertFrom-Json can choke on it)
  if ($jsonText.Length -gt 0 -and [int][char]$jsonText[0] -eq 65279) {
    $jsonText = $jsonText.Substring(1)
  }
  $manifest = $jsonText | ConvertFrom-Json
} catch {
  Fail "latest.json no es JSON válido"
}

# 4) Validar shape mínimo requerido por Tauri updater
$version = [string]$manifest.version
$platform = $manifest.platforms.'windows-x86_64'
if ([string]::IsNullOrWhiteSpace($version)) {
  Fail "latest.json no contiene version"
}
if (-not $platform) {
  Fail "latest.json no contiene platforms.windows-x86_64"
}
$url = [string]$platform.url
$signature = [string]$platform.signature
if ([string]::IsNullOrWhiteSpace($url)) {
  Fail "latest.json no contiene url para windows-x86_64"
}
if ([string]::IsNullOrWhiteSpace($signature)) {
  Fail "latest.json no contiene signature para windows-x86_64"
}
Ok "Manifest contiene version/url/signature para windows-x86_64"

# 5) Verificar que el asset URL exista (200 por redirección también válido)
$assetResp = Invoke-WebRequest -Uri $url -MaximumRedirection 10 -UseBasicParsing
if ([int]$assetResp.StatusCode -ne 200) {
  Fail "Asset URL no responde 200 (status=$($assetResp.StatusCode))"
}
Ok "Asset URL responde 200"

Info "Version publicada: $version"
Info "Asset URL: $url"
Info "Verificación completa: PASS"
