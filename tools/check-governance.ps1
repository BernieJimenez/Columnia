[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Require-File {
    param([string]$RelativePath)

    $Path = Join-Path $ProjectRoot $RelativePath
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Falta el contrato de gobierno: $RelativePath"
    }
}

function Require-Text {
    param(
        [string]$RelativePath,
        [string]$Pattern,
        [string]$Message
    )

    Require-File $RelativePath
    $Contents = Get-Content -LiteralPath (Join-Path $ProjectRoot $RelativePath) -Raw
    if ($Contents -notmatch $Pattern) {
        throw $Message
    }
}

foreach ($RelativePath in @(
        "LICENSE",
        "CONTRIBUTING.md",
        "docs\README.md",
        "docs\adr\0001-contratos-del-repositorio.md",
        "docs\reference\repository-governance.md",
        "docs\reference\dependency-audit.md",
        "THIRD_PARTY_NOTICES.md",
        "docs\reference\fixtures-policy.md",
        "fixtures\README.md",
        "fixtures\manifest.json"
    )) {
    Require-File $RelativePath
}

$PackageManifest = Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json
$CargoManifest = Get-Content -LiteralPath (Join-Path $ProjectRoot "src-tauri\Cargo.toml") -Raw

if ($PackageManifest.license -ne "MIT") {
    throw "package.json debe declarar license = MIT."
}
if ($CargoManifest -notmatch '(?m)^license\s*=\s*"MIT"\s*$') {
    throw "src-tauri/Cargo.toml debe declarar license = MIT."
}

Require-Text "LICENSE" "MIT License" "LICENSE debe contener el texto MIT."
Require-Text "CONTRIBUTING.md" "(?m)^## Ramas" "CONTRIBUTING.md debe definir la política de ramas."
Require-Text "CONTRIBUTING.md" "(?m)^## Commits" "CONTRIBUTING.md debe definir la política de commits."
Require-Text "CONTRIBUTING.md" "(?m)^## Revisión local" "CONTRIBUTING.md debe definir la revisión local."
Require-Text "docs\reference\dependency-audit.md" "npm outdated" "El inventario debe conservar el comando npm outdated."
Require-Text "docs\reference\dependency-audit.md" "cargo audit" "El inventario debe registrar la auditoría Cargo."
Require-Text "docs\reference\fixtures-policy.md" "sint" "La política debe declarar que las fixtures son sintéticas."

$Manifest = Get-Content -LiteralPath (Join-Path $ProjectRoot "fixtures\manifest.json") -Raw | ConvertFrom-Json
if ($Manifest.version -ne 1 -or $Manifest.policy -ne "synthetic-only-no-pii") {
    throw "fixtures/manifest.json no cumple el contrato sintético v1."
}

Write-Host "Gobernanza I0 aprobada: licencia, ADR, workflow, inventario y fixtures verificables."
