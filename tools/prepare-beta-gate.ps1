[CmdletBinding()]
param(
    [ValidateSet("Gate1", "Gate2")]
    [string]$Gate = "Gate1",

    [ValidatePattern("^[a-z0-9][a-z0-9.-]{2,79}$")]
    [string]$CandidateId
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ValidationRoot = Join-Path $ProjectRoot ".local\validation"
$BetaRoot = Join-Path $ProjectRoot ".local\beta"

function Get-GitOutput {
    param([string[]]$Arguments)

    $output = & git -C $ProjectRoot @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Git termino con codigo $LASTEXITCODE."
    }
    return ([string]::Join([Environment]::NewLine, @($output))).Trim()
}

$DirtyFiles = Get-GitOutput -Arguments @("status", "--porcelain")
if (-not [string]::IsNullOrWhiteSpace($DirtyFiles)) {
    throw "La beta requiere un arbol Git limpio. Registra o descarta los cambios antes de crear la RC."
}

$Commit = Get-GitOutput -Arguments @("rev-parse", "HEAD")
$ShortCommit = Get-GitOutput -Arguments @("rev-parse", "--short", "HEAD")
$Branch = Get-GitOutput -Arguments @("branch", "--show-current")
if ([string]::IsNullOrWhiteSpace($Branch)) {
    throw "La beta requiere una rama Git explicita; no se acepta HEAD separado."
}

$PackageManifest = Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json
$Version = [string]$PackageManifest.version
$GateLabel = if ($Gate -eq "Gate1") { "Gate 1 - baseline V1" } else { "Gate 2 - shell de espacios" }
$GateSlug = $Gate.ToLowerInvariant()
if ([string]::IsNullOrWhiteSpace($CandidateId)) {
    $CandidateId = "rc-$Version-$ShortCommit-$GateSlug"
}

$FullReports = @(
    Get-ChildItem -LiteralPath $ValidationRoot -File -Filter "*-$ShortCommit-full.json" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending
)
$AcceptedReport = $null
foreach ($ReportFile in $FullReports) {
    try {
        $Report = Get-Content -LiteralPath $ReportFile.FullName -Raw | ConvertFrom-Json
        if ($Report.schemaVersion -eq 1 -and
            $Report.profile -eq "Full" -and
            $Report.status -eq "passed" -and
            $Report.git.commit -eq $Commit -and
            $Report.git.dirty -eq $false) {
            $AcceptedReport = $ReportFile
            break
        }
    }
    catch {
        # Ignora evidencia incompleta o corrupta y continua buscando una valida.
    }
}
if ($null -eq $AcceptedReport) {
    throw "No existe un gate Full aprobado y limpio para $ShortCommit. Ejecuta .\tools\check.ps1 -Profile Full."
}

$CandidateRoot = Join-Path $BetaRoot $CandidateId
if (Test-Path -LiteralPath $CandidateRoot) {
    throw "La RC ya existe: .local/beta/$CandidateId. No se sobrescribe evidencia de sesiones."
}

New-Item -ItemType Directory -Path $CandidateRoot -Force | Out-Null
$SessionTemplate = Get-Content -LiteralPath (Join-Path $ProjectRoot "docs\templates\beta-session.md") -Raw -Encoding UTF8
for ($Index = 1; $Index -le 3; $Index++) {
    $Alias = "beta-{0:d2}" -f $Index
    $SessionDirectory = Join-Path $CandidateRoot $Alias
    New-Item -ItemType Directory -Path $SessionDirectory -Force | Out-Null
    $Session = $SessionTemplate
    $Session = $Session -replace '(?m)^(\| Alias de sesi.n \|) beta-___ (\|)$', "`$1 $Alias `$2"
    $Session = $Session.Replace("| Commit probado | ___ |", "| Commit probado | $Commit |")
    $Session = $Session -replace '(?m)^(\| Versi.n de Columnia \|) ___ (\|)$', "`$1 $Version `$2"
    $Session = $Session -replace '(?m)^(\| Ronda de medici.n \|) .+ (\|)$', "`$1 $GateLabel `$2"
    $Session = $Session.Replace("| Release candidate | rc-___ |", "| Release candidate | $CandidateId |")
    Set-Content -LiteralPath (Join-Path $SessionDirectory "session.md") -Value $Session -Encoding utf8
}

Copy-Item -LiteralPath (Join-Path $ProjectRoot "docs\templates\beta-summary.md") -Destination (Join-Path $CandidateRoot "summary-draft.md")
$ReportRelativePath = $AcceptedReport.FullName.Substring($ProjectRoot.Length + 1).Replace("\", "/")
[ordered]@{
    schemaVersion = 1
    candidateId = $CandidateId
    gate = $Gate
    gateLabel = $GateLabel
    version = $Version
    commit = $Commit
    branch = $Branch
    preparedAt = [DateTimeOffset]::UtcNow.ToString("o")
    technicalGate = [ordered]@{
        profile = "Full"
        status = "passed"
        report = $ReportRelativePath
    }
    sessions = @("beta-01/session.md", "beta-02/session.md", "beta-03/session.md")
    status = "awaiting-human-sessions"
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $CandidateRoot "manifest.json") -Encoding utf8

Write-Host "RC creada: $CandidateId"
Write-Host "Commit: $Commit"
Write-Host "Sesiones: .local/beta/$CandidateId/beta-01..03/session.md"
Write-Host "Estado: esperando tres sesiones humanas; no se registraron resultados automaticamente."
