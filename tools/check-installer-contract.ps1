[CmdletBinding()]
param(
    # OPS-14: the contract test checks altered copies of the configuration.
    [string]$ConfigPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($ConfigPath)) {
    $ConfigPath = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"
}
$Config = Get-Content -Encoding UTF8 -LiteralPath $ConfigPath -Raw | ConvertFrom-Json

# OPS-14: only the per-user NSIS installer is built; the MSI that Tauri
# generates installs per machine (Program Files, administrator) in English.
$Targets = @($Config.bundle.targets)
if ($Targets.Count -ne 1 -or $Targets[0] -ne "nsis") {
    throw "bundle.targets debe ser [""nsis""]: cualquier otro instalador sería por máquina o no estaría en español."
}
if ($Config.bundle.windows.nsis.installMode -ne "currentUser") {
    throw "El instalador NSIS debe usar installMode=currentUser."
}
$Languages = @($Config.bundle.windows.nsis.languages)
if ($Languages.Count -ne 1 -or $Languages[0] -ne "Spanish") {
    throw "El instalador NSIS debe estar en español: bundle.windows.nsis.languages = [""Spanish""]."
}
if ($Config.bundle.windows.webviewInstallMode.type -ne "downloadBootstrapper") {
    throw "La política WebView2 debe declarar downloadBootstrapper explícitamente."
}
if ([string]::IsNullOrWhiteSpace($Config.plugins.updater.pubkey) -or $Config.plugins.updater.pubkey -match "[\\/]") {
    throw "El updater debe declarar una clave pública embebida; nunca una ruta local."
}
if ($Config.plugins.updater.windows.installMode -ne "passive") {
    throw "El updater de Windows debe usar installMode=passive para mostrar progreso durante la instalación."
}
foreach ($Resource in @($Config.bundle.resources)) {
    $Path = Join-Path (Join-Path $ProjectRoot "src-tauri") $Resource
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "El recurso configurado no existe: $Resource"
    }
}

Write-Host "Contrato de instalador aprobado: solo NSIS currentUser en español, WebView2 downloadBootstrapper y recursos presentes."
