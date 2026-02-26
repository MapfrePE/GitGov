param(
  [Parameter(Mandatory = $true)]
  [string]$ExePath,

  [Parameter(Mandatory = $true)]
  [string]$SigPath,

  [Parameter(Mandatory = $true)]
  [string]$ManifestPath,

  [Parameter(Mandatory = $true)]
  [string]$Bucket,

  [Parameter(Mandatory = $false)]
  [string]$Channel = "stable",

  [Parameter(Mandatory = $false)]
  [string]$DesktopPrefix = "desktop",

  [Parameter(Mandatory = $false)]
  [string]$CloudFrontDistributionId
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

foreach ($path in @($ExePath, $SigPath, $ManifestPath)) {
  if (-not (Test-Path $path)) {
    throw "No existe el archivo requerido: $path"
  }
}

$exeName = Split-Path -Leaf $ExePath
$sigName = Split-Path -Leaf $SigPath
$manifestName = Split-Path -Leaf $ManifestPath

$baseKey = "$DesktopPrefix/$Channel"

Write-Host "Subiendo artefactos a s3://$Bucket/$baseKey/" -ForegroundColor Cyan

aws s3 cp $ExePath "s3://$Bucket/$baseKey/$exeName" --content-type "application/octet-stream"
aws s3 cp $SigPath "s3://$Bucket/$baseKey/$sigName" --content-type "text/plain"
aws s3 cp $ManifestPath "s3://$Bucket/$baseKey/$manifestName" --content-type "application/json"

Write-Host "Publicación completada en S3." -ForegroundColor Green

if ($CloudFrontDistributionId) {
  $paths = @(
    "/$baseKey/$manifestName",
    "/$baseKey/$exeName",
    "/$baseKey/$sigName"
  )
  Write-Host "Invalidando CloudFront ($CloudFrontDistributionId)..." -ForegroundColor Cyan
  aws cloudfront create-invalidation --distribution-id $CloudFrontDistributionId --paths $paths
  Write-Host "Invalidación enviada." -ForegroundColor Green
}

