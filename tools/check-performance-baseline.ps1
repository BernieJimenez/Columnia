param(
    [string]$BaselinePath = "fixtures/performance/performance-baseline-v1.json",
    [switch]$SkipPackage
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ValidationRoot = Join-Path $ProjectRoot ".local\validation"
$BaselineAbsolutePath = if ([System.IO.Path]::IsPathRooted($BaselinePath)) {
    $BaselinePath
}
else {
    Join-Path $ProjectRoot ($BaselinePath -replace '/', '\')
}
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/performance-baseline/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace '/', '\')
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$Checks = [System.Collections.Generic.List[object]]::new()
$Status = "failed"
$FailureMessage = $null

function Read-Json {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No existe la evidencia requerida: $((Get-RelativePath $Path))."
    }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Get-RelativePath {
    param([string]$Path)

    $ResolvedProject = [System.IO.Path]::GetFullPath($ProjectRoot)
    $ResolvedPath = [System.IO.Path]::GetFullPath($Path)
    if ($ResolvedPath.StartsWith($ResolvedProject, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $ResolvedPath.Substring($ResolvedProject.Length).TrimStart('\', '/').Replace('\', '/')
    }
    return "external"
}

function Add-Check {
    param(
        [string]$Id,
        [string]$State,
        [object]$Observed,
        [object]$Budget,
        [string]$Source,
        [string]$Message
    )

    [void]$Checks.Add([ordered]@{
            id = $Id
            status = $State
            observed = $Observed
            budget = $Budget
            source = $Source
            message = $Message
        })
}

try {
    $Baseline = Read-Json -Path $BaselineAbsolutePath
    if ($Baseline.schemaVersion -ne 1) {
        throw "Baseline de rendimiento no soportado: schemaVersion $($Baseline.schemaVersion)."
    }

    $PerformanceSummaryPath = Join-Path $ValidationRoot "performance-summary\summary.json"
    $PerformanceSummary = Read-Json -Path $PerformanceSummaryPath
    $CdpCategory = @($PerformanceSummary.categories | Where-Object { $_.id -eq "cdp-native" })[0]
    $CdpBudget = if ($null -eq $CdpCategory) { $null } else { $CdpCategory.latest.performanceBudget }
    if ($null -eq $CdpBudget) {
        Add-Check -Id "cdp-memory" -State "unavailable" -Observed $null -Budget $Baseline.budgets.cdp -Source (Get-RelativePath $PerformanceSummaryPath) -Message "Falta una muestra CDP con performanceBudget."
    }
    else {
        $CdpHasValues = $null -ne $CdpBudget.peakWorkingSetBytes -and $null -ne $CdpBudget.peakPrivateMemoryBytes
        $CdpPassed = $CdpHasValues -and $CdpBudget.status -eq "within_budget" -and
            [bool]$CdpBudget.workingSetWithinBudget -and
            [bool]$CdpBudget.privateMemoryWithinBudget -and
            [int64]$CdpBudget.peakWorkingSetBytes -le [int64]$Baseline.budgets.cdp.workingSetBytes -and
            [int64]$CdpBudget.peakPrivateMemoryBytes -le [int64]$Baseline.budgets.cdp.privateMemoryBytes
        $CdpState = if ($CdpPassed) { "passed" } else { "failed" }
        $CdpMessage = if ($CdpPassed) { "Memoria CDP dentro del presupuesto." } else { "Memoria CDP fuera del presupuesto." }
        Add-Check -Id "cdp-memory" -State $CdpState `
            -Observed ([ordered]@{
                status = $CdpBudget.status
                peakWorkingSetBytes = [int64]$CdpBudget.peakWorkingSetBytes
                peakPrivateMemoryBytes = [int64]$CdpBudget.peakPrivateMemoryBytes
            }) -Budget $Baseline.budgets.cdp -Source (Get-RelativePath $PerformanceSummaryPath) `
            -Message $CdpMessage
    }

    $BenchmarkFile = Get-ChildItem -LiteralPath (Join-Path $ValidationRoot "performance-benchmark") -Recurse -File -Filter "summary.json" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $BenchmarkFile) {
        Add-Check -Id "dataset-benchmark" -State "unavailable" -Observed $null -Budget $Baseline.budgets.benchmark -Source $null -Message "Falta ejecutar perf:benchmark."
    }
    else {
        $Benchmark = Read-Json -Path $BenchmarkFile.FullName
        $Commands = @($Benchmark.commands)
        $CommandNames = @($Commands | ForEach-Object { [string]$_.name })
        $MissingCommands = @($Baseline.budgets.benchmark.requiredCommands | Where-Object { $_ -notin $CommandNames })
        $BenchmarkHasPeaks = $Commands.Count -gt 0 -and @($Commands | Where-Object { $null -eq $_.peakWorkingSetBytes }).Count -eq 0
        $PeakWorkingSet = if (-not $BenchmarkHasPeaks) { 0L } else { [int64](($Commands | ForEach-Object { [int64]$_.peakWorkingSetBytes } | Measure-Object -Maximum).Maximum) }
        $SustainedRuns = [int]$Benchmark.sustainedRuns
        $ProjectUpdateRuns = [int]$Benchmark.projectUpdateRuns
        $TransformCommands = @($Commands | Where-Object { $_.name -in @("transform-csv", "transform-parquet") })
        $ProjectSaveCommands = @($Commands | Where-Object { $_.name -in @("project-save", "project-save-update") })
        $ProjectInspectCommands = @($Commands | Where-Object { $_.name -in @("project-inspect", "project-inspect-reopen") })
        $ProjectExportCommands = @($Commands | Where-Object { $_.name -eq "project-export" })
        $MaxTransformDuration = if ($TransformCommands.Count -eq 0) { 0.0 } else { [double](($TransformCommands | ForEach-Object { [double]$_.durationMs } | Measure-Object -Maximum).Maximum) }
        $MaxProjectSaveDuration = if ($ProjectSaveCommands.Count -eq 0) { 0.0 } else { [double](($ProjectSaveCommands | ForEach-Object { [double]$_.durationMs } | Measure-Object -Maximum).Maximum) }
        $MaxProjectInspectDuration = if ($ProjectInspectCommands.Count -eq 0) { 0.0 } else { [double](($ProjectInspectCommands | ForEach-Object { [double]$_.durationMs } | Measure-Object -Maximum).Maximum) }
        $MaxProjectExportDuration = if ($ProjectExportCommands.Count -eq 0) { 0.0 } else { [double](($ProjectExportCommands | ForEach-Object { [double]$_.durationMs } | Measure-Object -Maximum).Maximum) }
        $DurationBudgets = $Baseline.budgets.benchmark.maxDurationsMs
        $DurationsWithinBudget = $null -ne $DurationBudgets -and
            $MaxTransformDuration -le [double]$DurationBudgets.transform -and
            $MaxProjectSaveDuration -le [double]$DurationBudgets.projectSave -and
            $MaxProjectInspectDuration -le [double]$DurationBudgets.projectInspect -and
            $MaxProjectExportDuration -le [double]$DurationBudgets.projectExport
        $BenchmarkPassed = $BenchmarkHasPeaks -and $Benchmark.status -eq "passed" -and
            [bool]$Benchmark.cleanupConfirmed -and
            [int]$Benchmark.targetMiB -ge [int]$Baseline.budgets.benchmark.minTargetMiB -and
            $SustainedRuns -ge [int]$Baseline.budgets.benchmark.minSustainedRuns -and
            $ProjectUpdateRuns -ge [int]$Baseline.budgets.benchmark.minProjectUpdateRuns -and
            $MissingCommands.Count -eq 0 -and
            $PeakWorkingSet -le [int64]$Baseline.budgets.benchmark.maxPeakWorkingSetBytes -and
            $DurationsWithinBudget
        $BenchmarkState = if ($BenchmarkPassed) { "passed" } else { "failed" }
        $BenchmarkMessage = if ($BenchmarkPassed) { "Benchmark de datasets dentro del contrato." } else { "Benchmark de datasets incompleto o fuera del presupuesto." }
        Add-Check -Id "dataset-benchmark" -State $BenchmarkState `
            -Observed ([ordered]@{
                status = $Benchmark.status
                targetMiB = [int]$Benchmark.targetMiB
                sustainedRuns = $SustainedRuns
                projectUpdateRuns = $ProjectUpdateRuns
                cleanupConfirmed = [bool]$Benchmark.cleanupConfirmed
                commandNames = $CommandNames
                missingCommands = $MissingCommands
                peakWorkingSetBytes = $PeakWorkingSet
                maxDurationsMs = [ordered]@{
                    transform = $MaxTransformDuration
                    projectSave = $MaxProjectSaveDuration
                    projectInspect = $MaxProjectInspectDuration
                    projectExport = $MaxProjectExportDuration
                }
                durationsWithinBudget = $DurationsWithinBudget
            }) -Budget $Baseline.budgets.benchmark -Source (Get-RelativePath $BenchmarkFile.FullName) `
            -Message $BenchmarkMessage
    }

    if ($SkipPackage) {
        Add-Check -Id "frontend-bundle" -State "not-requested" -Observed $null -Budget $Baseline.budgets.frontendBundle -Source $null -Message "Comprobación de Package omitida explícitamente."
    }
    else {
        $PackageFile = Get-ChildItem -LiteralPath $ValidationRoot -File -Filter "*-package.json" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($null -eq $PackageFile) {
            Add-Check -Id "frontend-bundle" -State "unavailable" -Observed $null -Budget $Baseline.budgets.frontendBundle -Source $null -Message "Falta ejecutar check.ps1 -Profile Package."
        }
        else {
            $Package = Read-Json -Path $PackageFile.FullName
            $Totals = $Package.frontendBundle.totals
            $BundleHasTotals = $null -ne $Totals -and $null -ne $Totals.rawBytes -and $null -ne $Totals.gzipBytes
            $BundlePassed = $BundleHasTotals -and $Package.status -eq "passed" -and
                $Package.frontendBundle.status -eq "passed" -and
                [int64]$Totals.rawBytes -le [int64]$Baseline.budgets.frontendBundle.rawBytes -and
                [int64]$Totals.gzipBytes -le [int64]$Baseline.budgets.frontendBundle.gzipBytes
            $BundleState = if ($BundlePassed) { "passed" } else { "failed" }
            $BundleMessage = if ($BundlePassed) { "Bundle frontend dentro del presupuesto." } else { "Bundle frontend fuera del presupuesto." }
            Add-Check -Id "frontend-bundle" -State $BundleState `
                -Observed ([ordered]@{
                    packageStatus = $Package.status
                    bundleStatus = $Package.frontendBundle.status
                    rawBytes = [int64]$Totals.rawBytes
                    gzipBytes = [int64]$Totals.gzipBytes
                }) -Budget $Baseline.budgets.frontendBundle -Source (Get-RelativePath $PackageFile.FullName) `
                -Message $BundleMessage
        }
    }

    $FailedChecks = @($Checks | Where-Object { $_.status -eq "failed" -or $_.status -eq "unavailable" })
    if ($FailedChecks.Count -gt 0) {
        $FailureMessage = ($FailedChecks | ForEach-Object { "$($_.id): $($_.message)" }) -join " "
        throw $FailureMessage
    }
    $Status = "passed"
}
catch {
    if ([string]::IsNullOrWhiteSpace($FailureMessage)) {
        $FailureMessage = $_.Exception.Message
    }
}

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
[ordered]@{
    schemaVersion = 1
    status = $Status
    generatedAt = $StartedAt.ToString("o")
    baseline = Get-RelativePath $BaselineAbsolutePath
    checks = @($Checks)
    error = $FailureMessage
    evidenceDirectory = $EvidenceRelativePath
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $SummaryPath -Encoding utf8

if ($Status -ne "passed") {
    Write-Error "Baseline de rendimiento falló: $FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Baseline de rendimiento aprobado: $EvidenceRelativePath"
