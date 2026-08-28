param(
    [ValidateSet("Fast", "Full", "Release", "Package")]
    [string]$Profile = "Fast",

    [string]$ReportPath,

    [string]$TauriConfigPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$StartedAt = [DateTimeOffset]::UtcNow
$RunTimer = [System.Diagnostics.Stopwatch]::StartNew()
$StepResults = [System.Collections.Generic.List[object]]::new()
$ValidationStatus = "failed"
$FailureMessage = $null
$Commit = (git -C $ProjectRoot rev-parse HEAD).Trim()
$ShortCommit = (git -C $ProjectRoot rev-parse --short HEAD).Trim()
$Branch = (git -C $ProjectRoot branch --show-current).Trim()
$TreeDirty = @(git -C $ProjectRoot status --porcelain).Count -gt 0
$ProjectVersion = (Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json).version
$RunStamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$ReleaseLike = $Profile -in @("Release", "Package")
$TauriConfigArguments = @()
if (-not [string]::IsNullOrWhiteSpace($TauriConfigPath)) {
    if (-not (Test-Path -LiteralPath $TauriConfigPath -PathType Leaf)) {
        throw "La configuración Tauri alternativa no existe: $TauriConfigPath"
    }
    $TauriConfigArguments = @("--config", (Resolve-Path -LiteralPath $TauriConfigPath).Path)
}
$SbomRelativePath = ".local/validation/columnia.cdx.json"
$SbomPath = Join-Path $ProjectRoot ".local\validation\columnia.cdx.json"
$SbomEvidence = [ordered]@{
    status = if ($ReleaseLike) { "pending" } else { "not-requested" }
    path = if ($ReleaseLike) { $SbomRelativePath } else { $null }
    sha256 = $null
    componentCount = $null
}
$FrontendBundleRelativePath = ".local/validation/$RunStamp-$ShortCommit-frontend-bundle.json"
$FrontendBundlePath = Join-Path $ProjectRoot ($FrontendBundleRelativePath.Replace("/", "\"))
$FrontendBundleEvidence = [ordered]@{
    status = "pending"
    path = $FrontendBundleRelativePath
    sha256 = $null
    fileCount = $null
    totals = $null
    limits = $null
}
$PackageArtifactsRelativePath = ".local/validation/$RunStamp-$ShortCommit-package-artifacts.json"
$PackageArtifactsPath = Join-Path $ProjectRoot ($PackageArtifactsRelativePath.Replace("/", "\"))
$PackageSnapshotPath = Join-Path $ProjectRoot ".local\validation\$RunStamp-$ShortCommit-package-snapshot.json"
$PackageArtifactsEvidence = [ordered]@{
    status = if ($Profile -eq "Package") { "pending" } else { "not-requested" }
    path = if ($Profile -eq "Package") { $PackageArtifactsRelativePath } else { $null }
    sha256 = $null
    artifactCount = $null
    artifacts = @()
}

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $ProjectRoot ".local\validation\$RunStamp-$ShortCommit-$($Profile.ToLowerInvariant()).json"
}
elseif (-not [System.IO.Path]::IsPathRooted($ReportPath)) {
    $ReportPath = Join-Path $ProjectRoot $ReportPath
}

function Get-ToolVersion {
    param([scriptblock]$Command)

    try {
        $Output = @(& $Command 2>&1)
        if ($LASTEXITCODE -ne 0 -or $Output.Count -eq 0) {
            return "unavailable"
        }
        return $Output[0].ToString().Trim()
    }
    catch {
        return "unavailable"
    }
}

function Get-Sha256 {
    param([string]$Path)

    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $stream.Dispose()
        $algorithm.Dispose()
    }
}

function Get-LockfileFingerprint {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return [ordered]@{
            status = "unavailable"
            sha256 = $null
        }
    }

    try {
        return [ordered]@{
            status = "available"
            sha256 = Get-Sha256 $Path
        }
    }
    catch {
        return [ordered]@{
            status = "unavailable"
            sha256 = $null
        }
    }
}

$ToolVersions = [ordered]@{
    powershell = $PSVersionTable.PSVersion.ToString()
    node = Get-ToolVersion { node --version }
    npm = Get-ToolVersion { npm --version }
    rustc = Get-ToolVersion { rustc --version }
    cargo = Get-ToolVersion { cargo --version }
}

$RuntimeEnvironment = [ordered]@{
    operatingSystem = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription.Trim()
    architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
}

$LockfileFingerprints = [ordered]@{
    packageLock = Get-LockfileFingerprint (Join-Path $ProjectRoot "package-lock.json")
    cargoLock = Get-LockfileFingerprint (Join-Path $TauriRoot "Cargo.lock")
}

function Invoke-Checked {
    param(
        [string]$Label,
        [string]$WorkingDirectory,
        [scriptblock]$Command
    )

    Write-Host "[$Label]"
    $StepTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $StepStatus = "failed"
    $StepError = $null
    Push-Location $WorkingDirectory
    try {
        & $Command
        if ($LASTEXITCODE -ne 0) {
            throw "$Label falló con código $LASTEXITCODE."
        }
        $StepStatus = "passed"
    }
    catch {
        $StepError = $_.Exception.Message
        throw
    }
    finally {
        $StepTimer.Stop()
        Pop-Location
        $StepResults.Add([ordered]@{
            label = $Label
            status = $StepStatus
            durationMs = $StepTimer.ElapsedMilliseconds
            error = $StepError
        })
    }
}

try {
    Invoke-Checked "Documentation" $ProjectRoot {
        & node tools/check-documentation.mjs
    }
    Invoke-Checked "IPC inventory" $ProjectRoot {
        & node tools/check-ipc-inventory.mjs
    }
    Invoke-Checked "Toolchains" $ProjectRoot {
        & node tools/check-toolchains.mjs
    }
    Invoke-Checked "Repository governance" $ProjectRoot {
        & (Join-Path $ProjectRoot "tools\check-governance.ps1")
    }
    Invoke-Checked "Rust format" $TauriRoot { cargo fmt -- --check }
    Invoke-Checked "Rust check" $TauriRoot { cargo check }
    Invoke-Checked "Frontend tests" $ProjectRoot { npm test -- --run }
    if ($Profile -in @("Full", "Release", "Package")) {
        Invoke-Checked "Frontend coverage" $ProjectRoot { npm run test:coverage }
    }
    Invoke-Checked "Frontend build" $ProjectRoot { npm run build }
    Invoke-Checked "Frontend bundle budget" $ProjectRoot {
        node tools/check-bundle.mjs budget --dist dist --output $FrontendBundlePath
        if ($LASTEXITCODE -ne 0 -and (Test-Path -LiteralPath $FrontendBundlePath -PathType Leaf)) {
            $FailedBundle = Get-Content -LiteralPath $FrontendBundlePath -Raw | ConvertFrom-Json
            throw "Presupuesto frontend excedido: $(@($FailedBundle.violations) -join ' ') Divide el bundle o revisa explícitamente los límites."
        }
    }
    $FrontendBundleDocument = Get-Content -LiteralPath $FrontendBundlePath -Raw | ConvertFrom-Json
    $FrontendBundleEvidence.status = $FrontendBundleDocument.status
    $FrontendBundleEvidence.sha256 = Get-Sha256 $FrontendBundlePath
    $FrontendBundleEvidence.fileCount = @($FrontendBundleDocument.files).Count
    $FrontendBundleEvidence.totals = $FrontendBundleDocument.totals
    $FrontendBundleEvidence.limits = $FrontendBundleDocument.limits

    if ($Profile -in @("Full", "Release", "Package")) {
        Invoke-Checked "Rust clippy" $TauriRoot { cargo clippy --all-targets -- -D warnings }
        Invoke-Checked "Rust tests" $TauriRoot { cargo test --lib }
    }

    if ($ReleaseLike) {
        Invoke-Checked "CycloneDX SBOM" $ProjectRoot {
            & (Join-Path $ProjectRoot "tools\generate-sbom.ps1") -OutputPath $SbomPath
        }
        $SbomDocument = Get-Content -LiteralPath $SbomPath -Raw | ConvertFrom-Json
        $SbomEvidence.status = "available"
        $SbomEvidence.sha256 = Get-Sha256 $SbomPath
        $SbomEvidence.componentCount = @($SbomDocument.components).Count
        Invoke-Checked "Supply-chain audit" $ProjectRoot {
            & (Join-Path $ProjectRoot "tools\check-supply-chain.ps1") -RequireAuditTools
        }
        Invoke-Checked "Installer contract" $ProjectRoot {
            & (Join-Path $ProjectRoot "tools\check-installer-contract.ps1")
        }
        Invoke-Checked "Updater key policy" $ProjectRoot {
            & node tools/check-updater-key-policy.mjs
        }
        Invoke-Checked "Updater manifest contract" $ProjectRoot {
            npm run updater:contract:test
        }
        $TauriReleaseArguments = @("run", "tauri", "--", "build", "--no-bundle") + $TauriConfigArguments
        Invoke-Checked "Tauri release build" $ProjectRoot { & npm.cmd @TauriReleaseArguments }
    }

    if ($Profile -eq "Package") {
        Invoke-Checked "Bundle artifact snapshot" $ProjectRoot {
            node tools/check-bundle.mjs snapshot --project-root $ProjectRoot --bundle-root (Join-Path $TauriRoot "target\release\bundle") --output $PackageSnapshotPath
        }
        $TauriPackageArguments = @("run", "tauri", "--", "build") + $TauriConfigArguments
        Invoke-Checked "Tauri package build" $ProjectRoot { & npm.cmd @TauriPackageArguments }
        Invoke-Checked "Bundle artifact inventory" $ProjectRoot {
            node tools/check-bundle.mjs artifacts --project-root $ProjectRoot --bundle-root (Join-Path $TauriRoot "target\release\bundle") --snapshot $PackageSnapshotPath --output $PackageArtifactsPath
        }
        $PackageArtifactsDocument = Get-Content -LiteralPath $PackageArtifactsPath -Raw | ConvertFrom-Json
        $PackageArtifactsEvidence.status = $PackageArtifactsDocument.status
        $PackageArtifactsEvidence.sha256 = Get-Sha256 $PackageArtifactsPath
        $PackageArtifactsEvidence.artifactCount = @($PackageArtifactsDocument.artifacts).Count
        $PackageArtifactsEvidence.artifacts = @($PackageArtifactsDocument.artifacts)
        Invoke-Checked "Installed artifact smoke" $ProjectRoot {
            $NsisInstallerPath = Join-Path $TauriRoot "target\release\bundle\nsis\Columnia_$($ProjectVersion)_x64-setup.exe"
            if (-not (Test-Path -LiteralPath $NsisInstallerPath -PathType Leaf)) {
                throw "No se encontró el instalador NSIS empaquetado: $NsisInstallerPath"
            }
            & (Join-Path $ProjectRoot "tools\smoke-installed-artifact.ps1") -InstallerPath $NsisInstallerPath
        }
    }

    $ValidationStatus = "passed"
    Write-Host "Validación local $Profile completada."
}
catch {
    $FailureMessage = $_.Exception.Message
    if ($ReleaseLike -and $SbomEvidence.status -eq "pending") {
        $SbomEvidence.status = "failed"
    }
    if ($FrontendBundleEvidence.status -eq "pending" -and (Test-Path -LiteralPath $FrontendBundlePath -PathType Leaf)) {
        try {
            $FailedBundleDocument = Get-Content -LiteralPath $FrontendBundlePath -Raw | ConvertFrom-Json
            $FrontendBundleEvidence.status = $FailedBundleDocument.status
            $FrontendBundleEvidence.sha256 = Get-Sha256 $FrontendBundlePath
            $FrontendBundleEvidence.fileCount = @($FailedBundleDocument.files).Count
            $FrontendBundleEvidence.totals = $FailedBundleDocument.totals
            $FrontendBundleEvidence.limits = $FailedBundleDocument.limits
        }
        catch {
            $FrontendBundleEvidence.status = "failed"
        }
    }
    elseif ($FrontendBundleEvidence.status -eq "pending") {
        $FrontendBundleEvidence.status = "failed"
    }
    if ($Profile -eq "Package" -and $PackageArtifactsEvidence.status -eq "pending" -and (Test-Path -LiteralPath $PackageArtifactsPath -PathType Leaf)) {
        try {
            $FailedArtifactsDocument = Get-Content -LiteralPath $PackageArtifactsPath -Raw | ConvertFrom-Json
            $PackageArtifactsEvidence.status = $FailedArtifactsDocument.status
            $PackageArtifactsEvidence.sha256 = Get-Sha256 $PackageArtifactsPath
            $PackageArtifactsEvidence.artifactCount = @($FailedArtifactsDocument.artifacts).Count
            $PackageArtifactsEvidence.artifacts = @($FailedArtifactsDocument.artifacts)
        }
        catch {
            $PackageArtifactsEvidence.status = "failed"
        }
    }
    elseif ($Profile -eq "Package" -and $PackageArtifactsEvidence.status -eq "pending") {
        $PackageArtifactsEvidence.status = "failed"
    }
    throw
}
finally {
    $RunTimer.Stop()
    $ReportDirectory = Split-Path -Parent $ReportPath
    New-Item -ItemType Directory -Path $ReportDirectory -Force | Out-Null
    [ordered]@{
        schemaVersion = 1
        project = "Columnia"
        projectVersion = $ProjectVersion
        profile = $Profile
        status = $ValidationStatus
        startedAt = $StartedAt.ToString("o")
        finishedAt = [DateTimeOffset]::UtcNow.ToString("o")
        durationMs = $RunTimer.ElapsedMilliseconds
        git = [ordered]@{
            commit = $Commit
            branch = $Branch
            dirty = $TreeDirty
        }
        tools = $ToolVersions
        environment = $RuntimeEnvironment
        lockfiles = $LockfileFingerprints
        sbom = $SbomEvidence
        frontendBundle = $FrontendBundleEvidence
        packageArtifacts = $PackageArtifactsEvidence
        steps = $StepResults
        error = $FailureMessage
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReportPath -Encoding utf8
    Write-Host "Reporte: $ReportPath"
}
