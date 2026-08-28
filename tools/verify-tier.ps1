param(
    [switch]$SkipPackage,
    [switch]$SkipNative
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$StartedAt = [DateTimeOffset]::UtcNow
$PowerShellExecutable = (Get-Command pwsh.exe -ErrorAction SilentlyContinue).Source
if ([string]::IsNullOrWhiteSpace($PowerShellExecutable)) {
    $PowerShellExecutable = (Get-Command powershell.exe -ErrorAction Stop).Source
}

function Invoke-NpmStage {
    param([string]$Label, [string[]]$Arguments)

    Write-Host "`n== $Label =="
    Push-Location $ProjectRoot
    try {
        & npm.cmd @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Label terminó con código $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-PowerShellStage {
    param([string]$Label, [string]$Script, [string[]]$Arguments = @())

    Write-Host "`n== $Label =="
    Push-Location $ProjectRoot
    try {
        & $PowerShellExecutable -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot $Script) @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Label terminó con código $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

try {
    Invoke-NpmStage "Vitest" @("run", "test")
    Invoke-NpmStage "Playwright" @("run", "test:e2e")
    Invoke-NpmStage "Build" @("run", "build")
    Invoke-NpmStage "Evidencia visual" @("run", "accessibility:visual")
    Invoke-NpmStage "Baseline visual" @("run", "accessibility:check")
    Invoke-NpmStage "Benchmark sostenido y proyecto durable" @("run", "perf:benchmark")
    Invoke-NpmStage "Resumen de rendimiento" @("run", "perf:summary")

    # SkipPackage only skips the expensive MSI/NSIS bundling. Release still
    # executes Rust, coverage, Clippy, supply-chain, installer-contract and
    # the unbundled Tauri build.
    $CheckProfile = if ($SkipPackage) { "Release" } else { "Package" }
    Invoke-PowerShellStage "Release gates" "tools/check.ps1" @("-Profile", $CheckProfile)

    Invoke-PowerShellStage "Smoke CLI" "tools/smoke-cli.ps1"

    if (-not $SkipNative) {
        Invoke-PowerShellStage "Smoke desktop (npm run tauri dev)" "tools/smoke-tauri.ps1" @("-TimeoutSeconds", "120")
        Invoke-NpmStage "Smoke WebView2/reinicio" @("run", "smoke:restart")
        Invoke-NpmStage "Smoke WebView2/CDP sostenido" @("run", "smoke:cdp")
        Invoke-NpmStage "Smoke selectores nativos Win32" @("run", "smoke:native-selectors")
        Invoke-NpmStage "Resumen de rendimiento final" @("run", "perf:summary")
    }

    Invoke-PowerShellStage "Gate de rendimiento" "tools/check-performance-baseline.ps1"
    Invoke-PowerShellStage "Verificación compuesta" "tools/verify-experience.ps1"
    $Duration = [math]::Round(([DateTimeOffset]::UtcNow - $StartedAt).TotalMinutes, 2)
    Write-Host "`nTier verificado correctamente en $Duration minutos."
}
catch {
    Write-Error "Verificación del tier falló: $($_.Exception.Message)"
    exit 1
}
