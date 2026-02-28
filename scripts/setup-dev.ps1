# GitGov — Setup de identidad git local por repo
# ================================================
# Uso: .\scripts\setup-dev.ps1
# Uso con parametros: .\scripts\setup-dev.ps1 -Name "Tu Nombre" -Email "tu@email.com"
#
# Configura user.name, user.email y core.hooksPath de forma LOCAL (--local),
# sin afectar la configuracion global de git ni otros repositorios.
# Idempotente: es seguro ejecutarlo multiples veces.

param(
    [string]$Name  = "",
    [string]$Email = ""
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "GitGov — Setup de identidad git (--local)" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Este script configura tu identidad git SOLO para este repo," -ForegroundColor DarkGray
Write-Host "sin tocar tu configuracion global de git." -ForegroundColor DarkGray
Write-Host ""

# Verificar que estamos en la raiz del repo GitGov
if (-not (Test-Path ".git")) {
    Write-Host "ERROR: No se encontro .git en el directorio actual." -ForegroundColor Red
    Write-Host "Ejecuta este script desde la raiz del repositorio GitGov:" -ForegroundColor Yellow
    Write-Host "  cd C:\ruta\al\GitGov" -ForegroundColor Yellow
    Write-Host "  .\scripts\setup-dev.ps1" -ForegroundColor Yellow
    exit 1
}

# Leer valores actuales (local primero, luego global como referencia)
$currentLocalName  = git config --local user.name  2>$null
$currentLocalEmail = git config --local user.email 2>$null
$globalName        = git config --global user.name  2>$null
$globalEmail       = git config --global user.email 2>$null

if ($currentLocalName -or $currentLocalEmail) {
    Write-Host "Configuracion local actual en este repo:" -ForegroundColor Green
    Write-Host "  user.name  = $(if ($currentLocalName)  { $currentLocalName }  else { '(no definido)' })" -ForegroundColor White
    Write-Host "  user.email = $(if ($currentLocalEmail) { $currentLocalEmail } else { '(no definido)' })" -ForegroundColor White
    Write-Host ""
}

# --- Nombre ---
if (-not $Name) {
    $hint = if ($currentLocalName) { " [$currentLocalName]" } elseif ($globalName) { " [global: $globalName]" } else { "" }
    $answer = Read-Host "Tu nombre completo$hint"
    if (-not $answer -and $currentLocalName) {
        $Name = $currentLocalName
    } elseif (-not $answer -and $globalName) {
        $Name = $globalName
    } else {
        $Name = $answer
    }
}

if (-not $Name) {
    Write-Host "ERROR: El nombre no puede estar vacio." -ForegroundColor Red
    exit 1
}

# --- Email ---
if (-not $Email) {
    $hint = if ($currentLocalEmail) { " [$currentLocalEmail]" } elseif ($globalEmail) { " [global: $globalEmail]" } else { "" }
    $answer = Read-Host "Tu email$hint"
    if (-not $answer -and $currentLocalEmail) {
        $Email = $currentLocalEmail
    } elseif (-not $answer -and $globalEmail) {
        $Email = $globalEmail
    } else {
        $Email = $answer
    }
}

if (-not $Email) {
    Write-Host "ERROR: El email no puede estar vacio." -ForegroundColor Red
    exit 1
}

# Validar formato basico de email
if ($Email -notmatch "^[^@\s]+@[^@\s]+\.[^@\s]+$") {
    Write-Host "ERROR: El email '$Email' no tiene un formato valido." -ForegroundColor Red
    Write-Host "Ejemplo valido: nombre@empresa.com" -ForegroundColor Yellow
    exit 1
}

# --- Aplicar configuracion local ---
git config --local user.name  "$Name"
git config --local user.email "$Email"
git config --local core.hooksPath ".githooks"

# Hacer el hook ejecutable (via Git Bash si esta disponible en Windows)
$hookPath = ".githooks\pre-commit"
if (Test-Path $hookPath) {
    $gitBashCandidates = @(
        "C:\Program Files\Git\bin\bash.exe",
        "C:\Program Files (x86)\Git\bin\bash.exe"
    )
    foreach ($bash in $gitBashCandidates) {
        if (Test-Path $bash) {
            & $bash -c "chmod +x .githooks/pre-commit" 2>$null
            break
        }
    }
}

# --- Resumen final ---
Write-Host ""
Write-Host "Configuracion aplicada correctamente:" -ForegroundColor Green
Write-Host "  user.name   = $(git config --local user.name)" -ForegroundColor White
Write-Host "  user.email  = $(git config --local user.email)" -ForegroundColor White
Write-Host "  hooksPath   = $(git config --local core.hooksPath)" -ForegroundColor White
Write-Host ""

# Advertir si la config difiere de la global
if ($globalName -and $globalName -ne $Name) {
    Write-Host "Nota: tu nombre local ('$Name') difiere del global ('$globalName')." -ForegroundColor Yellow
    Write-Host "Esto es intencional: el repo GitGov usa su propia identidad." -ForegroundColor DarkGray
}
if ($globalEmail -and $globalEmail -ne $Email) {
    Write-Host "Nota: tu email local ('$Email') difiere del global ('$globalEmail')." -ForegroundColor Yellow
    Write-Host "Esto es intencional: el repo GitGov usa su propia identidad." -ForegroundColor DarkGray
}

Write-Host ""
Write-Host "El pre-commit hook validara tu identidad antes de cada commit CLI." -ForegroundColor Cyan
Write-Host "La Desktop App siempre usa tu cuenta GitHub autenticada." -ForegroundColor Cyan
Write-Host ""
Write-Host "Para mas informacion: docs/QUICKSTART.md" -ForegroundColor DarkGray
Write-Host ""
