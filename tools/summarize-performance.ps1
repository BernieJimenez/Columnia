param()

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ValidationRoot = Join-Path $ProjectRoot ".local\validation"
$OutputDirectory = Join-Path $ValidationRoot "performance-summary"
$JsonPath = Join-Path $OutputDirectory "summary.json"
$CsvPath = Join-Path $OutputDirectory "summary.csv"
$StartedAt = [DateTimeOffset]::UtcNow
$FirstRenderBudgetMs = 3000

if (-not (Test-Path -LiteralPath $ValidationRoot -PathType Container)) {
    throw "No existe el directorio local de validación: .local/validation."
}

function Get-Number {
    param($Value)

    if ($null -eq $Value -or [string]::IsNullOrWhiteSpace([string]$Value)) {
        return $null
    }

    try {
        return [double]$Value
    }
    catch {
        return $null
    }
}

function Get-ObservedAt {
    param(
        $Document,
        [string]$EvidenceName
    )

    if ($null -ne $Document.startedAt) {
        try {
            return [DateTimeOffset]::Parse([string]$Document.startedAt).ToUniversalTime()
        }
        catch {
        }
    }

    if ($EvidenceName -match '(?<stamp>\d{8}T\d{6}Z)') {
        try {
            return [DateTimeOffset]::ParseExact(
                $Matches.stamp,
                "yyyyMMdd'T'HHmmss'Z'",
                [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::AssumeUniversal
            ).ToUniversalTime()
        }
        catch {
        }
    }

    return $null
}

function Get-RelativeEvidencePath {
    param([string]$Path)

    $Relative = $Path.Substring($ProjectRoot.Length).TrimStart('\', '/')
    return $Relative.Replace('\', '/')
}

function New-Metric {
    param(
        [string]$Name,
        [double]$ValueMs,
        [Nullable[double]]$BudgetMs = $null,
        [string]$State = "observed"
    )

    $BudgetState = "not-applicable"
    if ($null -ne $BudgetMs) {
        $BudgetState = if ($ValueMs -le [double]$BudgetMs) { "within-budget" } else { "over-budget" }
    }

    return [ordered]@{
        name = $Name
        valueMs = [math]::Round($ValueMs, 2)
        budgetMs = if ($null -eq $BudgetMs) { $null } else { [math]::Round([double]$BudgetMs, 2) }
        budgetState = $BudgetState
        state = $State
        deltaMs = $null
    }
}

function New-Sample {
    param(
        [string]$Category,
        [string]$Source,
        [Nullable[DateTimeOffset]]$ObservedAt,
        [string]$Status,
        [object[]]$Metrics,
        [object]$ProcessProfile = $null,
        [object]$PerformanceBudget = $null,
        [object]$NativeSustained = $null
    )

    [ordered]@{
        category = $Category
        source = $Source
        observedAt = if ($null -eq $ObservedAt) { $null } else { $ObservedAt.ToString("o") }
        status = $Status
        metrics = @($Metrics)
        processProfile = $ProcessProfile
        performanceBudget = $PerformanceBudget
        nativeSustained = $NativeSustained
    }
}

function Get-Document {
    param([string]$Path)

    try {
        return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    }
    catch {
        return $null
    }
}

function Get-ShellWebSample {
    param(
        $Document,
        [string]$Source,
        [string]$EvidenceName
    )

    $FirstRenderMs = Get-Number $Document.firstRenderMs
    if ($null -eq $FirstRenderMs -and $null -ne $Document.firstRender) {
        $FirstRenderMs = Get-Number $Document.firstRender.startTime
    }
    if ($null -eq $FirstRenderMs -and $null -ne $Document.performance) {
        $FirstRenderMs = Get-Number $Document.performance.firstRenderMs
    }
    if ($null -eq $FirstRenderMs) {
        return $null
    }

    $Status = if ($null -ne $Document.status) { [string]$Document.status } else { "observed" }
    $ObservedAt = Get-ObservedAt -Document $Document -EvidenceName $EvidenceName
    return New-Sample -Category "shell-web" -Source $Source -ObservedAt $ObservedAt -Status $Status -Metrics @(
        (New-Metric -Name "firstRenderMs" -ValueMs $FirstRenderMs -BudgetMs $FirstRenderBudgetMs -State "measured")
    )
}

function Get-CdpNativeSample {
    param(
        $Document,
        [string]$Source,
        [string]$EvidenceName
    )

    if ($null -eq $Document.playwright -or @($Document.playwright.pages).Count -eq 0) {
        return $null
    }

    $Page = @($Document.playwright.pages)[0]
    $FirstRender = $Page.firstRender
    $FirstRenderMs = if ($null -eq $FirstRender) { $null } else { Get-Number $FirstRender.startTime }
    $Metrics = [System.Collections.Generic.List[object]]::new()
    if ($null -ne $FirstRenderMs) {
        [void]$Metrics.Add((New-Metric -Name "firstRenderMs" -ValueMs $FirstRenderMs -BudgetMs $FirstRenderBudgetMs -State "observational"))
    }
    $ProbeDurationMs = Get-Number $Document.durationMs
    if ($null -ne $ProbeDurationMs) {
        [void]$Metrics.Add((New-Metric -Name "probeDurationMs" -ValueMs $ProbeDurationMs -State "observational"))
    }
    $ProjectPage = if ($null -eq $Document.projects) { $null } else { @($Document.projects.pages)[0] }
    $NativeEvidence = if ($null -eq $ProjectPage) { $null } else { $ProjectPage.nativeIpc }
    $NativeOperationDurationMs = if ($null -eq $NativeEvidence) {
        $null
    }
    else {
        Get-Number $NativeEvidence.nativeOperationDurationMs
    }
    if ($null -ne $NativeOperationDurationMs) {
        [void]$Metrics.Add((New-Metric -Name "nativeOperationDurationMs" -ValueMs $NativeOperationDurationMs -State "observational"))
    }
    $MaxTransformDurationMs = if ($null -eq $NativeEvidence) { $null } else { Get-Number $NativeEvidence.nativeSustainedTransformMaxMs }
    $MaxExportDurationMs = if ($null -eq $NativeEvidence) { $null } else { Get-Number $NativeEvidence.nativeSustainedExportMaxMs }
    if ($null -ne $MaxTransformDurationMs) {
        [void]$Metrics.Add((New-Metric -Name "nativeSustainedTransformMaxMs" -ValueMs $MaxTransformDurationMs -State "observational"))
    }
    if ($null -ne $MaxExportDurationMs) {
        [void]$Metrics.Add((New-Metric -Name "nativeSustainedExportMaxMs" -ValueMs $MaxExportDurationMs -State "observational"))
    }
    if ($Metrics.Count -eq 0) {
        return $null
    }

    $Status = if ($Document.playwrightStatus -eq "passed" -and $Document.status -eq "supported") { "observed" } else { "unavailable" }
    $ObservedAt = Get-ObservedAt -Document $Document -EvidenceName $EvidenceName
    $NativeSustained = if ($null -eq $NativeEvidence) { $null } else {
        [ordered]@{
            runs = [int]$NativeEvidence.nativeSustainedRuns
            maxTransformDurationMs = if ($null -eq $MaxTransformDurationMs) { $null } else { [math]::Round($MaxTransformDurationMs, 2) }
            maxExportDurationMs = if ($null -eq $MaxExportDurationMs) { $null } else { [math]::Round($MaxExportDurationMs, 2) }
        }
    }
    return New-Sample -Category "cdp-native" -Source $Source -ObservedAt $ObservedAt -Status $Status -Metrics @($Metrics) -ProcessProfile $Document.processProfile -PerformanceBudget $Document.performanceBudget -NativeSustained $NativeSustained
}

function Get-DesktopSmokeSample {
    param(
        $Document,
        [string]$Source,
        [string]$EvidenceName
    )

    if ($null -eq $Document.milestones) {
        return $null
    }

    $Metrics = [System.Collections.Generic.List[object]]::new()
    $Definitions = @(
        @{ Name = "smokeDurationMs"; Value = Get-Number $Document.durationMs },
        @{ Name = "viteReadyMs"; Value = Get-Number $Document.milestones.viteReady.elapsedMs },
        @{ Name = "desktopProcessReadyMs"; Value = Get-Number $Document.milestones.desktopProcessReady.elapsedMs },
        @{ Name = "windowVisibleMs"; Value = Get-Number $Document.milestones.windowVisible.elapsedMs }
    )
    foreach ($Definition in $Definitions) {
        if ($null -ne $Definition.Value) {
            [void]$Metrics.Add((New-Metric -Name $Definition.Name -ValueMs $Definition.Value -State "observed"))
        }
    }
    if ($Metrics.Count -eq 0) {
        return $null
    }

    $Status = if ($null -ne $Document.status) { [string]$Document.status } else { "observed" }
    $ObservedAt = Get-ObservedAt -Document $Document -EvidenceName $EvidenceName
    return New-Sample -Category "desktop-smoke" -Source $Source -ObservedAt $ObservedAt -Status $Status -Metrics @($Metrics)
}

$CategorySamples = [ordered]@{
    "shell-web" = [System.Collections.Generic.List[object]]::new()
    "cdp-native" = [System.Collections.Generic.List[object]]::new()
    "desktop-smoke" = [System.Collections.Generic.List[object]]::new()
}

$EvidenceFiles = Get-ChildItem -LiteralPath $ValidationRoot -Recurse -File -Filter "summary.json" |
    Where-Object { $_.FullName -notlike "$OutputDirectory\*" }

foreach ($EvidenceFile in $EvidenceFiles) {
    $Document = Get-Document -Path $EvidenceFile.FullName
    if ($null -eq $Document) {
        continue
    }

    $Source = Get-RelativeEvidencePath -Path $EvidenceFile.FullName
    $EvidenceName = $Source.Replace('/', '\')
    $Sample = $null
    if ($EvidenceName -match '^\.local\\validation\\webview2-cdp\\') {
        $Sample = Get-CdpNativeSample -Document $Document -Source $Source -EvidenceName $EvidenceName
    }
    elseif ($EvidenceName -match '^\.local\\validation\\desktop-smoke\\') {
        $Sample = Get-DesktopSmokeSample -Document $Document -Source $Source -EvidenceName $EvidenceName
    }
    elseif ($EvidenceName -match '^\.local\\validation\\(shell-web|web-shell|performance)\\') {
        $Sample = Get-ShellWebSample -Document $Document -Source $Source -EvidenceName $EvidenceName
    }

    if ($null -ne $Sample) {
        [void]$CategorySamples[$Sample.category].Add($Sample)
    }
}

$CategoryOutput = [System.Collections.Generic.List[object]]::new()
$CsvRows = [System.Collections.Generic.List[object]]::new()

foreach ($Category in @($CategorySamples.Keys)) {
    $Samples = @($CategorySamples[$Category] | Sort-Object {
            if ($null -eq $_.observedAt) { "0001-01-01T00:00:00Z" } else { $_.observedAt }
        })
    $PreviousByMetric = @{}
    foreach ($Sample in $Samples) {
        foreach ($Metric in @($Sample.metrics)) {
            $Previous = $PreviousByMetric[$Metric.name]
            if ($null -ne $Previous -and $null -ne $Metric.valueMs) {
                $Metric.deltaMs = [math]::Round(([double]$Metric.valueMs - [double]$Previous), 2)
            }
            $PreviousByMetric[$Metric.name] = $Metric.valueMs
            [void]$CsvRows.Add([pscustomobject][ordered]@{
                category = $Category
                metric = $Metric.name
                observedAt = $Sample.observedAt
                status = $Sample.status
                valueMs = $Metric.valueMs
                budgetMs = $Metric.budgetMs
                budgetState = $Metric.budgetState
                metricState = $Metric.state
                deltaMs = $Metric.deltaMs
                source = $Sample.source
            })
        }
    }

    $Latest = if ($Samples.Count -eq 0) { $null } else { $Samples[-1] }
    $CategoryState = "not-observed"
    if ($null -ne $Latest) {
        $CategoryState = if ($Category -eq "cdp-native") { "observational" } elseif ($Latest.status -eq "passed") { "passed" } else { $Latest.status }
    }
    [void]$CategoryOutput.Add([ordered]@{
        id = $Category
        state = $CategoryState
        sampleCount = $Samples.Count
        latest = $Latest
        samples = @($Samples)
    })
}

$Output = [ordered]@{
    schemaVersion = 1
    generatedAt = $StartedAt.ToString("o")
    sourceRoot = ".local/validation"
    firstRenderBudgetMs = $FirstRenderBudgetMs
    categories = @($CategoryOutput)
    rowCount = $CsvRows.Count
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$Output | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $JsonPath -Encoding utf8
@($CsvRows) | ConvertTo-Csv -NoTypeInformation | Set-Content -LiteralPath $CsvPath -Encoding utf8

Write-Host "Resumen de rendimiento generado: .local/validation/performance-summary/summary.json"
Write-Host "CSV generado: .local/validation/performance-summary/summary.csv"
foreach ($Category in @($CategoryOutput)) {
    Write-Host ("{0}: {1} muestra(s), estado {2}" -f $Category.id, $Category.sampleCount, $Category.state)
}
