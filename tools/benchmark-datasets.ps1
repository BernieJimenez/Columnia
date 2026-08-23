param(
    [ValidateRange(1, 500)]
    [int]$TargetMiB = 100,
    [ValidateRange(30, 900)]
    [int]$TimeoutSeconds = 900
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/performance-benchmark/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$WorkDirectory = Join-Path $EvidenceDirectory "work"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$InputPath = Join-Path $WorkDirectory "benchmark-input.csv"
$RecipePath = Join-Path $WorkDirectory "benchmark-recipe.json"
$RulesPath = Join-Path $WorkDirectory "benchmark-rules.json"
$CsvOutputPath = Join-Path $WorkDirectory "benchmark-output.csv"
$ParquetOutputPath = Join-Path $WorkDirectory "benchmark-output.parquet"
$Status = "failed"
$FailureMessage = $null
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$BuildDurationMs = $null
$InputRowCount = 0
$InputSizeBytes = 0L
$CommandResults = [System.Collections.Generic.List[object]]::new()
$OutputResults = [System.Collections.Generic.List[object]]::new()
$CliPath = $null

function Get-RelativePath {
    param([string]$Path)

    return $Path.Substring($ProjectRoot.Length).TrimStart("\", "/").Replace("\", "/")
}

function Write-SyntheticCsv {
    param(
        [string]$Destination,
        [long]$TargetBytes
    )

    $Encoding = [System.Text.UTF8Encoding]::new($false)
    $Stream = [System.IO.FileStream]::new(
        $Destination,
        [System.IO.FileMode]::Create,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None,
        1024 * 1024,
        [System.IO.FileOptions]::SequentialScan
    )
    $Writer = [System.IO.StreamWriter]::new($Stream, $Encoding, 1024 * 1024)
    $Rows = 0
    try {
        $Writer.WriteLine("id,name,amount,notes")
        while ($Stream.Length -lt $TargetBytes) {
            for ($Index = 0; $Index -lt 8192 -and $Stream.Length -lt $TargetBytes; $Index++) {
                $Rows++
                $Writer.Write("row-")
                $Writer.Write($Rows)
                $Writer.Write(",Synthetic name ")
                $Writer.Write($Rows)
                $Writer.Write(",123.45,Columnia benchmark deterministic payload 0123456789 abcdefghijklmnopqrstuvwxyz")
                $Writer.WriteLine()
            }
            $Writer.Flush()
        }
    }
    finally {
        $Writer.Dispose()
        $Stream.Dispose()
    }

    return [ordered]@{
        rowCount = $Rows
        sizeBytes = (Get-Item -LiteralPath $Destination).Length
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
}

function Invoke-MeasuredCli {
    param(
        [string]$Name,
        [string[]]$Arguments
    )

    $StdoutPath = Join-Path $EvidenceDirectory "$Name.stdout.log"
    $StderrPath = Join-Path $EvidenceDirectory "$Name.stderr.log"
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
    try {
        while (-not $Process.HasExited) {
            $Process.Refresh()
            $PeakWorkingSetBytes = [math]::Max($PeakWorkingSetBytes, [int64]$Process.WorkingSet64)
            if ($Stopwatch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
                try { $Process.Kill($true) } catch { $Process.Kill() }
                throw "$Name excedió el timeout de $TimeoutSeconds segundos."
            }
            Start-Sleep -Milliseconds 25
        }
        $Process.Refresh()
        $PeakWorkingSetBytes = [math]::Max($PeakWorkingSetBytes, [int64]$Process.PeakWorkingSet64)
        $Process.WaitForExit()
        $Stdout = $StdoutTask.Result
        $Stderr = $StderrTask.Result
        $ExitCode = $Process.ExitCode
    }
    finally {
        $Stopwatch.Stop()
        $Process.Dispose()
    }

    [System.IO.File]::WriteAllText($StdoutPath, $Stdout)
    [System.IO.File]::WriteAllText($StderrPath, $Stderr)
    if ($ExitCode -ne 0) {
        throw "$Name terminó con código $ExitCode."
    }
    if ($Stderr -match [regex]::Escape($ProjectRoot) -or $Stderr -match [regex]::Escape($EvidenceDirectory)) {
        throw "$Name expuso una ruta absoluta por stderr."
    }

    return [ordered]@{
        name = $Name
        status = "passed"
        exitCode = $ExitCode
        durationMs = [math]::Round($Stopwatch.Elapsed.TotalMilliseconds, 2)
        peakWorkingSetBytes = $PeakWorkingSetBytes
    }
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
        $BuildStdout = Join-Path $EvidenceDirectory "build.stdout.log"
        $BuildStderr = Join-Path $EvidenceDirectory "build.stderr.log"
        $PreviousErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = "Continue"
            & cargo build --quiet --bin columnia-cli 1> $BuildStdout 2> $BuildStderr
            if ($LASTEXITCODE -ne 0) {
                throw "No se pudo compilar columnia-cli."
            }
        }
        finally {
            $ErrorActionPreference = $PreviousErrorActionPreference
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

    $InputInfo = Write-SyntheticCsv -Destination $InputPath -TargetBytes $TargetBytes
    $InputRowCount = $InputInfo.rowCount
    $InputSizeBytes = $InputInfo.sizeBytes
    New-BenchmarkFiles

    $InputRelative = Get-RelativePath -Path $InputPath
    $RecipeRelative = Get-RelativePath -Path $RecipePath
    $RulesRelative = Get-RelativePath -Path $RulesPath
    $CsvOutputRelative = Get-RelativePath -Path $CsvOutputPath
    $ParquetOutputRelative = Get-RelativePath -Path $ParquetOutputPath

    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "inspect" -Arguments @(
        "inspect", "--input", $InputRelative
    )))
    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "validate" -Arguments @(
        "validate", "--input", $InputRelative, "--rules", $RulesRelative
    )))
    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "transform-csv" -Arguments @(
        "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
        "--output", $CsvOutputRelative, "--format", "csv"
    )))
    Assert-Output -Name "transform-csv" -Path $CsvOutputPath -Format "CSV"
    [void]$CommandResults.Add((Invoke-MeasuredCli -Name "transform-parquet" -Arguments @(
        "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
        "--output", $ParquetOutputRelative, "--format", "parquet"
    )))
    Assert-Output -Name "transform-parquet" -Path $ParquetOutputPath -Format "Parquet"
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
    $_ | Format-List * -Force | Out-String | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "failure.log") -Encoding utf8
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
    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        durationMs = [math]::Round($Timer.Elapsed.TotalMilliseconds, 2)
        targetMiB = $TargetMiB
        targetBytes = [int64]$TargetMiB * 1024 * 1024
        input = [ordered]@{
            fileName = "benchmark-input.csv"
            sizeBytes = $InputSizeBytes
            rowCount = $InputRowCount
            columnCount = 4
        }
        buildDurationMs = $BuildDurationMs
        commands = @($CommandResults)
        outputs = @($OutputResults)
        cleanupConfirmed = -not (Test-Path -LiteralPath $WorkDirectory)
        command = "columnia-cli"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Benchmark de datasets aprobado: $InputSizeBytes bytes, $InputRowCount filas, inspect/validate/transform CSV+Parquet medidos."
Write-Host "Evidencia: $EvidenceRelativePath"
