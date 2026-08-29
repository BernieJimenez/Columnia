[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$SkipPackage,
    [switch]$WithUpdater,
    [ValidateSet("nsis", "msi")]
    [string]$UpdaterArtifactKind = "nsis",
    [string]$UpdaterTarget = "windows-x86_64",
    [string]$UpdaterNotesPath,
    [string]$ReportPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$StartedAt = [DateTimeOffset]::UtcNow
$Stamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$Profile = if ($SkipPackage) { "Release" } else { "Package" }
$ProjectVersion = (Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json).version
$EvidenceRelativePath = ".local/validation/release-orchestration/$Stamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    Join-Path $EvidenceDirectory "summary.json"
}
elseif ([System.IO.Path]::IsPathRooted($ReportPath)) {
    $ReportPath
}
else {
    Join-Path $ProjectRoot $ReportPath
}
$Steps = [System.Collections.Generic.List[object]]::new()
$Status = "failed"
$FailureMessage = $null
$UpdaterConfigPath = Join-Path $EvidenceDirectory "tauri-updater-config.json"
$UpdaterManifestPath = Join-Path $EvidenceDirectory "updater-manifest.json"
$UpdaterInventoryPath = Join-Path $EvidenceDirectory "updater-integrity.json"
$UpdaterEndpoint = $null
$UpdaterAssetBaseUrl = $null
$UpdaterEvidence = [ordered]@{
    status = if ($WithUpdater) { "pending" } else { "not-requested" }
    target = if ($WithUpdater) { $UpdaterTarget } else { $null }
    artifactKind = if ($WithUpdater) { $UpdaterArtifactKind } else { $null }
    endpoint = $null
    assetBaseUrl = $null
    manifestPath = if ($WithUpdater) { "$EvidenceRelativePath/updater-manifest.json" } else { $null }
    inventoryPath = if ($WithUpdater) { "$EvidenceRelativePath/updater-integrity.json" } else { $null }
    artifact = $null
}

function Get-GitState {
    [ordered]@{
        commit = (git -C $ProjectRoot rev-parse HEAD).Trim()
        branch = (git -C $ProjectRoot branch --show-current).Trim()
        dirty = @(git -C $ProjectRoot status --porcelain).Count -gt 0
    }
}

function Invoke-ReleaseStep {
    param(
        [string]$Label,
        [scriptblock]$Command
    )

    Write-Host "[$Label]"
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $stepStatus = "failed"
    $stepError = $null
    try {
        Push-Location $ProjectRoot
        try {
            & $Command
            if ($LASTEXITCODE -ne 0) {
                throw "$Label terminó con código $LASTEXITCODE."
            }
        }
        finally {
            Pop-Location
        }
        $stepStatus = "passed"
    }
    catch {
        $stepError = $_.Exception.Message
        throw
    }
    finally {
        $timer.Stop()
        [void]$Steps.Add([ordered]@{
            label = $Label
            status = $stepStatus
            durationMs = $timer.ElapsedMilliseconds
            error = $stepError
        })
    }
}

