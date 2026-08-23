[CmdletBinding()]
param(
    [string]$OutputPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ProjectRootUri = [Uri]::new("$ProjectRoot\")
$Stamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot ".local\validation\$Stamp-secrets.json"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot $OutputPath
}

$TextExtensions = @(
    ".c", ".cc", ".cpp", ".css", ".html", ".h", ".hpp", ".js", ".json",
    ".md", ".mjs", ".ps1", ".rs", ".toml", ".ts", ".tsx", ".txt", ".xml",
    ".yaml", ".yml"
)
$Patterns = @(
    @{ name = "private_key"; regex = "-----BEGIN (?:RSA|OPENSSH|EC|DSA|PGP|PRIVATE) KEY-----" },
    @{ name = "provider_token"; regex = "\b(?:ghp|github_pat|sk|xox[baprs])_[A-Za-z0-9_-]{16,}\b" },
    @{ name = "cloud_access_key"; regex = "\bAKIA[0-9A-Z]{16}\b" },
    @{ name = "assigned_secret"; regex = '(?i)\b(?:api[_-]?key|access[_-]?token|client[_-]?secret|secret[_-]?key|password)\b\s*[:=]\s*["''][A-Za-z0-9_./+=-]{16,}["'']' }
)

function Relative-Path {
    param([string]$Path)
    $ProjectRootUri.MakeRelativeUri([Uri]$Path).ToString().Replace("%20", " ").Replace("/", "/")
}

$Hits = [System.Collections.Generic.List[object]]::new()
$TrackedFiles = @(git -C $ProjectRoot ls-files --cached --others --exclude-standard)
foreach ($RelativePath in $TrackedFiles) {
    $FullPath = Join-Path $ProjectRoot $RelativePath
    if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) { continue }
    if ($TextExtensions -notcontains ([System.IO.Path]::GetExtension($FullPath).ToLowerInvariant())) { continue }

    $Bytes = [System.IO.File]::ReadAllBytes($FullPath)
    if ($Bytes -contains 0) { continue }
    $Contents = [System.Text.Encoding]::UTF8.GetString($Bytes)
    $LineNumber = 0
    foreach ($Line in ($Contents -split "`r?`n")) {
        $LineNumber++
        foreach ($Pattern in $Patterns) {
            if ($Line -match $Pattern.regex) {
                $Hits.Add([ordered]@{
                        file = $RelativePath.Replace("\", "/")
                        line = $LineNumber
                        rule = $Pattern.name
                    })
            }
        }
    }
}

$Document = [ordered]@{
    schemaVersion = 1
    status = if ($Hits.Count -eq 0) { "passed" } else { "failed" }
    scannedFileCount = $TrackedFiles.Count
    hits = @($Hits)
    generatedAt = [DateTimeOffset]::UtcNow.ToString("o")
}
New-Item -ItemType Directory -Path (Split-Path -Parent $OutputPath) -Force | Out-Null
$Document | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutputPath -Encoding utf8

if ($Hits.Count -gt 0) {
    throw "El escaneo de secretos encontró $($Hits.Count) coincidencia(s). Evidencia: $($ProjectRootUri.MakeRelativeUri([Uri]$OutputPath).ToString())"
}
Write-Host "Escaneo de secretos aprobado: $($TrackedFiles.Count) archivos inspeccionados."
