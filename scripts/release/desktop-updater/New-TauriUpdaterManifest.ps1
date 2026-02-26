param(
  [Parameter(Mandatory = $true)]
  [string]$Version,

  [Parameter(Mandatory = $true)]
  [string]$Url,

  [Parameter(Mandatory = $true)]
  [string]$Signature,

  [Parameter(Mandatory = $false)]
  [string]$Notes = "",

  [Parameter(Mandatory = $false)]
  [string]$Platform = "windows-x86_64",

  [Parameter(Mandatory = $false)]
  [string]$PubDateUtc,

  [Parameter(Mandatory = $false)]
  [string]$OutputPath = "latest.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $PubDateUtc) {
  $PubDateUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
}

$manifest = [ordered]@{
  version = $Version
  notes = $Notes
  pub_date = $PubDateUtc
  platforms = [ordered]@{
    $Platform = [ordered]@{
      signature = $Signature
      url = $Url
    }
  }
}

$dir = Split-Path -Parent $OutputPath
if ($dir) {
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

$manifest | ConvertTo-Json -Depth 10 | Set-Content -Path $OutputPath -Encoding UTF8

Write-Host "Updater manifest generado:" -ForegroundColor Green
Write-Host "  $OutputPath"

