param(
    [ValidateRange(1, 500)]
    [int]$TargetMiB = 100,
    [ValidateSet("standard", "wide", "low-cardinality", "long-text")]
    [string]$ShapeProfile = "standard",
    [ValidateRange(2, 5)]
    [int]$SustainedRuns = 3,
    [ValidateRange(1, 3)]
    [int]$ProjectUpdateRuns = 2,
    [ValidateRange(30, 900)]
    [int]$TimeoutSeconds = 900
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Import-Module (Join-Path $PSScriptRoot "performance-matrix.psm1") -Force
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/performance-benchmark/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$WorkDirectory = Join-Path $EvidenceDirectory "work"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$InputPath = Join-Path $WorkDirectory "benchmark-input.csv"
$ScaleMatrixDefinition = Get-PerformanceScaleMatrixDefinition -Path (Join-Path $ProjectRoot "fixtures\performance\dataset-scale-matrix-v1.json")
$ShapeProfileDefinition = Get-PerformanceScaleProfile -ProfileId $ShapeProfile -Definition $ScaleMatrixDefinition
$RecipePath = Join-Path $WorkDirectory "benchmark-recipe.json"
$RulesPath = Join-Path $WorkDirectory "benchmark-rules.json"
$ProjectRulesPath = Join-Path $WorkDirectory "benchmark-project-rules.json"
$ProjectStorePath = Join-Path $WorkDirectory "benchmark-project-store"
$CsvOutputPath = Join-Path $WorkDirectory "benchmark-output.csv"
$ParquetOutputPath = Join-Path $WorkDirectory "benchmark-output.parquet"
$ProjectOutputPath = Join-Path $WorkDirectory "benchmark-project-output.parquet"
$Status = "failed"
$FailureMessage = $null
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$BuildDurationMs = $null
$InputRowCount = 0
$InputSizeBytes = 0L
$InputInfo = [ordered]@{
    profileId = $ShapeProfile
    rowCount = 0
    sizeBytes = 0L
    columnCount = [int]$ShapeProfileDefinition.columnCount
    nameDistinctCount = 0
    notesDistinctCount = 0
    notesBytesPerRow = [int]$ShapeProfileDefinition.notesBytesPerRow
    generatedColumnCount = [int]$ShapeProfileDefinition.columnCount - 4
}
$PeakWorkspaceDiskBytes = 0L
$CommandResults = [System.Collections.Generic.List[object]]::new()
$OutputResults = [System.Collections.Generic.List[object]]::new()
$CliPath = $null
$CancellationBenchmark = [ordered]@{
    status = "not-measured"
    reason = "probe-not-run"
}

function Get-RelativePath {
    param([string]$Path)

    return $Path.Substring($ProjectRoot.Length).TrimStart("\", "/").Replace("\", "/")
}

function Write-SanitizedEvidenceText {
    param(
        [string]$Path,
        [string]$Text
    )

    $Sanitized = $Text -replace [regex]::Escape($ProjectRoot), "[project-root]"
    $Sanitized = $Sanitized -replace [regex]::Escape($EvidenceDirectory), "[evidence]"
    [System.IO.File]::WriteAllText($Path, $Sanitized)
}

function Get-WorkspaceDiskBytes {
    # The CLI creates and removes staging files while this samples the
    # workspace; a file vanishing mid-enumeration throws a terminating
    # Win32Exception that -ErrorAction does not suppress, so retry the sample.
    for ($Attempt = 1; ; $Attempt++) {
        try {
            $Files = @(Get-ChildItem -LiteralPath $WorkDirectory -File -Recurse -Force -ErrorAction SilentlyContinue)
            if ($Files.Count -eq 0) {
                return 0L
            }
            return [int64](($Files | Measure-Object -Property Length -Sum).Sum)
        }
        catch [System.ComponentModel.Win32Exception], [System.IO.IOException] {
            if ($Attempt -ge 5) {
                throw
            }
        }
    }
}

function New-BenchmarkFiles {
    Set-Content -LiteralPath $RecipePath -Encoding ascii -Value @'
{
  "version": 1,
  "name": "Performance benchmark",
  "savedAt": "2026-01-01T00:00:00Z",
  "recipe": {
    "renames": [{ "from": "amount", "to": "total" }],
    "casts": [],
    "dateParses": [],
    "filters": [],
    "calculatedColumn": null,
    "findReplace": null,
    "keepColumns": null,
    "splitColumn": null,
    "mergeColumns": null,
    "outlierTreatments": [],
    "groupSummary": null,
    "contactNormalizations": [],
    "textExtractions": []
  }
}
'@
    Set-Content -LiteralPath $RulesPath -Encoding ascii -Value @'
{
  "version": 1,
  "rules": [{ "column": "amount", "kind": "not_null", "maxInvalid": 0 }]
}
'@
    Set-Content -LiteralPath $ProjectRulesPath -Encoding ascii -Value @'
{
  "version": 1,
  "rules": [{ "column": "total", "kind": "not_null", "maxInvalid": 0 }]
}
'@
}

function Invoke-MeasuredCli {
    param(
        [string]$Name,
        [string[]]$Arguments,
        [string]$EvidenceTag = $Name,
        [switch]$ReturnStdout,
        [switch]$SanitizeStdout
    )

    $StdoutPath = Join-Path $EvidenceDirectory "$EvidenceTag.stdout.log"
    $StderrPath = Join-Path $EvidenceDirectory "$EvidenceTag.stderr.log"
    $StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = $CliPath
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
    $PeakWorkingSetBytes = 0L
    $PeakCommandWorkspaceDiskBytes = Get-WorkspaceDiskBytes
    try {
        while (-not $Process.HasExited) {
            $Process.Refresh()
            $PeakWorkingSetBytes = [math]::Max($PeakWorkingSetBytes, [int64]$Process.WorkingSet64)
            $PeakCommandWorkspaceDiskBytes = [math]::Max($PeakCommandWorkspaceDiskBytes, (Get-WorkspaceDiskBytes))
            if ($Stopwatch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
                try { $Process.Kill($true) } catch { $Process.Kill() }
                throw "$Name excedió el timeout de $TimeoutSeconds segundos."
            }
            Start-Sleep -Milliseconds 25
        }
        $Process.Refresh()
        $PeakWorkingSetBytes = [math]::Max($PeakWorkingSetBytes, [int64]$Process.PeakWorkingSet64)
        $PeakCommandWorkspaceDiskBytes = [math]::Max($PeakCommandWorkspaceDiskBytes, (Get-WorkspaceDiskBytes))
        $Process.WaitForExit()
        $Stdout = $StdoutTask.Result
        $Stderr = $StderrTask.Result
        $ExitCode = $Process.ExitCode
    }
    finally {
        $Stopwatch.Stop()
        $Process.Dispose()
    }

    $PersistedStdout = if ($SanitizeStdout) {
        "[stdout sanitizado; el contrato se validó en memoria]`n"
    }
    else {
        $Stdout
    }
    [System.IO.File]::WriteAllText($StdoutPath, $PersistedStdout)
    [System.IO.File]::WriteAllText($StderrPath, $Stderr)
    if ($ExitCode -ne 0) {
        throw "$Name terminó con código $ExitCode."
    }
    if ($Stderr -match [regex]::Escape($ProjectRoot) -or $Stderr -match [regex]::Escape($EvidenceDirectory)) {
        throw "$Name expuso una ruta absoluta por stderr."
    }

    if ($Stdout -match [regex]::Escape($ProjectRoot) -or $Stdout -match [regex]::Escape($EvidenceDirectory)) {
        throw "$Name expuso una ruta absoluta por stdout."
    }

    $Result = [ordered]@{
        name = $Name
        status = "passed"
        exitCode = $ExitCode
        durationMs = [math]::Round($Stopwatch.Elapsed.TotalMilliseconds, 2)
        peakWorkingSetBytes = $PeakWorkingSetBytes
        peakSampledWorkspaceDiskBytes = $PeakCommandWorkspaceDiskBytes
    }
    $script:PeakWorkspaceDiskBytes = [math]::Max($script:PeakWorkspaceDiskBytes, $PeakCommandWorkspaceDiskBytes)
    if ($ReturnStdout) {
        $Result.stdout = $Stdout
    }
    return $Result
}

function Invoke-CancellationBenchmark {
    $CancellationOutputDirectory = Join-Path $WorkDirectory "cancellation"
    New-Item -ItemType Directory -Path $CancellationOutputDirectory -Force | Out-Null

    $StdoutPath = Join-Path $EvidenceDirectory "cancellation.stdout.log"
    $StderrPath = Join-Path $EvidenceDirectory "cancellation.stderr.log"
    $StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = "cargo"
    $StartInfo.WorkingDirectory = $TauriRoot
    $StartInfo.UseShellExecute = $false
    $StartInfo.CreateNoWindow = $true
    $StartInfo.RedirectStandardOutput = $true
    $StartInfo.RedirectStandardError = $true
    $Arguments = @(
        "test",
        "--lib",
        "dataset::tests::benchmark_source_backed_export_cooperative_cancellation",
        "--",
        "--exact",
        "--ignored",
        "--nocapture"
    )
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
    $StartInfo.Environment["COLUMNIA_TEST_HARNESS_MANIFEST"] = "1"
    $StartInfo.Environment["COLUMNIA_BENCHMARK_CANCELLATION_INPUT"] = $InputPath
    $StartInfo.Environment["COLUMNIA_BENCHMARK_CANCELLATION_OUTPUT"] = $CancellationOutputDirectory

    $Process = [System.Diagnostics.Process]::new()
    $Process.StartInfo = $StartInfo
    $Stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$Process.Start()
    $StdoutTask = $Process.StandardOutput.ReadToEndAsync()
    $StderrTask = $Process.StandardError.ReadToEndAsync()
    try {
        while (-not $Process.HasExited) {
            if ($Stopwatch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
                try { $Process.Kill($true) } catch { $Process.Kill() }
                throw "La medición de cancelación excedió el timeout de $TimeoutSeconds segundos."
            }
            Start-Sleep -Milliseconds 25
        }
        $Process.WaitForExit()
        $Stdout = $StdoutTask.Result
        $Stderr = $StderrTask.Result
        $ExitCode = $Process.ExitCode
    }
    finally {
        $Stopwatch.Stop()
        $Process.Dispose()
    }

    Write-SanitizedEvidenceText -Path $StdoutPath -Text $Stdout
    Write-SanitizedEvidenceText -Path $StderrPath -Text $Stderr
    if ($ExitCode -ne 0) {
        throw "La prueba nativa de cancelación terminó con código $ExitCode."
    }

    $JsonMatches = [regex]::Matches($Stdout, '(?m)^COLUMNIA_CANCELLATION_BENCHMARK_JSON=(.+)$')
    if ($JsonMatches.Count -ne 1) {
        throw "La prueba nativa no produjo una medición JSON de cancelación."
    }
    $Benchmark = $JsonMatches[0].Groups[1].Value.Trim() | ConvertFrom-Json
    if ($Benchmark.schemaVersion -ne 1 -or
        $Benchmark.status -ne "measured" -or
        $Benchmark.operation -ne "sourceBackedCsvExport" -or
        [int]$Benchmark.sampleCount -ne 3 -or
        @($Benchmark.latencySamplesMs).Count -ne 3 -or
        [double]$Benchmark.maxRequestToTerminalLatencyMs -lt 0 -or
        $Benchmark.partialPublication -ne $false -or
        $Benchmark.previousOutputPreserved -ne $true -or
        $Benchmark.cleanupConfirmed -ne $true) {
        throw "La evidencia de cancelación no confirmó latencia, atomicidad y limpieza."
    }
    if (@(Get-ChildItem -LiteralPath $CancellationOutputDirectory -Force).Count -ne 0) {
        throw "La prueba nativa dejó restos en su directorio temporal."
    }
    return $Benchmark
}

function Assert-Output {
    param(
        [string]$Name,
        [string]$Path,
        [string]$Format
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Name no produjo una salida."
    }
    $Size = (Get-Item -LiteralPath $Path).Length
    if ($Size -le 0) {
        throw "$Name produjo una salida vacía."
    }
    [void]$OutputResults.Add([ordered]@{
        name = $Name
        format = $Format
        fileName = [System.IO.Path]::GetFileName($Path)
        sizeBytes = $Size
    })
}

New-Item -ItemType Directory -Path $WorkDirectory -Force | Out-Null

try {
    $TargetBytes = [int64]$TargetMiB * 1024 * 1024
    $BuildTimer = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location $TauriRoot
    try {
        $PreviousCargoProfileDevDebug = $env:CARGO_PROFILE_DEV_DEBUG
        $env:CARGO_PROFILE_DEV_DEBUG = "0"
        $BuildStdout = Join-Path $EvidenceDirectory "build.stdout.log"
        $BuildStderr = Join-Path $EvidenceDirectory "build.stderr.log"
        $PreviousErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = "Continue"
            & cargo build --quiet --bin columnia-cli 1> $BuildStdout 2> $BuildStderr
            foreach ($BuildLog in @($BuildStdout, $BuildStderr)) {
                if (Test-Path -LiteralPath $BuildLog -PathType Leaf) {
                    Write-SanitizedEvidenceText -Path $BuildLog -Text ([System.IO.File]::ReadAllText($BuildLog))
                }
            }
            if ($LASTEXITCODE -ne 0) {
                throw "No se pudo compilar columnia-cli."
            }
        }
        finally {
            $ErrorActionPreference = $PreviousErrorActionPreference
            if ($null -eq $PreviousCargoProfileDevDebug) {
                Remove-Item Env:CARGO_PROFILE_DEV_DEBUG -ErrorAction SilentlyContinue
            }
            else {
                $env:CARGO_PROFILE_DEV_DEBUG = $PreviousCargoProfileDevDebug
            }
        }
    }
    finally {
        Pop-Location
        $BuildTimer.Stop()
        $BuildDurationMs = [math]::Round($BuildTimer.Elapsed.TotalMilliseconds, 2)
    }

    $ExecutableName = if ($env:OS -eq "Windows_NT") { "columnia-cli.exe" } else { "columnia-cli" }
    $CliPath = Join-Path $TauriRoot "target\debug\$ExecutableName"
    if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
        throw "La compilación no produjo columnia-cli."
    }

    $InputInfo = Write-PerformanceScaleCsv -Destination $InputPath -TargetBytes $TargetBytes -Profile $ShapeProfileDefinition
    $InputRowCount = $InputInfo.rowCount
    $InputSizeBytes = $InputInfo.sizeBytes
    $PeakWorkspaceDiskBytes = Get-WorkspaceDiskBytes
    New-BenchmarkFiles

    $InputRelative = Get-RelativePath -Path $InputPath
    $RecipeRelative = Get-RelativePath -Path $RecipePath
    $RulesRelative = Get-RelativePath -Path $RulesPath
    $ProjectRulesRelative = Get-RelativePath -Path $ProjectRulesPath
    $ProjectStoreRelative = Get-RelativePath -Path $ProjectStorePath
    $CsvOutputRelative = Get-RelativePath -Path $CsvOutputPath
    $ParquetOutputRelative = Get-RelativePath -Path $ParquetOutputPath
    $ProjectOutputRelative = Get-RelativePath -Path $ProjectOutputPath

    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "inspect" -Arguments @(
        "inspect", "--input", $InputRelative
    )))
    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "validate" -Arguments @(
        "validate", "--input", $InputRelative, "--rules", $RulesRelative
    )))
    for ($Iteration = 1; $Iteration -le $SustainedRuns; $Iteration++) {
        $CsvResult = Invoke-MeasuredCli -Name "transform-csv" -EvidenceTag "transform-csv-$Iteration" -Arguments @(
            "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
            "--output", $CsvOutputRelative, "--format", "csv"
        )
        $CsvResult.iteration = $Iteration
        [void]$CommandResults.Add($CsvResult)
        Assert-Output -Name "transform-csv" -Path $CsvOutputPath -Format "CSV"

        $ParquetResult = Invoke-MeasuredCli -Name "transform-parquet" -EvidenceTag "transform-parquet-$Iteration" -Arguments @(
            "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
            "--output", $ParquetOutputRelative, "--format", "parquet"
        )
        $ParquetResult.iteration = $Iteration
        [void]$CommandResults.Add($ParquetResult)
        Assert-Output -Name "transform-parquet" -Path $ParquetOutputPath -Format "Parquet"
    }

    $ProjectName = "__columnia_benchmark__"
    $SaveResult = Invoke-MeasuredCli -Name "project-save" -EvidenceTag "project-save" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-save", "--store", $ProjectStoreRelative, "--name", $ProjectName,
        "--input", $InputRelative, "--recipe", $RecipeRelative,
        "--rules", $ProjectRulesRelative, "--profile"
    )
    $SaveOutput = $SaveResult.stdout | ConvertFrom-Json
    $ProjectId = [string]$SaveOutput.project.id
    $SaveResult.Remove("stdout")
    if ([string]::IsNullOrWhiteSpace($ProjectId) -or $ProjectId.Length -gt 128) {
        throw "project-save no devolvió un identificador de proyecto válido."
    }
    [void]$CommandResults.Add($SaveResult)

    $InspectResult = Invoke-MeasuredCli -Name "project-inspect" -EvidenceTag "project-inspect" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-inspect", "--store", $ProjectStoreRelative, "--id", $ProjectId
    )
    $InspectOutput = $InspectResult.stdout | ConvertFrom-Json
    $InspectResult.Remove("stdout")
    if ($InspectOutput.profileCached -ne $true -or $InspectOutput.recipeDraftPresent -ne $true -or
        [int]$InspectOutput.history.entryCount -lt 1) {
        throw "project-inspect no confirmó perfil, receta e historial del benchmark."
    }
    [void]$CommandResults.Add($InspectResult)

    for ($Iteration = 1; $Iteration -le $ProjectUpdateRuns; $Iteration++) {
        $UpdateResult = Invoke-MeasuredCli -Name "project-save-update" -EvidenceTag "project-save-update-$Iteration" -ReturnStdout -SanitizeStdout -Arguments @(
            "project-save", "--store", $ProjectStoreRelative, "--name", $ProjectName,
            "--id", $ProjectId, "--input", $InputRelative, "--recipe", $RecipeRelative,
            "--rules", $ProjectRulesRelative, "--profile"
        )
        $UpdateOutput = $UpdateResult.stdout | ConvertFrom-Json
        $UpdateResult.Remove("stdout")
        $UpdateResult.iteration = $Iteration
        if ($UpdateOutput.created -ne $false -or [string]$UpdateOutput.project.id -ne $ProjectId) {
            throw "project-save-update no confirmó una actualización del mismo proyecto."
        }
        [void]$CommandResults.Add($UpdateResult)
    }

    $ReopenResult = Invoke-MeasuredCli -Name "project-inspect-reopen" -EvidenceTag "project-inspect-reopen" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-inspect", "--store", $ProjectStoreRelative, "--id", $ProjectId
    )
    $ReopenOutput = $ReopenResult.stdout | ConvertFrom-Json
    $ReopenResult.Remove("stdout")
    if ([string]$ReopenOutput.project.id -ne $ProjectId -or $ReopenOutput.profileCached -ne $true -or
        $ReopenOutput.recipeDraftPresent -ne $true -or [int]$ReopenOutput.history.entryCount -lt 1) {
        throw "project-inspect-reopen no confirmó la reapertura durable del proyecto."
    }
    [void]$CommandResults.Add($ReopenResult)

    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "project-export" -EvidenceTag "project-export" -Arguments @(
        "project-export", "--store", $ProjectStoreRelative, "--id", $ProjectId,
        "--output", $ProjectOutputRelative, "--format", "parquet"
    )))
    Assert-Output -Name "project-export" -Path $ProjectOutputPath -Format "Parquet"

    $ListBeforeDelete = Invoke-MeasuredCli -Name "project-list" -EvidenceTag "project-list-before-delete" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-list", "--store", $ProjectStoreRelative
    )
    $ListOutput = $ListBeforeDelete.stdout | ConvertFrom-Json
    $ListBeforeDelete.Remove("stdout")
    if (@($ListOutput.projects).Count -ne 1) {
        throw "project-list no devolvió exactamente el proyecto del benchmark."
    }
    [void]$CommandResults.Add($ListBeforeDelete)

    $DeleteResult = Invoke-MeasuredCli -Name "project-delete" -EvidenceTag "project-delete" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-delete", "--store", $ProjectStoreRelative, "--id", $ProjectId, "--confirm", $ProjectId
    )
    $DeleteOutput = $DeleteResult.stdout | ConvertFrom-Json
    $DeleteResult.Remove("stdout")
    if ($DeleteOutput.deleted -ne $true) {
        throw "project-delete no confirmó el borrado."
    }
    [void]$CommandResults.Add($DeleteResult)

    $ListAfterDelete = Invoke-MeasuredCli -Name "project-list" -EvidenceTag "project-list-after-delete" -ReturnStdout -SanitizeStdout -Arguments @(
        "project-list", "--store", $ProjectStoreRelative
    )
    $ListAfterOutput = $ListAfterDelete.stdout | ConvertFrom-Json
    $ListAfterDelete.Remove("stdout")
    if (@($ListAfterOutput.projects).Count -ne 0) {
        throw "project-list conservó proyectos después del cleanup."
    }
    [void]$CommandResults.Add($ListAfterDelete)
    $CancellationBenchmark = Invoke-CancellationBenchmark
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
    if ($CancellationBenchmark.status -eq "not-measured") {
        $CancellationBenchmark.reason = "probe-failed-or-not-reached"
    }
    $FailureLog = $_ | Format-List * -Force | Out-String
    Write-SanitizedEvidenceText -Path (Join-Path $EvidenceDirectory "failure.log") -Text $FailureLog
}
finally {
    if (Test-Path -LiteralPath $WorkDirectory) {
        $ResolvedWork = [System.IO.Path]::GetFullPath($WorkDirectory)
        $ResolvedEvidence = [System.IO.Path]::GetFullPath($EvidenceDirectory)
        if (-not $ResolvedWork.StartsWith($ResolvedEvidence + [System.IO.Path]::DirectorySeparatorChar)) {
            $Status = "failed"
            $FailureMessage = "Cleanup rechazado porque el directorio temporal salió del área de evidencia."
        }
        else {
            Remove-Item -LiteralPath $ResolvedWork -Recurse -Force
        }
    }
    $Timer.Stop()
    $CommandDurations = @($CommandResults | ForEach-Object { [double]$_.durationMs })
    $MeasuredWorkingSets = @($CommandResults | ForEach-Object { [int64]$_.peakWorkingSetBytes })
    $MeasuredDiskPeaks = @($CommandResults | ForEach-Object { [int64]$_.peakSampledWorkspaceDiskBytes })
    $MaximumCommandDurationMs = if ($CommandDurations.Count -eq 0) { 0.0 } else { [double](($CommandDurations | Measure-Object -Maximum).Maximum) }
    $TotalCommandDurationMs = if ($CommandDurations.Count -eq 0) { 0.0 } else { [double](($CommandDurations | Measure-Object -Sum).Sum) }
    $MaximumWorkingSetBytes = if ($MeasuredWorkingSets.Count -eq 0) { 0L } else { [int64](($MeasuredWorkingSets | Measure-Object -Maximum).Maximum) }
    $MaximumWorkspaceDiskBytes = if ($MeasuredDiskPeaks.Count -eq 0) { [int64]$PeakWorkspaceDiskBytes } else { [int64](($MeasuredDiskPeaks | Measure-Object -Maximum).Maximum) }
    $OutputSizeValues = @($OutputResults | ForEach-Object { [int64]$_.sizeBytes })
    $RecordedOutputBytes = if ($OutputSizeValues.Count -eq 0) { 0L } else { [int64](($OutputSizeValues | Measure-Object -Sum).Sum) }
    $CleanupConfirmed = -not (Test-Path -LiteralPath $WorkDirectory)
    $ScaleMatrixEvidence = [ordered]@{
        schemaVersion = 1
        profileId = $ShapeProfile
        dimensions = [ordered]@{
            targetMiB = $TargetMiB
            inputSizeBytes = $InputSizeBytes
            rowCount = $InputInfo.rowCount
            columnCount = $InputInfo.columnCount
            idDistinctCount = $InputInfo.rowCount
            nameDistinctCount = $InputInfo.nameDistinctCount
            notesDistinctCount = $InputInfo.notesDistinctCount
            notesBytesPerRow = $InputInfo.notesBytesPerRow
            generatedColumnCount = $InputInfo.generatedColumnCount
        }
        measures = [ordered]@{
            peakWorkingSetBytes = $MaximumWorkingSetBytes
            peakSampledWorkspaceDiskBytes = $MaximumWorkspaceDiskBytes
            workspaceDiskMeasurement = "workspace file sizes sampled while each CLI command runs"
            recordedOutputBytes = $RecordedOutputBytes
            commandCount = $CommandResults.Count
            maxCommandDurationMs = $MaximumCommandDurationMs
            totalCommandDurationMs = $TotalCommandDurationMs
            cancellation = $CancellationBenchmark
            cleanupConfirmed = $CleanupConfirmed
        }
    }
    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        durationMs = [math]::Round($Timer.Elapsed.TotalMilliseconds, 2)
        targetMiB = $TargetMiB
        sustainedRuns = $SustainedRuns
        projectUpdateRuns = $ProjectUpdateRuns
        targetBytes = [int64]$TargetMiB * 1024 * 1024
        input = [ordered]@{
            fileName = "benchmark-input.csv"
            sizeBytes = $InputSizeBytes
            rowCount = $InputRowCount
            columnCount = $InputInfo.columnCount
        }
        buildDurationMs = $BuildDurationMs
        scaleMatrix = $ScaleMatrixEvidence
        commands = @($CommandResults)
        outputs = @($OutputResults)
        cleanupConfirmed = $CleanupConfirmed
        command = "columnia-cli"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Benchmark de datasets aprobado: $InputSizeBytes bytes, $InputRowCount filas, transformaciones sostenidas y ciclo durable de proyecto medidos."
Write-Host ("Cancelación source-backed: máximo {0} ms; publicación parcial={1}; limpieza={2}." -f $CancellationBenchmark.maxRequestToTerminalLatencyMs, $CancellationBenchmark.partialPublication, $CancellationBenchmark.cleanupConfirmed)
Write-Host "Evidencia: $EvidenceRelativePath"
