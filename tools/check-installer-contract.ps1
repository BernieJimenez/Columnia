[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Config = Get-Content -LiteralPath (Join-Path $ProjectRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json

if ($Config.bundle.windows.nsis.installMode -ne "currentUser") {
    throw "El instalador NSIS debe usar installMode=currentUser."
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

Write-Host "Contrato de instalador aprobado: NSIS currentUser, WebView2 downloadBootstrapper y recursos presentes."
