# RV21 / QA-02: drives each synthetic file in fixtures/roundtrip through the
# real app (load, Preparar proposal, apply) and fails if what the proposal
# announced differs from what the resulting data shows. Columnia must be closed.
param(
    [ValidateRange(60, 900)]
    [int]$TimeoutSeconds = 600,
    # QA-47: the contract test swaps in a stand-in probe and its own fixtures.
    [string]$ProbePath = (Join-Path $PSScriptRoot "probe-webview2-cdp.ps1"),
    [string]$FixtureDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) "fixtures\roundtrip")
)

$ErrorActionPreference = "Stop"
$Fixtures = @(Get-ChildItem -LiteralPath $FixtureDirectory -Filter "*.csv" | Sort-Object Name)
$Failed = @()

foreach ($Fixture in $Fixtures) {
    Write-Host "Ida y vuelta: $($Fixture.Name)"
    # One failing fixture must not hide the result of the others.
    try {
        # QA-47: a probe that only sets its exit code also counts as failed.
        $global:LASTEXITCODE = 0
        & $ProbePath -RunNativeSelectors -RunPrepareFlow -NativeDatasetPath $Fixture.FullName -TimeoutSeconds $TimeoutSeconds
        if (-not $? -or $LASTEXITCODE -ne 0) {
            $Failed += $Fixture.Name
        }
    }
    catch {
        $Failed += $Fixture.Name
    }
}

if ($Failed.Count -gt 0) {
    throw "La batería de ida y vuelta falló en: $($Failed -join ', '). Revisa announcedChecks en .local/validation/webview2-cdp/."
}
Write-Host "Batería de ida y vuelta aprobada: $($Fixtures.Count) fixtures; lo anunciado coincide con lo aplicado."
