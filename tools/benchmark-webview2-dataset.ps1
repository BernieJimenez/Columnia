param(
    [ValidateRange(1, 500)]
    [int]$TargetMiB = 100,
    [ValidateRange(120, 900)]
    [int]$TimeoutSeconds = 900,
    [ValidateRange(1024, 4096)]
    [int]$MemoryWorkingSetBudgetMiB = 1536,
    [ValidateRange(1024, 4096)]
    [int]$MemoryPrivateBudgetMiB = 1024
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/performance-webview2/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$WorkDirectory = Join-Path $EvidenceDirectory "work"
$InputPath = Join-Path $WorkDirectory "webview2-benchmark-input.csv"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$CdpScriptPath = Join-Path $ProjectRoot "tools\probe-webview2-cdp.ps1"
$PowerShellExecutable = (Get-Command pwsh.exe -ErrorAction SilentlyContinue).Source
if ([string]::IsNullOrWhiteSpace($PowerShellExecutable)) {
    $PowerShellExecutable = (Get-Command powershell.exe -ErrorAction Stop).Source
}
$Status = "failed"
$FailureMessage = $null
$InputInfo = [ordered]@{ sizeBytes = 0L; rowCount = 0; columnCount = 4 }
$CdpEvidencePath = $null
$CdpSummary = $null
$CdpOutput = @()
$Timer = [System.Diagnostics.Stopwatch]::StartNew()

function Write-SanitizedEvidenceText {
    param(
        [string]$Path,
        [string]$Text
    )

    $Sanitized = $Text -replace [regex]::Escape($ProjectRoot), "[project-root]"
    $Sanitized = $Sanitized -replace [regex]::Escape($EvidenceDirectory), "[evidence]"
    [System.IO.File]::WriteAllText($Path, $Sanitized)
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
                $Writer.Write(",123.45,Columnia WebView2 benchmark deterministic payload 0123456789 abcdefghijklmnopqrstuvwxyz")
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
        fileName = "webview2-benchmark-input.csv"
        sizeBytes = (Get-Item -LiteralPath $Destination).Length
        rowCount = $Rows
        columnCount = 4
    }
}

