param(
    [ValidateRange(1, 500)]
    [int]$TargetMiB = 100,
    [ValidateRange(30, 900)]
    [int]$TimeoutSeconds = 900
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$ReferenceRoot = Resolve-Path (Join-Path $ProjectRoot "..\dataprepv1.1")
$ReferenceScript = Join-Path $ReferenceRoot "tools\benchmark_dataset.py"
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/i1-benchmark/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$WorkDirectory = Join-Path $EvidenceDirectory "work"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$InputPath = Join-Path $WorkDirectory "benchmark-input.csv"
$Status = "failed"
$FailureMessage = $null
$CliPath = $null
$ColumniaResult = $null
$DataprepResult = $null
$InputInfo = [ordered]@{ sizeBytes = 0; rowCount = 0; columnCount = 4 }

function Write-SanitizedText {
    param([string]$Path, [string]$Text)

    $Sanitized = $Text -replace [regex]::Escape($ProjectRoot), "[project-root]"
    $Sanitized = $Sanitized -replace [regex]::Escape($EvidenceDirectory), "[evidence]"
    [System.IO.File]::WriteAllText($Path, $Sanitized)
}

function Write-SyntheticCsv {
    param([string]$Destination, [long]$TargetBytes)

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
        sizeBytes = (Get-Item -LiteralPath $Destination).Length
        rowCount = $Rows
        columnCount = 4
    }
}

function Invoke-MeasuredProcess {
    param(
        [string]$Name,
        [string]$FileName,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$EvidenceTag
    )

    $StdoutPath = Join-Path $EvidenceDirectory "$EvidenceTag.stdout.log"
    $StderrPath = Join-Path $EvidenceDirectory "$EvidenceTag.stderr.log"
    $StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = $FileName
    $StartInfo.WorkingDirectory = $WorkingDirectory
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
            if ($_ -notmatch '[\s"]') { $_ }
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

    Write-SanitizedText -Path $StdoutPath -Text $Stdout
    Write-SanitizedText -Path $StderrPath -Text $Stderr
    if ($ExitCode -ne 0) {
        throw "$Name terminó con código $ExitCode."
    }
    if ($Stdout -match [regex]::Escape($ProjectRoot) -or $Stderr -match [regex]::Escape($ProjectRoot)) {
        throw "$Name expuso una ruta absoluta del workspace."
    }

    return [ordered]@{
        engine = $Name
        status = "passed"
        durationMs = [math]::Round($Stopwatch.Elapsed.TotalMilliseconds, 2)
        peakWorkingSetBytes = $PeakWorkingSetBytes
        stdout = $Stdout
    }
}

New-Item -ItemType Directory -Path $WorkDirectory -Force | Out-Null

