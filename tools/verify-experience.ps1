$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

function Invoke-ExperienceCheck {
    param(
        [string]$Label,
        [string[]]$Arguments
    )

    Write-Host "== $Label =="
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

Invoke-ExperienceCheck -Label "Accessibility baseline" -Arguments @("run", "accessibility:check")
Invoke-ExperienceCheck -Label "Performance baseline" -Arguments @("run", "perf:check")
Write-Host "Verificación de experiencia aprobada: accesibilidad visual y rendimiento dentro de contrato."
