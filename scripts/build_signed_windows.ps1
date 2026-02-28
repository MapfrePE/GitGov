param(
    [string]$RepoRoot = ".",
    [string]$PfxPath = "",
    [string]$PfxBase64 = "",
    [string]$PfxPassword = "",
    [string]$Thumbprint = ""
)

$ErrorActionPreference = "Stop"

function Fail([string]$Message) {
    Write-Error $Message
    exit 1
}

function Normalize-Thumbprint([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return ""
    }
    return ($Value -replace "\s", "").ToUpper()
}

$repoRootResolved = (Resolve-Path $RepoRoot).Path
$appRoot = Join-Path $repoRootResolved "gitgov"
$tauriConfigPath = Join-Path $appRoot "src-tauri\\tauri.conf.json"

if (-not (Test-Path $tauriConfigPath)) {
    Fail "No se encontro tauri.conf.json en: $tauriConfigPath"
}

if ([string]::IsNullOrWhiteSpace($PfxPath) -and [string]::IsNullOrWhiteSpace($PfxBase64) -and [string]::IsNullOrWhiteSpace($Thumbprint)) {
    Fail "Debes enviar PfxPath/PfxBase64 o Thumbprint."
}

if ((-not [string]::IsNullOrWhiteSpace($PfxPath) -or -not [string]::IsNullOrWhiteSpace($PfxBase64)) -and [string]::IsNullOrWhiteSpace($PfxPassword)) {
    Fail "Si importas PFX, PfxPassword es obligatorio."
}

if (-not [string]::IsNullOrWhiteSpace($PfxPath) -and -not (Test-Path $PfxPath)) {
    Fail "No se encontro PFX en: $PfxPath"
}

if (-not [string]::IsNullOrWhiteSpace($PfxBase64)) {
    $tempPfxPath = Join-Path $env:TEMP ("gitgov-codesign-" + [Guid]::NewGuid().ToString() + ".pfx")
    [IO.File]::WriteAllBytes($tempPfxPath, [Convert]::FromBase64String($PfxBase64))
    $PfxPath = $tempPfxPath
}

try {
    if (-not [string]::IsNullOrWhiteSpace($PfxPath)) {
        $securePassword = ConvertTo-SecureString $PfxPassword -AsPlainText -Force
        Import-PfxCertificate -FilePath $PfxPath -CertStoreLocation Cert:\CurrentUser\My -Password $securePassword | Out-Null
    }

    $normalizedThumbprint = Normalize-Thumbprint $Thumbprint
    if ([string]::IsNullOrWhiteSpace($normalizedThumbprint)) {
        $codesignCerts = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Sort-Object NotAfter -Descending
        if ($codesignCerts.Count -eq 0) {
            Fail "No hay certificados de code-signing en Cert:\CurrentUser\My."
        }
        $normalizedThumbprint = ($codesignCerts[0].Thumbprint -replace "\s", "").ToUpper()
    }

    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { ($_.Thumbprint -replace "\s", "").ToUpper() -eq $normalizedThumbprint } | Select-Object -First 1
    if (-not $cert) {
        Fail "No se encontro el certificado con thumbprint $normalizedThumbprint en Cert:\CurrentUser\My."
    }

    $originalConfig = Get-Content $tauriConfigPath -Raw
    try {
        $config = $originalConfig | ConvertFrom-Json -Depth 100
        if (-not $config.bundle.windows) {
            $config.bundle | Add-Member -NotePropertyName windows -NotePropertyValue (@{})
        }
        $config.bundle.windows.certificateThumbprint = $normalizedThumbprint
        $config.bundle.windows.digestAlgorithm = "sha256"
        if ([string]::IsNullOrWhiteSpace($config.bundle.windows.timestampUrl)) {
            $config.bundle.windows.timestampUrl = "http://timestamp.digicert.com"
        }
        $config | ConvertTo-Json -Depth 100 | Set-Content $tauriConfigPath -Encoding UTF8

        Push-Location $appRoot
        try {
            npm run tauri build
        }
        finally {
            Pop-Location
        }
    }
    finally {
        Set-Content -Path $tauriConfigPath -Value $originalConfig -Encoding UTF8
    }

    $bundleRoot = Join-Path $appRoot "src-tauri\\target\\release\\bundle"
    $msi = Get-ChildItem -Path $bundleRoot -Recurse -Filter "*.msi" | Select-Object -First 1
    $nsis = Get-ChildItem -Path $bundleRoot -Recurse -Filter "*-setup.exe" | Select-Object -First 1
    if (-not $msi -or -not $nsis) {
        Fail "No se encontraron artefactos MSI/NSIS en $bundleRoot"
    }

    $targets = @($msi.FullName, $nsis.FullName)
    foreach ($target in $targets) {
        $sig = Get-AuthenticodeSignature $target
        if ($sig.Status -ne "Valid") {
            Fail "Firma invalida en '$target' (Status: $($sig.Status))."
        }
        $hash = Get-FileHash -Path $target -Algorithm SHA256
        $hash.Hash | Out-File -FilePath ($target + ".sha256") -Encoding ascii
        Write-Host "[OK] Signed + hash: $target"
    }

    Write-Host "[OK] Build firmado completado."
}
finally {
    if ((Test-Path variable:tempPfxPath) -and (Test-Path $tempPfxPath)) {
        Remove-Item $tempPfxPath -Force -ErrorAction SilentlyContinue
    }
}