try {
    $BuildStdout = Join-Path $EvidenceDirectory "build.stdout.log"
    $BuildStderr = Join-Path $EvidenceDirectory "build.stderr.log"
    Push-Location $TauriRoot
    try {
        $PreviousErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = "Continue"
            & cargo build --quiet --bin columnia-cli 1> $BuildStdout 2> $BuildStderr
            if ($LASTEXITCODE -ne 0) { throw "No se pudo compilar columnia-cli." }
        }
        finally {
            $ErrorActionPreference = $PreviousErrorActionPreference
        }
    }
    finally {
        Pop-Location
    }

    $ExecutableName = if ($env:OS -eq "Windows_NT") { "columnia-cli.exe" } else { "columnia-cli" }
    $CliPath = Join-Path $TauriRoot "target\debug\$ExecutableName"
    if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
        throw "La compilación no produjo columnia-cli."
    }
    if (-not (Test-Path -LiteralPath $ReferenceScript -PathType Leaf)) {
        throw "No se encontró benchmark_dataset.py en dataprepv1.1."
    }
    $ReferencePython = Join-Path $ReferenceRoot ".venv\Scripts\python.exe"
    $Python = if (Test-Path -LiteralPath $ReferencePython -PathType Leaf) {
        $ReferencePython
    }
    else {
        (Get-Command python -ErrorAction Stop).Source
    }

    $InputInfo = Write-SyntheticCsv -Destination $InputPath -TargetBytes ([int64]$TargetMiB * 1024 * 1024)
    $InputRelative = $InputPath.Substring($ProjectRoot.Length).TrimStart("\", "/").Replace("\", "/")
    $ColumniaResult = Invoke-MeasuredProcess -Name "columnia" -FileName $CliPath -WorkingDirectory $ProjectRoot -EvidenceTag "columnia" -Arguments @(
        "inspect", "--input", $InputRelative
    )
    $ColumniaOutput = $ColumniaResult.stdout | ConvertFrom-Json
    if ([int64]$ColumniaOutput.rowCount -ne [int64]$InputInfo.rowCount -or [int]$ColumniaOutput.columnCount -ne $InputInfo.columnCount) {
        throw "Columnia no confirmó las dimensiones del input compartido."
    }

    $DataprepResult = Invoke-MeasuredProcess -Name "dataprepv1.1" -FileName $Python -WorkingDirectory $ReferenceRoot -EvidenceTag "dataprepv1-1" -Arguments @(
        "-u", $ReferenceScript, "--input", $InputPath
    )
    $DataprepOutput = $DataprepResult.stdout | ConvertFrom-Json
    if ([int64]$DataprepOutput.rows -ne [int64]$InputInfo.rowCount -or [int]$DataprepOutput.columns -ne $InputInfo.columnCount) {
        throw "dataprepv1.1 no confirmó las dimensiones del input compartido."
    }
    if ([int64]$DataprepOutput.input_bytes -ne [int64]$InputInfo.sizeBytes) {
        throw "dataprepv1.1 no leyó el mismo input byte a byte."
    }
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
    $FailureLog = $_ | Format-List * -Force | Out-String
    Write-SanitizedText -Path (Join-Path $EvidenceDirectory "failure.log") -Text $FailureLog
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

    $ColumniaSummary = if ($null -eq $ColumniaResult) { $null } else {
        [ordered]@{
            engine = $ColumniaResult.engine
            status = $ColumniaResult.status
            durationMs = $ColumniaResult.durationMs
            peakWorkingSetBytes = $ColumniaResult.peakWorkingSetBytes
        }
    }
    $DataprepSummary = if ($null -eq $DataprepResult) { $null } else {
        [ordered]@{
            engine = $DataprepResult.engine
            status = $DataprepResult.status
            durationMs = $DataprepResult.durationMs
            peakWorkingSetBytes = $DataprepResult.peakWorkingSetBytes
        }
    }
    $DurationRatio = if ($ColumniaSummary -and $DataprepSummary -and $DataprepSummary.durationMs -gt 0) {
        [math]::Round($ColumniaSummary.durationMs / $DataprepSummary.durationMs, 3)
    } else { $null }
    $MemoryRatio = if ($ColumniaSummary -and $DataprepSummary -and $DataprepSummary.peakWorkingSetBytes -gt 0) {
        [math]::Round($ColumniaSummary.peakWorkingSetBytes / $DataprepSummary.peakWorkingSetBytes, 3)
    } else { $null }
    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        targetMiB = $TargetMiB
        comparisonScope = "same deterministic CSV; Columnia inspect versus dataprepv1.1 load/page/sample"
        input = [ordered]@{
            fileName = "benchmark-input.csv"
            sizeBytes = [int64]$InputInfo.sizeBytes
            rowCount = [int64]$InputInfo.rowCount
            columnCount = [int]$InputInfo.columnCount
        }
        runs = @($ColumniaSummary, $DataprepSummary)
        comparison = [ordered]@{
            durationRatioColumniaOverDataprep = $DurationRatio
            peakWorkingSetRatioColumniaOverDataprep = $MemoryRatio
            directional = $true
        }
        cleanupConfirmed = -not (Test-Path -LiteralPath $WorkDirectory)
        reference = "../dataprepv1.1/tools/benchmark_dataset.py"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Benchmark I1 aprobado: $($InputInfo.sizeBytes) bytes, $($InputInfo.rowCount) filas, Columnia contra dataprepv1.1 en tiempo y RAM."
Write-Host "Evidencia: $EvidenceRelativePath"
