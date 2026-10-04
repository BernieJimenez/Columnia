[CmdletBinding()]
param(
    [string]$OutputPath,
    # Scans every file under this folder instead of the repository (tests).
    [string]$ScanRoot,
    # Git repository to list (tests); the project by default.
    [string]$RepositoryRoot
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

# SEG-03: every file is scanned unless it is binary or of a binary kind; the
# Git history is not scanned (use `git log -p` with the same patterns).
$BinaryExtensions = @(
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".icns", ".bmp", ".webp", ".woff", ".woff2",
    ".ttf", ".otf", ".parquet", ".xlsx", ".xls", ".xlsb", ".ods", ".zip", ".gz", ".7z",
    ".exe", ".dll", ".pdb", ".msi", ".pdf", ".sqlite", ".sqlite3", ".duckdb", ".wasm"
)
$Patterns = @(
    @{ name = "private_key"; regex = "-----BEGIN (?:(?:RSA|OPENSSH|EC|DSA|PGP|ENCRYPTED) )?PRIVATE KEY(?: BLOCK)?-----" },
    @{ name = "provider_token"; regex = "\b(?:gh[pousr]_|github_pat_|xox[baprs]-|sk-(?:ant-|proj-)?|sk_(?:live|test)_)[A-Za-z0-9_-]{16,}" },
    @{ name = "cloud_access_key"; regex = "\bAKIA[0-9A-Z]{16}\b" },
    # Tauri updater signing key (rsign/minisign secret key), as text or base64.
    @{ name = "signing_secret_key"; regex = "untrusted comment: (?:rsign|minisign) encrypted secret key|IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5|IG1pbmlzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5" },
    @{ name = "assigned_secret"; regex = '(?i)\b(?:api[_-]?key|access[_-]?token|client[_-]?secret|secret[_-]?key|password)\b\s*[:=]\s*["''][A-Za-z0-9_./+=-]{16,}["'']' },
    # .env style: NAME_PASSWORD=value without quotes.
    @{ name = "env_secret"; configOnly = $true; regex = '(?m)^\s*[A-Z0-9_]*(?:PASSWORD|SECRET|TOKEN|API_KEY|PRIVATE_KEY)[A-Z0-9_]*\s*=\s*[^\s#"''$][^\s#]{7,}\s*$' }
)

function Relative-Path {
    param([string]$Path)
    $ProjectRootUri.MakeRelativeUri([Uri]$Path).ToString().Replace("%20", " ").Replace("/", "/")
}

$Hits = [System.Collections.Generic.List[object]]::new()
if ([string]::IsNullOrWhiteSpace($ScanRoot)) {
    $ScanBase = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) { $ProjectRoot } else { (Resolve-Path $RepositoryRoot).Path }
    # OPS-05: NUL-separated, unquoted names (ñ.ts stays ñ.ts), and a failing
    # or empty listing is an error, never «0 files, no secrets».
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $Listing = git -C $ScanBase -c core.quotepath=off ls-files --cached --others --exclude-standard -z 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "No se pudo listar los archivos del repositorio con git (código $LASTEXITCODE)."
    }
    $TrackedFiles = @(([string]($Listing -join "")) -split "`0" | Where-Object { $_ -ne "" })
    if ($TrackedFiles.Count -eq 0) {
        throw "git no devolvió ningún archivo que escanear."
    }
}
else {
    $ScanBase = (Resolve-Path $ScanRoot).Path
    $TrackedFiles = @(Get-ChildItem -LiteralPath $ScanBase -Recurse -File | ForEach-Object {
            $_.FullName.Substring($ScanBase.Length).TrimStart("\", "/")
        })
}
foreach ($RelativePath in $TrackedFiles) {
    $FullPath = Join-Path $ScanBase $RelativePath
    if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) { continue }
    if ($BinaryExtensions -contains ([System.IO.Path]::GetExtension($FullPath).ToLowerInvariant())) { continue }
    # The patterns of this script would match themselves.
    if ($RelativePath.Replace("\", "/") -eq "tools/check-secrets.ps1") { continue }
    $FileName = [System.IO.Path]::GetFileName($FullPath).ToLowerInvariant()
    $IsConfigFile = $FileName.StartsWith(".env") -or ($FileName -match "\.(env|ini|cfg|conf|properties)$")
    if ((Get-Item -LiteralPath $FullPath).Length -gt 20MB) { continue }

    $Bytes = [System.IO.File]::ReadAllBytes($FullPath)
    if ($Bytes -contains 0) { continue }
    $Contents = [System.Text.Encoding]::UTF8.GetString($Bytes)
    $LineNumber = 0
    foreach ($Line in ($Contents -split "`r?`n")) {
        $LineNumber++
        foreach ($Pattern in $Patterns) {
            if ($Pattern.configOnly -and -not $IsConfigFile) { continue }
            if ($Line -cmatch $Pattern.regex) {
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
