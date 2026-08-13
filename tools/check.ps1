param(
    [ValidateSet("Fast", "Full", "Release")]
    [string]$Profile = "Fast"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot "src-tauri"

function Invoke-Checked {
    param(
        [string]$Label,
        [string]$WorkingDirectory,
        [scriptblock]$Command
    )

    Write-Host "[$Label]"
    Push-Location $WorkingDirectory
    try {
        & $Command
        if ($LASTEXITCODE -ne 0) {
            throw "$Label falló con código $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

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

Write-Host "Validación local $Profile completada."
