param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("stable", "beta")]
  [string]$Channel,

  [Parameter(Mandatory = $true)]
  [string]$BaseUrl,

  [Parameter(Mandatory = $true)]
  [string]$PubKey,

  [Parameter(Mandatory = $false)]
  [string]$OutputPath = ".\release\desktop\tauri.updater.$Channel.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($PubKey)) {
  throw "PubKey no puede estar vacío."
}

$normalizedBase = $BaseUrl.Trim().TrimEnd("/")
$endpoint = "$normalizedBase/$Channel/latest.json"

$snippet = [ordered]@{
  plugins = [ordered]@{
    updater = [ordered]@{
      endpoints = @($endpoint)
      pubkey = $PubKey.Trim()
    }
  }
}

$dir = Split-Path -Parent $OutputPath
if ($dir) {
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

$snippet | ConvertTo-Json -Depth 10 | Set-Content -Path $OutputPath -Encoding UTF8

Write-Host "Snippet de updater generado:" -ForegroundColor Green
Write-Host "  $OutputPath"
Write-Host "Endpoint:" -ForegroundColor Cyan
Write-Host "  $endpoint"
Write-Host ""
Write-Host "Copia el bloque 'plugins.updater' en gitgov/src-tauri/tauri.conf.json para ese canal." -ForegroundColor Yellow
