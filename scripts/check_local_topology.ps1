<#
.SYNOPSIS
    GitGov — diagnóstico de topología local.
    Detecta procesos en puertos 3000/3001 y consulta /health en ambos.

.DESCRIPTION
    Objetivo: evitar split-brain entre server local (127.0.0.1:3000) y
    Docker/server alternativo (127.0.0.1:3001).

    Salida esperada (estado limpio):
        [OK] Solo un server activo en 3000. Sin riesgo de split-brain.

    Señal de alerta:
        [WARN] Procesos en AMBOS puertos — riesgo split-brain.

.EXAMPLE
    .\scripts\check_local_topology.ps1
    .\scripts\check_local_topology.ps1 -Verbose

.NOTES
    Compatible con PowerShell 5.1+
#>

[CmdletBinding()]
param()

$PORTS   = @(3000, 3001)
$TIMEOUT = 3   # segundos para cada HTTP request

# ── helpers ──────────────────────────────────────────────────────────────────

function Get-ProcessOnPort {
    param([int]$Port)
    $lines = netstat -ano 2>$null | Select-String ":$Port\s"
    if (-not $lines) { return @() }
    $pids = $lines |
        ForEach-Object { ($_ -split '\s+')[-1] } |
        Where-Object { $_ -match '^\d+$' } |
        Sort-Object -Unique
    $procs = foreach ($p in $pids) {
        try { Get-Process -Id ([int]$p) -ErrorAction SilentlyContinue | Select-Object -First 1 }
        catch { }
    }
    @($procs | Where-Object { $_ -ne $null })
}

function Invoke-Health {
    param([int]$Port)
    $url = "http://127.0.0.1:$Port/health"
    try {
        $resp = Invoke-WebRequest -Uri $url -TimeoutSec $TIMEOUT -UseBasicParsing -ErrorAction Stop
        $bodyText = $resp.Content
        $parsed = $null
        try { $parsed = $bodyText | ConvertFrom-Json -ErrorAction Stop } catch { }
        if ($parsed -ne $null) { $bodyText = ($parsed | ConvertTo-Json -Compress) }
        return [pscustomobject]@{
            Status  = [int]$resp.StatusCode
            Body    = $bodyText
            Error   = $null
        }
    }
    catch {
        $code = 0
        if ($_.Exception.Response -ne $null) {
            $code = [int]$_.Exception.Response.StatusCode
        }
        return [pscustomobject]@{
            Status = $code
            Body   = $null
            Error  = $_.Exception.Message
        }
    }
}

# ── main ─────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  GitGov - Diagnostico de Topologia Local" -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host ""

$results = @{}

foreach ($port in $PORTS) {
    $label = if ($port -eq 3000) { "server local (dev)" } else { "server Docker / alternativo" }
    Write-Host "-- Puerto $port  [$label]" -ForegroundColor Yellow

    # 1. proceso
    $procs = Get-ProcessOnPort -Port $port
    if ($procs.Count -gt 0) {
        foreach ($p in $procs) {
            Write-Host "   Proceso : $($p.Name) (PID $($p.Id))" -ForegroundColor White
        }
        $results[$port] = @{ HasProcess = $true; Procs = $procs }
    } else {
        Write-Host "   Proceso : ninguno en escucha" -ForegroundColor DarkGray
        $results[$port] = @{ HasProcess = $false; Procs = @() }
    }

    # 2. /health
    Write-Host "   GET /health ..." -NoNewline
    $h = Invoke-Health -Port $port
    if ($h.Status -ge 200 -and $h.Status -lt 300) {
        Write-Host " HTTP $($h.Status) OK" -ForegroundColor Green
        Write-Verbose "   Body: $($h.Body)"
        $results[$port].HealthOk = $true
    } elseif ($h.Status -gt 0) {
        Write-Host " HTTP $($h.Status)" -ForegroundColor Red
        $results[$port].HealthOk = $false
    } else {
        $errMsg = if ($h.Error) { $h.Error } else { "sin respuesta" }
        Write-Host " Sin respuesta ($errMsg)" -ForegroundColor DarkGray
        $results[$port].HealthOk = $false
    }

    Write-Host ""
}

# ── diagnostico final ─────────────────────────────────────────────────────────

$active3000 = $results[3000].HasProcess -or $results[3000].HealthOk
$active3001 = $results[3001].HasProcess -or $results[3001].HealthOk

Write-Host "-------------------------------------------------------" -ForegroundColor Cyan

if ($active3000 -and $active3001) {
    Write-Host ""
    Write-Host "  [WARN] Procesos en AMBOS puertos 3000 y 3001." -ForegroundColor Red
    Write-Host "         Riesgo de SPLIT-BRAIN: Desktop puede enviar eventos" -ForegroundColor Red
    Write-Host "         a un proceso y el dashboard leer del otro." -ForegroundColor Red
    Write-Host ""
    Write-Host "  Accion recomendada:" -ForegroundColor Yellow
    Write-Host "    - Server local dev  --> debe estar en 127.0.0.1:3000" -ForegroundColor Yellow
    Write-Host "    - Server Docker     --> debe estar en 127.0.0.1:3001" -ForegroundColor Yellow
    Write-Host "    - Verifica GITGOV_SERVER_URL y VITE_SERVER_URL apuntan" -ForegroundColor Yellow
    Write-Host "      al proceso correcto antes de usar el dashboard." -ForegroundColor Yellow
} elseif ($active3000 -and -not $active3001) {
    Write-Host ""
    Write-Host "  [OK] Solo un server activo en 3000. Sin riesgo de split-brain." -ForegroundColor Green
} elseif (-not $active3000 -and $active3001) {
    Write-Host ""
    Write-Host "  [OK] Solo un server activo en 3001 (Docker)." -ForegroundColor Green
    Write-Host "       Asegurate de que GITGOV_SERVER_URL=http://127.0.0.1:3001" -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "  [INFO] Ningun server GitGov detectado en 3000 ni 3001." -ForegroundColor DarkGray
    Write-Host "         Arranca el server antes de usar el dashboard." -ForegroundColor DarkGray
}

Write-Host "-------------------------------------------------------" -ForegroundColor Cyan
Write-Host ""
