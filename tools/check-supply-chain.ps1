[CmdletBinding()]
param(
    [switch]$RequireAuditTools
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$ProjectRootUri = [Uri]::new("$ProjectRoot\")
$Stamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$EvidenceDirectory = Join-Path $ProjectRoot ".local\validation\supply-chain\$Stamp"
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null

function Relative-Path {
    param([string]$Path)
    $ProjectRootUri.MakeRelativeUri([Uri]$Path).ToString().Replace("%20", " ")
}

function Run-JsonCommand {
    param(
        [string]$Label,
        [string]$Executable,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$OutputPath
    )
    $SafeLabel = $Label -replace "[^a-zA-Z0-9.-]", "-"
    $StdoutPath = Join-Path $EvidenceDirectory "$SafeLabel.stdout.txt"
    $StderrPath = Join-Path $EvidenceDirectory "$SafeLabel.stderr.txt"
    Push-Location $WorkingDirectory
    try {
        $Process = Start-Process -FilePath $Executable -ArgumentList $Arguments -WorkingDirectory $WorkingDirectory `
            -RedirectStandardOutput $StdoutPath -RedirectStandardError $StderrPath -Wait -PassThru -NoNewWindow
        $ExitCode = $Process.ExitCode
    }
    finally {
        Pop-Location
    }
    $Stdout = if (Test-Path -LiteralPath $StdoutPath) { Get-Content -LiteralPath $StdoutPath -Raw } else { "" }
    $Stderr = if (Test-Path -LiteralPath $StderrPath) { Get-Content -LiteralPath $StderrPath -Raw } else { "" }
    [System.IO.File]::WriteAllText($OutputPath, ($Stdout + $Stderr), [System.Text.UTF8Encoding]::new($false))
    [ordered]@{
        status = if ($ExitCode -eq 0) { "passed" } else { "failed" }
        exitCode = $ExitCode
        evidence = Relative-Path $OutputPath
    }
}

$Results = [ordered]@{}
$NpmAuditPath = Join-Path $EvidenceDirectory "npm-audit.json"
$Results.npmAudit = Run-JsonCommand "npm audit" "npm.cmd" @("audit", "--json", "--omit=optional") $ProjectRoot $NpmAuditPath
try {
    $NpmAudit = Get-Content -LiteralPath $NpmAuditPath -Raw | ConvertFrom-Json
    $Vulnerabilities = $NpmAudit.metadata.vulnerabilities
    $TotalVulnerabilities = @($Vulnerabilities.PSObject.Properties | ForEach-Object { [int]$_.Value } | Measure-Object -Sum).Sum
    $Results.npmAudit.vulnerabilityCount = [int]$TotalVulnerabilities
    if ($TotalVulnerabilities -gt 0) { throw "npm audit reporta $TotalVulnerabilities vulnerabilidad(es)." }
}
catch {
    if ($Results.npmAudit.status -eq "passed") { throw }
    $Results.npmAudit.parseError = $_.Exception.Message
}

$CargoAuditCommand = Get-Command cargo-audit -ErrorAction SilentlyContinue
if ($null -eq $CargoAuditCommand) {
    $Results.cargoAudit = [ordered]@{ status = "unavailable"; reason = "cargo-audit no está instalado" }
    if ($RequireAuditTools) { throw "Falta cargo-audit. Instala cargo-audit o ejecuta sin -RequireAuditTools para conservar un estado no bloqueante." }
}
else {
    $CargoAuditPath = Join-Path $EvidenceDirectory "cargo-audit.json"
    $Results.cargoAudit = Run-JsonCommand "cargo audit" "cargo.exe" @(
        "audit",
        "--json",
        "--ignore", "RUSTSEC-2026-0194",
        "--ignore", "RUSTSEC-2026-0195"
    ) $TauriRoot $CargoAuditPath
    if ($Results.cargoAudit.status -ne "passed") { throw "cargo audit falló. Evidencia: $(Relative-Path $CargoAuditPath)" }
    $CargoAuditStdoutPath = Join-Path $EvidenceDirectory "cargo-audit.stdout.txt"
    try {
        $CargoAuditDocument = Get-Content -LiteralPath $CargoAuditStdoutPath -Raw | ConvertFrom-Json
        $Results.cargoAudit.vulnerabilityCount = @($CargoAuditDocument.vulnerabilities.list).Count
        $Results.cargoAudit.unmaintainedCount = @($CargoAuditDocument.warnings.unmaintained).Count
        $Results.cargoAudit.unsoundCount = @($CargoAuditDocument.warnings.unsound).Count
        $Results.cargoAudit.ignoredAdvisories = @($CargoAuditDocument.settings.ignore)
    }
    catch {
        throw "cargo audit no produjo JSON parseable. Evidencia: $(Relative-Path $CargoAuditPath)"
    }
}

$CargoDenyCommand = Get-Command cargo-deny -ErrorAction SilentlyContinue
if ($null -eq $CargoDenyCommand) {
    $Results.cargoDeny = [ordered]@{ status = "unavailable"; reason = "cargo-deny no está instalado" }
    if ($RequireAuditTools) { throw "Falta cargo-deny. Instala cargo-deny o ejecuta sin -RequireAuditTools para conservar un estado no bloqueante." }
}
else {
    $CargoDenyPath = Join-Path $EvidenceDirectory "cargo-deny.log"
    $Results.cargoDeny = Run-JsonCommand "cargo deny" $CargoDenyCommand.Source @("--format", "json", "check") $TauriRoot $CargoDenyPath
    if ($Results.cargoDeny.status -ne "passed") { throw "cargo deny falló. Evidencia: $(Relative-Path $CargoDenyPath)" }
}

$SecretsPath = Join-Path $EvidenceDirectory "secrets.json"
$SecretsScript = Join-Path $ProjectRoot "tools\check-secrets.ps1"
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $SecretsScript -OutputPath $SecretsPath
if ($LASTEXITCODE -ne 0) { throw "El escaneo de secretos falló." }
$Results.secrets = [ordered]@{ status = "passed"; evidence = Relative-Path $SecretsPath }

$NoticesScript = Join-Path $ProjectRoot "tools\generate-third-party-notices.ps1"
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $NoticesScript -Check
if ($LASTEXITCODE -ne 0) { throw "El inventario de third-party notices está desactualizado." }
$Results.thirdPartyNotices = [ordered]@{ status = "passed"; path = "THIRD_PARTY_NOTICES.md" }

$NetworkCheck = Join-Path $ProjectRoot "tools\check-network-policy.mjs"
& node.exe $NetworkCheck
if ($LASTEXITCODE -ne 0) { throw "La política de red/telemetría falló." }
$Results.networkPolicy = [ordered]@{ status = "passed"; path = "tools/check-network-policy.mjs" }

$Document = [ordered]@{
    schemaVersion = 1
    status = "passed"
    requireAuditTools = [bool]$RequireAuditTools
    generatedAt = [DateTimeOffset]::UtcNow.ToString("o")
    results = $Results
    evidenceDirectory = Relative-Path $EvidenceDirectory
}
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$Document | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
Write-Host "Supply chain aprobado. Evidencia: $(Relative-Path $SummaryPath)"
