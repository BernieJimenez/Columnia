param(
    [ValidateSet("Fast", "Full", "Release")]
    [string]$Profile = "Fast",

    [string]$ReportPath
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

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
    $ReportPath = Join-Path $ProjectRoot ".local\validation\$Timestamp-$ShortCommit-$($Profile.ToLowerInvariant()).json"
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
            sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
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
    Invoke-Checked "Rust format" $TauriRoot { cargo fmt -- --check }
    Invoke-Checked "Rust check" $TauriRoot { cargo check }
    Invoke-Checked "Frontend tests" $ProjectRoot { npm test -- --run }
    Invoke-Checked "Frontend build" $ProjectRoot { npm run build }

    if ($Profile -in @("Full", "Release")) {
        Invoke-Checked "Rust clippy" $TauriRoot { cargo clippy --all-targets -- -D warnings }
        Invoke-Checked "Rust tests" $TauriRoot { cargo test --lib }
    }

    if ($Profile -eq "Release") {
        Invoke-Checked "Tauri release build" $ProjectRoot { npm run tauri build -- --no-bundle }
    }

    $ValidationStatus = "passed"
    Write-Host "Validación local $Profile completada."
}
catch {
    $FailureMessage = $_.Exception.Message
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
        steps = $StepResults
        error = $FailureMessage
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReportPath -Encoding utf8
    Write-Host "Reporte: $ReportPath"
}