function Read-CdpSummaryFromOutput {
    param([object[]]$Output)

    $EvidenceLine = @(
        $Output |
            ForEach-Object { [string]$_ } |
            Where-Object { $_ -match '^Evidencia:\s+(?<path>\.local/validation/webview2-cdp/[^\s]+)$' } |
            Select-Object -Last 1
    )
    if ($EvidenceLine.Count -eq 0) {
        return $null
    }
    $Line = [string]$EvidenceLine[0]
    if ($Line -notmatch '^Evidencia:\s+(?<path>\.local/validation/webview2-cdp/[^\s]+)$') {
        return $null
    }
    $script:CdpEvidencePath = $Matches.path
    $SummaryFile = Join-Path $ProjectRoot (($Matches.path) -replace "/", "\")
    $SummaryFile = Join-Path $SummaryFile "summary.json"
    if (-not (Test-Path -LiteralPath $SummaryFile -PathType Leaf)) {
        return $null
    }
    return Get-Content -LiteralPath $SummaryFile -Raw | ConvertFrom-Json
}

function Assert-PositiveNumber {
    param(
        $Value,
        [string]$Name
    )

    if ($null -eq $Value -or [double]$Value -le 0) {
        throw "$Name no reportó una duración positiva."
    }
}

New-Item -ItemType Directory -Path $WorkDirectory -Force | Out-Null

try {
    if (-not (Test-Path -LiteralPath $CdpScriptPath -PathType Leaf)) {
        throw "No se encontró tools/probe-webview2-cdp.ps1."
    }

    $InputInfo = Write-SyntheticCsv -Destination $InputPath -TargetBytes ([int64]$TargetMiB * 1024 * 1024)
    $CdpArguments = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $CdpScriptPath,
        "-TimeoutSeconds",
        [string]$TimeoutSeconds,
        "-RunNativeSelectors",
        "-NativeDatasetPath",
        $InputPath,
        "-NativeDatasetExpectedRowCount",
        [string]$InputInfo.rowCount,
        "-MemoryWorkingSetBudgetMiB",
        [string]$MemoryWorkingSetBudgetMiB,
        "-MemoryPrivateBudgetMiB",
        [string]$MemoryPrivateBudgetMiB
    )
    $CdpOutput = @(& $PowerShellExecutable @CdpArguments 2>&1)
    Write-SanitizedEvidenceText -Path (Join-Path $EvidenceDirectory "cdp.stdout.log") -Text ([string]::Join([Environment]::NewLine, @($CdpOutput | ForEach-Object { [string]$_ })))
    $CdpSummary = Read-CdpSummaryFromOutput -Output $CdpOutput
    if ($null -eq $CdpSummary) {
        throw "El probe WebView2 no produjo un summary.json identificable."
    }
    if ($CdpSummary.status -ne "supported" -or $CdpSummary.nativeSelectorsStatus -ne "passed") {
        throw "El probe WebView2 no aprobó la ejecución nativa del dataset grande."
    }

    $DatasetBenchmark = $CdpSummary.nativeSelectors.datasetBenchmark
    if ($null -eq $DatasetBenchmark -or $DatasetBenchmark.requested -ne $true) {
        throw "La evidencia CDP no contiene el benchmark nativo solicitado."
    }
    if ([int64]$DatasetBenchmark.sizeBytes -lt [int64]$InputInfo.sizeBytes -or
        [int64]$DatasetBenchmark.rowCount -ne [int64]$InputInfo.rowCount -or
        [int]$DatasetBenchmark.columnCount -ne $InputInfo.columnCount) {
        throw "WebView2 no confirmó las dimensiones del dataset grande."
    }
    Assert-PositiveNumber -Value $DatasetBenchmark.loadDurationMs -Name "load_dataset_selection"
    Assert-PositiveNumber -Value $DatasetBenchmark.pageDurationMs -Name "get_dataset_page"
    Assert-PositiveNumber -Value $DatasetBenchmark.transformDurationMs -Name "apply_transform_recipe"
    Assert-PositiveNumber -Value $DatasetBenchmark.exportDurationMs -Name "probe_export_dataset"
    if ($CdpSummary.performanceBudget.status -ne "within_budget" -or
        $CdpSummary.cleanupConfirmed -ne $true -or
        [int]$CdpSummary.processProfile.sampleCount -le 0) {
        throw "El benchmark nativo no confirmó memoria dentro del límite, muestras o cleanup."
    }
    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
    Write-SanitizedEvidenceText -Path (Join-Path $EvidenceDirectory "failure.log") -Text ($_ | Format-List * -Force | Out-String)
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
        input = $InputInfo
        cdpEvidence = $CdpEvidencePath
        cdp = if ($null -eq $CdpSummary) { $null } else {
            [ordered]@{
                status = $CdpSummary.status
                nativeSelectorsStatus = $CdpSummary.nativeSelectorsStatus
                datasetBenchmark = $CdpSummary.nativeSelectors.datasetBenchmark
                processProfile = $CdpSummary.processProfile
                performanceBudget = $CdpSummary.performanceBudget
                cleanupConfirmed = $CdpSummary.cleanupConfirmed
            }
        }
        cleanupConfirmed = -not (Test-Path -LiteralPath $WorkDirectory)
        command = "probe-webview2-cdp.ps1 -RunNativeSelectors -NativeDatasetPath"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Benchmark WebView2 de dataset grande aprobado: $($InputInfo.sizeBytes) bytes, $($InputInfo.rowCount) filas, carga/paginación/transformación/exportación y cleanup confirmados."
Write-Host "Evidencia: $EvidenceRelativePath"
