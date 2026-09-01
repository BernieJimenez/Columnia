param(
    [ValidateRange(512, 2048)]
    [int]$TargetMiB = 512,
    [ValidateRange(60, 3600)]
    [int]$TimeoutSeconds = 1800
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/duckdb-join-benchmark/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$StdoutPath = Join-Path $EvidenceDirectory "cargo.stdout.log"
$StderrPath = Join-Path $EvidenceDirectory "cargo.stderr.log"
$Status = "failed"
$FailureMessage = $null
$Metrics = $null
$ExitCode = $null
$PeakWorkingSetBytes = 0L
$ElapsedMs = $null
$Stdout = ""
$Stderr = ""
$WorkingSetBudgetMiB = 512

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null

try {
    $Cargo = Get-Command cargo.exe -ErrorAction Stop
    $TargetBytes = [int64]$TargetMiB * 1MB
    $PreviousTarget = [Environment]::GetEnvironmentVariable(
        "COLUMNIA_DUCKDB_JOIN_TARGET_MIB",
        "Process"
    )
    [Environment]::SetEnvironmentVariable(
        "COLUMNIA_DUCKDB_JOIN_TARGET_MIB",
        [string]$TargetMiB,
        "Process"
    )

    try {
        $Arguments = @(
            "test",
            "--manifest-path",
            (Join-Path $ProjectRoot "src-tauri/Cargo.toml"),
            "--lib",
            "duckdb_source_backed_join_handles_large_file",
            "--",
            "--ignored",
            "--nocapture"
        )
        $StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $StartInfo.FileName = $Cargo.Source
        $StartInfo.WorkingDirectory = $ProjectRoot
        $StartInfo.UseShellExecute = $false
        $StartInfo.CreateNoWindow = $true
        $StartInfo.RedirectStandardOutput = $true
        $StartInfo.RedirectStandardError = $true
        if ($StartInfo.PSObject.Properties.Name -contains "ArgumentList") {
            foreach ($Argument in $Arguments) {
                [void]$StartInfo.ArgumentList.Add($Argument)
            }
        }
        else {
            $StartInfo.Arguments = ($Arguments | ForEach-Object {
                if ($_ -notmatch '[\s"]') {
                    $_
                }
                else {
                    $Escaped = $_ -replace '(\\*)"', '$1$1\"'
                    $Escaped = $Escaped -replace '(\\+)$', '$1$1'
                    '"' + $Escaped + '"'
                }
            }) -join " "
        }
        $Process = [System.Diagnostics.Process]::new()
        $Process.StartInfo = $StartInfo
        $Stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        [void]$Process.Start()
        $StdoutTask = $Process.StandardOutput.ReadToEndAsync()
        $StderrTask = $Process.StandardError.ReadToEndAsync()
        try {
            while (-not $Process.HasExited) {
                $Process.Refresh()
                $PeakWorkingSetBytes = [math]::Max(
                    $PeakWorkingSetBytes,
                    [int64]$Process.WorkingSet64
                )
                Get-Process -Name "columnia_lib-*" -ErrorAction SilentlyContinue |
                    ForEach-Object {
                        $_.Refresh()
                        $PeakWorkingSetBytes = [math]::Max(
                            $PeakWorkingSetBytes,
                            [int64]$_.WorkingSet64
                        )
                    }
                if ($Stopwatch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
                    try { $Process.Kill($true) } catch { $Process.Kill() }
                    throw "El benchmark DuckDB excedió el timeout de $TimeoutSeconds segundos."
                }
                Start-Sleep -Milliseconds 25
            }
            $Process.Refresh()
            $PeakWorkingSetBytes = [math]::Max(
                $PeakWorkingSetBytes,
                [int64]$Process.PeakWorkingSet64
            )
            $Process.WaitForExit()
            $Stdout = $StdoutTask.Result
            $Stderr = $StderrTask.Result
            $ExitCode = $Process.ExitCode
            $ElapsedMs = $Stopwatch.ElapsedMilliseconds
        }
        finally {
            $Stopwatch.Stop()
            $Process.Dispose()
        }
    }
    finally {
        [Environment]::SetEnvironmentVariable(
            "COLUMNIA_DUCKDB_JOIN_TARGET_MIB",
            $PreviousTarget,
            "Process"
        )
    }

    [System.IO.File]::WriteAllText(
        $StdoutPath,
        $Stdout,
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        $StderrPath,
        $Stderr,
        [System.Text.UTF8Encoding]::new($false)
    )
    $Output = @(
        ($Stdout -split "\r?\n")
        ($Stderr -split "\r?\n")
    )
    $Marker = $Output |
        Where-Object { $_ -match '^DUCKDB_JOIN_BENCHMARK:(?<json>\{.*\})$' } |
        Select-Object -Last 1
    if ($null -eq $Marker) {
        throw "El benchmark no emitió la métrica DUCKDB_JOIN_BENCHMARK."
    }
    $Metrics = ($Marker -replace '^DUCKDB_JOIN_BENCHMARK:', '') | ConvertFrom-Json
    if ($ExitCode -ne 0) {
        throw "El test de escala terminó con código $ExitCode."
    }
    if ([int64]$Metrics.fileSizeBytes -lt $TargetBytes) {
        throw "La fuente generada quedó por debajo de $TargetMiB MiB."
    }
    if ([int64]$Metrics.resultRowCount -ne [int64]$Metrics.rowCount) {
        throw "El conteo del JOIN no coincide con el conteo de la fuente."
    }
    if ([int64]$Metrics.sourceBackedFrameRows -ne 0) {
        throw "El benchmark materializó el frame source-backed."
    }
    if ($Metrics.cleanupVerified -ne $true) {
        throw "El benchmark no confirmó cleanup."
    }
    if ($PeakWorkingSetBytes -gt ([int64]$WorkingSetBudgetMiB * 1MB)) {
        throw "El working set del benchmark excedió el presupuesto de $WorkingSetBudgetMiB MiB."
    }
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
}

$Commit = (& git -C $ProjectRoot rev-parse HEAD 2>$null).Trim()
$Version = ((Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw) | ConvertFrom-Json).version
$Summary = [ordered]@{
    schemaVersion = 1
    status = $Status
    generatedAt = $StartedAt.ToString("O")
    projectVersion = $Version
    commit = $Commit
    targetMiB = $TargetMiB
    timeoutSeconds = $TimeoutSeconds
    workingSetBudgetMiB = $WorkingSetBudgetMiB
    exitCode = $ExitCode
    elapsedMs = $ElapsedMs
    peakWorkingSetBytes = $PeakWorkingSetBytes
    metrics = $Metrics
    stdout = "$EvidenceRelativePath/cargo.stdout.log"
    stderr = "$EvidenceRelativePath/cargo.stderr.log"
    error = $FailureMessage
    evidenceDirectory = $EvidenceRelativePath
}
[System.IO.File]::WriteAllText(
    $SummaryPath,
    (($Summary | ConvertTo-Json -Depth 8) + [Environment]::NewLine),
    [System.Text.UTF8Encoding]::new($false)
)

if ($Status -ne "passed") {
    Write-Error "Benchmark DuckDB JOIN falló: $FailureMessage. Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Output "Benchmark DuckDB JOIN aprobado: $EvidenceRelativePath"
