# RV21 / QA-02: drives each synthetic file in fixtures/roundtrip through the
# real app (load, Preparar proposal, apply) and fails if what the proposal
# announced differs from what the resulting data shows. Columnia must be closed.
param(
    [ValidateRange(60, 900)]
    [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Probe = Join-Path $PSScriptRoot "probe-webview2-cdp.ps1"
$Fixtures = Get-ChildItem -LiteralPath (Join-Path $ProjectRoot "fixtures\roundtrip") -Filter "*.csv" | Sort-Object Name
$Failed = @()

foreach ($Fixture in $Fixtures) {
    Write-Host "Ida y vuelta: $($Fixture.Name)"
    & $Probe -RunNativeSelectors -RunPrepareFlow -NativeDatasetPath $Fixture.FullName -TimeoutSeconds $TimeoutSeconds
    if (-not $?) {
        $Failed += $Fixture.Name
    }
}

if ($Failed.Count -gt 0) {
    throw "La batería de ida y vuelta falló en: $($Failed -join ', '). Revisa announcedChecks en .local/validation/webview2-cdp/."
}
Write-Host "Batería de ida y vuelta aprobada: $($Fixtures.Count) fixtures; lo anunciado coincide con lo aplicado."
