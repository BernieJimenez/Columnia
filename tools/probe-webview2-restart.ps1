param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 9222,
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "app-data-guard.ps1")
$Timestamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/webview2-restart/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$StartedAt = [DateTimeOffset]::UtcNow
$ReusableTaskName = "__columnia_native_probe__restart_$([Guid]::NewGuid().ToString('N').ToLowerInvariant())"

function Invoke-RestartPhase {
    param(
        [ValidateSet("restart-prepare", "restart-verify")][string]$Mode,
        [Parameter(Mandatory = $true)][string]$ReusableTaskName
    )

    $ScriptPath = Join-Path $ProjectRoot "tools\probe-webview2-cdp.ps1"
    $PowerShellCommand = (Get-Command powershell.exe -ErrorAction Stop).Source
    $Output = @(& $PowerShellCommand -NoProfile -ExecutionPolicy Bypass -File $ScriptPath `
        -Port $Port `
        -TimeoutSeconds $TimeoutSeconds `
        -RunPlaywright `
        -RunProjects `
        -RunProjectMutations `
        -ProjectProbeMode $Mode `
        -ProjectProbeTaskName $ReusableTaskName 2>&1)
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
    $PhaseSummaryPath = Join-Path $ProjectRoot (($Evidence -replace '/', '\') + '\summary.json')
    if (-not (Test-Path -LiteralPath $PhaseSummaryPath -PathType Leaf)) {
        throw "La fase $Mode no produjo summary.json en su evidencia."
    }
    $PhaseSummary = Get-Content -LiteralPath $PhaseSummaryPath -Raw | ConvertFrom-Json
    $NativeIpc = if ($null -eq $PhaseSummary.projects) { $null } else { @($PhaseSummary.projects.pages)[0].nativeIpc }
    [ordered]@{
        mode = $Mode
        status = "passed"
        exitCode = $ExitCode
        evidenceDirectory = $Evidence
        performanceBudget = $PhaseSummary.performanceBudget
        nativeOperationCount = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.nativeOperationCount }
        nativeOperationDurationMs = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.nativeOperationDurationMs }
        nativeSustainedRuns = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.nativeSustainedRuns }
        nativeSustainedTransformMaxMs = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.nativeSustainedTransformMaxMs }
        nativeSustainedExportMaxMs = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.nativeSustainedExportMaxMs }
        activePhaseToPersist = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.activePhaseToPersist }
        activePhaseRestored = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.activePhaseRestored }
        restoredActivePhase = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.restoredActivePhase }
        reusableTaskPersisted = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.reusableTaskPersisted }
        reusableTaskRestored = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.reusableTaskRestored }
        reusableTaskCleanupConfirmed = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.reusableTaskCleanupConfirmed }
        reusableTaskCatalogCountBefore = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.reusableTaskCatalogCountBefore }
        reusableTaskCatalogCountAfter = if ($null -eq $NativeIpc) { $null } else { $NativeIpc.reusableTaskCatalogCountAfter }
    }
}

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$Status = "failed"
$FailureMessage = $null
$Prepare = $null
$Verify = $null
$AppDataGuard = Backup-ColumniaAppData
$AppDataRestored = $false

try {
    $Prepare = Invoke-RestartPhase -Mode "restart-prepare" -ReusableTaskName $ReusableTaskName
    $Verify = Invoke-RestartPhase -Mode "restart-verify" -ReusableTaskName $ReusableTaskName
    if ($Prepare.activePhaseToPersist -ne "prepare") {
        throw "El smoke no confirmó que guardó el proyecto en la fase prepare."
    }
    if ($Verify.activePhaseRestored -ne $true -or $Verify.restoredActivePhase -ne "prepare") {
        throw "El smoke no confirmó que activePhase=prepare sobrevivió al reinicio."
    }
    if ($Prepare.reusableTaskPersisted -ne $true) {
        throw "El smoke no confirmó que guardó la tarea reutilizable antes del reinicio."
    }
    if ($Verify.reusableTaskRestored -ne $true -or $Verify.reusableTaskCleanupConfirmed -ne $true) {
        throw "El smoke no confirmó la reapertura y limpieza de la tarea reutilizable después del reinicio."
    }
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
}
finally {
    # Both phases stop their app processes before returning.
    try {
        $AppDataRestored = Restore-ColumniaAppData -Guard $AppDataGuard
    }
    catch {
        Write-Warning "No se pudieron restaurar los datos de Columnia; la copia sigue en $($AppDataGuard.Backup)."
    }
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
        activePhasePersistedAcrossRestart = [bool]($Prepare.activePhaseToPersist -eq "prepare" -and $Verify.activePhaseRestored -eq $true -and $Verify.restoredActivePhase -eq "prepare")
        reusableTaskPersistedAcrossRestart = [bool]($Prepare.reusableTaskPersisted -eq $true -and $Verify.reusableTaskRestored -eq $true -and $Verify.reusableTaskCleanupConfirmed -eq $true)
        reusableTaskName = $ReusableTaskName
        evidenceDirectory = $EvidenceRelativePath
        cleanupDelegatedToCdpPhases = $true
        appDataRestored = $AppDataRestored
        error = $FailureMessage
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -eq "passed") {
    Write-Host "Reinicio WebView2 aprobado: proyecto y tarea reutilizable sobreviven al reinicio; cleanup confirmado."
    Write-Host "Evidencia: $EvidenceRelativePath"
    exit 0
}

Write-Error "Probe de reinicio WebView2: $Status. $FailureMessage Evidencia: $EvidenceRelativePath"
exit 1
