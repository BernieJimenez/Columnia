param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 9222,
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Timestamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/webview2-restart/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$StartedAt = [DateTimeOffset]::UtcNow

function Invoke-RestartPhase {
    param([ValidateSet("restart-prepare", "restart-verify")][string]$Mode)

    $ScriptPath = Join-Path $ProjectRoot "tools\probe-webview2-cdp.ps1"
    $PowerShellCommand = (Get-Command powershell.exe -ErrorAction Stop).Source
    $Output = @(& $PowerShellCommand -NoProfile -ExecutionPolicy Bypass -File $ScriptPath `
        -Port $Port `
        -TimeoutSeconds $TimeoutSeconds `
        -RunPlaywright `
        -RunProjects `
        -RunProjectMutations `
        -ProjectProbeMode $Mode 2>&1)
    $ExitCode = $LASTEXITCODE
    $OutputText = [string]::Join([Environment]::NewLine, @($Output | ForEach-Object { [string]$_ }))
    $EvidenceLine = @($Output | Where-Object { ([string]$_) -match "^Evidencia:\s+" } | Select-Object -Last 1)
    $Evidence = if ($EvidenceLine.Count -gt 0) {
        ([string]$EvidenceLine[0] -replace "^Evidencia:\s+", "").Trim()
    }
    else {
        $null
    }
    if ($ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($Evidence)) {
        $detail = if ($OutputText) { $OutputText } else { "El probe no produjo salida." }
        throw "La fase $Mode falló: $detail"
    }
    [ordered]@{
        mode = $Mode
        status = "passed"
        exitCode = $ExitCode
        evidenceDirectory = $Evidence
    }
}

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$Status = "failed"
$FailureMessage = $null
$Prepare = $null
$Verify = $null

try {
    $Prepare = Invoke-RestartPhase -Mode "restart-prepare"
    $Verify = Invoke-RestartPhase -Mode "restart-verify"
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
}
finally {
    $Timer.Stop()
    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        durationMs = $Timer.ElapsedMilliseconds
        timeoutSeconds = $TimeoutSeconds
        cdpPort = $Port
        command = "npm run tauri dev"
        phases = @($Prepare, $Verify)
        evidenceDirectory = $EvidenceRelativePath
        cleanupDelegatedToCdpPhases = $true
        error = $FailureMessage
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -eq "passed") {
    Write-Host "Reinicio WebView2 aprobado: preparar→cerrar→reiniciar→reabrir→eliminar, con cleanup en ambas fases."
    Write-Host "Evidencia: $EvidenceRelativePath"
    exit 0
}

Write-Error "Probe de reinicio WebView2: $Status. $FailureMessage Evidencia: $EvidenceRelativePath"
exit 1