function Write-ReleaseSummary {
    param(
        [object]$GitState
    )

    New-Item -ItemType Directory -Path (Split-Path -Parent $SummaryPath) -Force | Out-Null
    [ordered]@{
        schemaVersion = 1
        status = $Status
        dryRun = [bool]$DryRun
        packageRequested = -not [bool]$SkipPackage
        profile = $Profile
        startedAt = $StartedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        git = $GitState
        steps = @($Steps)
        updater = $UpdaterEvidence
        error = $FailureMessage
        publishing = [ordered]@{
            attempted = $false
            note = "Este orquestador no crea tags, no publica artefactos y no contacta servicios remotos."
        }
        evidenceDirectory = $EvidenceRelativePath
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

$Git = $null
try {
    $Git = Get-GitState
    if ([string]::IsNullOrWhiteSpace($Git.branch)) {
        throw "El release requiere una rama Git explícita; no se acepta HEAD separado."
    }
    if ($Git.dirty) {
        throw "El release requiere un árbol Git limpio; registra primero todos los cambios."
    }
    if ($WithUpdater -and $SkipPackage) {
        throw "-WithUpdater requiere empaquetado; no puede combinarse con -SkipPackage."
    }
    if ($WithUpdater) {
        $UpdaterEndpoint = [Environment]::GetEnvironmentVariable("COLUMNIA_UPDATER_ENDPOINT", "Process")
        $UpdaterAssetBaseUrl = [Environment]::GetEnvironmentVariable("COLUMNIA_UPDATER_ASSET_BASE_URL", "Process")
        $SigningKey = [Environment]::GetEnvironmentVariable("TAURI_SIGNING_PRIVATE_KEY", "Process")
        if ([string]::IsNullOrWhiteSpace($UpdaterEndpoint)) {
            throw "-WithUpdater requiere COLUMNIA_UPDATER_ENDPOINT."
        }
        if ([string]::IsNullOrWhiteSpace($UpdaterAssetBaseUrl)) {
            throw "-WithUpdater requiere COLUMNIA_UPDATER_ASSET_BASE_URL para construir las URLs de artefactos."
        }
        $EndpointUri = $null
        if (-not [Uri]::TryCreate($UpdaterEndpoint, [UriKind]::Absolute, [ref]$EndpointUri) -or $EndpointUri.Scheme -ne "https") {
            throw "COLUMNIA_UPDATER_ENDPOINT debe ser una URL HTTPS absoluta."
        }
        $AssetUri = $null
        if (-not [Uri]::TryCreate($UpdaterAssetBaseUrl, [UriKind]::Absolute, [ref]$AssetUri) -or $AssetUri.Scheme -ne "https") {
            throw "COLUMNIA_UPDATER_ASSET_BASE_URL debe ser una URL HTTPS absoluta."
        }
        if ([string]::IsNullOrWhiteSpace($SigningKey)) {
            throw "-WithUpdater requiere TAURI_SIGNING_PRIVATE_KEY; la clave privada nunca se guarda en el repositorio."
        }
        $UpdaterEvidence.endpoint = $UpdaterEndpoint
        $UpdaterEvidence.assetBaseUrl = $UpdaterAssetBaseUrl
        New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
        $TauriConfig = Get-Content -LiteralPath (Join-Path $ProjectRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
        $TauriConfig.bundle.createUpdaterArtifacts = $true
        $TauriConfig.plugins.updater.endpoints = @($UpdaterEndpoint)
        $TauriConfig | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $UpdaterConfigPath -Encoding utf8
    }

    Invoke-ReleaseStep "Toolchains" {
        & node tools/check-toolchains.mjs
    }
    Invoke-ReleaseStep "Documentation" {
        & node tools/check-documentation.mjs
    }
    Invoke-ReleaseStep "IPC inventory" {
        & node tools/check-ipc-inventory.mjs
    }
    Invoke-ReleaseStep "Release profile" {
        if ($WithUpdater) {
            & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tools\check.ps1") -Profile $Profile -TauriConfigPath $UpdaterConfigPath
        }
        else {
            & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tools\check.ps1") -Profile $Profile
        }
    }
    Invoke-ReleaseStep "CLI smoke" {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tools\smoke-cli.ps1")
    }
    Invoke-ReleaseStep "Release accessibility evidence" {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tools\capture-release-evidence.ps1") -SkipBuild
    }
    Invoke-ReleaseStep "Release accessibility baseline" {
        & npm.cmd run accessibility:release:check
    }
    Invoke-ReleaseStep "WebView2 large dataset benchmark" {
        & npm.cmd run perf:webview2
    }
    Invoke-ReleaseStep "Performance baseline" {
        & npm.cmd run perf:check
    }
    if ($WithUpdater) {
        Invoke-ReleaseStep "Updater manifest and integrity" {
            $ManifestArguments = @(
                "tools/generate-updater-manifest.mjs",
                "--bundle-root", (Join-Path $ProjectRoot "src-tauri\target\release\bundle"),
                "--output", $UpdaterManifestPath,
                "--inventory-output", $UpdaterInventoryPath,
                "--base-url", $UpdaterAssetBaseUrl,
                "--target", $UpdaterTarget,
                "--version", $ProjectVersion,
                "--artifact-kind", $UpdaterArtifactKind
            )
            $UpdaterBundleDirectory = Join-Path $ProjectRoot "src-tauri\target\release\bundle\$UpdaterArtifactKind"
            $UpdaterCandidates = @(
                Get-ChildItem -LiteralPath $UpdaterBundleDirectory -File -ErrorAction SilentlyContinue |
                    Where-Object { $_.Name -match [Regex]::Escape($ProjectVersion) -and $_.Extension.ToLowerInvariant() -eq $(if ($UpdaterArtifactKind -eq "msi") { ".msi" } else { ".exe" }) }
            )
            if ($UpdaterCandidates.Count -ne 1) {
                throw "Se esperaba exactamente un instalador $UpdaterArtifactKind de la versión $ProjectVersion; se encontraron $($UpdaterCandidates.Count)."
            }
            $ManifestArguments += @("--artifact", $UpdaterCandidates[0].FullName)
            if (-not [string]::IsNullOrWhiteSpace($UpdaterNotesPath)) {
                $ManifestArguments += @("--notes-file", $UpdaterNotesPath)
            }
            & node @ManifestArguments
            $UpdaterInventory = Get-Content -LiteralPath $UpdaterInventoryPath -Raw | ConvertFrom-Json
            & node tools/check-updater-manifest.mjs --manifest $UpdaterManifestPath --inventory $UpdaterInventoryPath
            $UpdaterEvidence.status = $UpdaterInventory.status
            $UpdaterEvidence.artifact = $UpdaterInventory.artifact
        }
    }

    $Status = "passed"
    if ($DryRun) {
        Write-Host "Release dry-run aprobado: gates locales ejecutados; no se publicaron artefactos."
    }
    else {
        Write-Host "Release local aprobado: gates ejecutados; la publicación sigue siendo manual."
    }
}
catch {
    $FailureMessage = $_.Exception.Message
    if ($WithUpdater -and $UpdaterEvidence.status -eq "pending") {
        $UpdaterEvidence.status = "failed"
    }
    Write-Error "Orquestador de release: $FailureMessage"
}
finally {
    if ($null -eq $Git) {
        try { $Git = Get-GitState } catch { $Git = [ordered]@{ commit = $null; branch = $null; dirty = $null } }
    }
    Write-ReleaseSummary -GitState $Git
}

Write-Host "Reporte: $SummaryPath"
if ($Status -ne "passed") {
    exit 1
}
exit 0
