[CmdletBinding()]
param(
    [ValidateSet("Fast", "Full")]
    [string]$Profile = "Fast"
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot ".."))

function Invoke-Checked {
    param(
        [Parameter(Mandatory)] [string]$Command,
        [Parameter(Mandatory)] [string[]]$Arguments,
        [Parameter(Mandatory)] [string]$WorkingDirectory
    )

    Push-Location $WorkingDirectory
    try {
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "Falló: $Command $($Arguments -join ' ') (código $LASTEXITCODE)"
        }
    }
    finally {
        Pop-Location
    }
}

Invoke-Checked -Command "npm" -Arguments @("run", "build") -WorkingDirectory $projectRoot
Invoke-Checked -Command "npm" -Arguments @("test") -WorkingDirectory $projectRoot

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    throw "Rust/Cargo no está instalado. Instala rustup y vuelve a ejecutar tools/check.ps1."
}

$rustRoot = Join-Path $projectRoot "src-tauri"
Invoke-Checked -Command "cargo" -Arguments @("fmt", "--check") -WorkingDirectory $rustRoot
Invoke-Checked -Command "cargo" -Arguments @("clippy", "--all-targets", "--", "-D", "warnings") -WorkingDirectory $rustRoot
Invoke-Checked -Command "cargo" -Arguments @("test") -WorkingDirectory $rustRoot

if ($Profile -eq "Full") {
    Invoke-Checked -Command "npm" -Arguments @("run", "tauri", "build", "--", "--no-bundle") -WorkingDirectory $projectRoot
}

Write-Host "Validación local $Profile completada." -ForegroundColor Green

