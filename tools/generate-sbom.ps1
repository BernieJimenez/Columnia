param(
    [string]$OutputPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$PackageManifestPath = Join-Path $ProjectRoot "package.json"
$PackageLockPath = Join-Path $ProjectRoot "package-lock.json"
$CargoManifestPath = Join-Path $ProjectRoot "src-tauri\Cargo.toml"
$CargoLockPath = Join-Path $ProjectRoot "src-tauri\Cargo.lock"

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot ".local\validation\columnia.cdx.json"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot $OutputPath
}

function Get-Sha256 {
    param([string]$Path)

    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $stream.Dispose()
        $algorithm.Dispose()
    }
}

function Convert-IntegrityHashes {
    param([string]$Integrity)

    if ([string]::IsNullOrWhiteSpace($Integrity)) {
        return @()
    }

    $AlgorithmNames = @{
        sha256 = "SHA-256"
        sha384 = "SHA-384"
        sha512 = "SHA-512"
    }
    $Hashes = foreach ($Token in ($Integrity -split '\s+')) {
        if ($Token -notmatch '^(sha256|sha384|sha512)-(.+)$') {
            continue
        }
        try {
            $Bytes = [Convert]::FromBase64String($Matches[2])
        }
        catch {
            throw "package-lock.json contiene un hash integrity inválido."
        }
        [ordered]@{
            alg = $AlgorithmNames[$Matches[1]]
            content = ([BitConverter]::ToString($Bytes)).Replace("-", "").ToLowerInvariant()
        }
    }
    @($Hashes | Sort-Object alg, content)
}

function Get-NpmPackageName {
    param([string]$PackagePath)

    $NormalizedPath = "/" + $PackagePath.TrimStart("/")
    $Marker = "/node_modules/"
    $MarkerIndex = $NormalizedPath.LastIndexOf($Marker, [StringComparison]::Ordinal)
    if ($MarkerIndex -ge 0) {
        return $NormalizedPath.Substring($MarkerIndex + $Marker.Length)
    }
    return $null
}

function Get-Purl {
    param(
        [ValidateSet("npm", "cargo")]
        [string]$Ecosystem,
        [string]$Name,
        [string]$Version
    )

    $EncodedVersion = [Uri]::EscapeDataString($Version)
    if ($Ecosystem -eq "npm" -and $Name.StartsWith("@") -and $Name.Contains("/")) {
        $Parts = $Name.Split("/", 2)
        return "pkg:npm/$([Uri]::EscapeDataString($Parts[0]))/$([Uri]::EscapeDataString($Parts[1]))@$EncodedVersion"
    }
    "pkg:$Ecosystem/$([Uri]::EscapeDataString($Name))@$EncodedVersion"
}

function New-Component {
    param(
        [ValidateSet("npm", "cargo")]
        [string]$Ecosystem,
        [string]$Name,
        [string]$Version,
        [object[]]$Hashes
    )

    $Purl = Get-Purl $Ecosystem $Name $Version
    $Component = [ordered]@{
        type = "library"
        "bom-ref" = $Purl
        name = $Name
        version = $Version
        purl = $Purl
        properties = @(
            [ordered]@{
                name = "columnia:ecosystem"
                value = $Ecosystem
            }
        )
    }
    if ($Hashes.Count -gt 0) {
        $Component.hashes = @($Hashes)
    }
    $Component
}

$PackageManifest = Get-Content -LiteralPath $PackageManifestPath -Raw | ConvertFrom-Json
$PackageLockEntriesJson = & node (Join-Path $PSScriptRoot "extract-package-lock-packages.mjs") $PackageLockPath
if ($LASTEXITCODE -ne 0) {
    throw "No se pudo leer package-lock.json con el extractor Node.js."
}
$PackageLockEntries = @($PackageLockEntriesJson | ConvertFrom-Json)
$CargoManifest = Get-Content -LiteralPath $CargoManifestPath -Raw
$CargoLock = Get-Content -LiteralPath $CargoLockPath -Raw

if ($CargoManifest -notmatch '(?ms)^\[package\]\s+.*?^name\s*=\s*"([^"]+)"') {
    throw "No se pudo leer el nombre del paquete Cargo."
}
$CargoProjectName = $Matches[1]
if ($CargoManifest -notmatch '(?ms)^\[package\]\s+.*?^version\s*=\s*"([^"]+)"') {
    throw "No se pudo leer la versión del paquete Cargo."
}
$CargoProjectVersion = $Matches[1]
if ($PackageManifest.name -ne $CargoProjectName -or $PackageManifest.version -ne $CargoProjectVersion) {
    throw "Los manifiestos npm y Cargo no identifican la misma versión del proyecto."
}

$ComponentsByKey = @{}
foreach ($Entry in $PackageLockEntries) {
    if ([string]::IsNullOrEmpty($Entry.path)) {
        continue
    }
    $Name = Get-NpmPackageName $Entry.path
    if ([string]::IsNullOrWhiteSpace($Name)) {
        continue
    }
    $Version = $Entry.version.ToString()
    $Key = "npm|$Name|$Version"
    if (-not $ComponentsByKey.ContainsKey($Key)) {
        $Hashes = Convert-IntegrityHashes $Entry.integrity
        $ComponentsByKey[$Key] = New-Component "npm" $Name $Version $Hashes
    }
}

foreach ($Block in ($CargoLock -split '(?m)^\[\[package\]\]\s*$')) {
    if ($Block -notmatch '(?m)^name\s*=\s*"([^"]+)"\s*$') {
        continue
    }
    $Name = $Matches[1]
    if ($Block -notmatch '(?m)^version\s*=\s*"([^"]+)"\s*$') {
        continue
    }
    $Version = $Matches[1]
    if ($Name -eq $CargoProjectName -and $Version -eq $CargoProjectVersion) {
        continue
    }
    $Key = "cargo|$Name|$Version"
    if ($ComponentsByKey.ContainsKey($Key)) {
        continue
    }
    $Hashes = @()
    if ($Block -match '(?m)^checksum\s*=\s*"([0-9a-fA-F]+)"\s*$') {
        $Hashes = @(
            [ordered]@{
                alg = "SHA-256"
                content = $Matches[1].ToLowerInvariant()
            }
        )
    }
    $ComponentsByKey[$Key] = New-Component "cargo" $Name $Version $Hashes
}

$ComponentKeys = [string[]]@($ComponentsByKey.Keys)
[Array]::Sort($ComponentKeys, [StringComparer]::Ordinal)
$Components = @($ComponentKeys | ForEach-Object { $ComponentsByKey[$_] })
$ProjectPurl = "pkg:generic/$([Uri]::EscapeDataString($PackageManifest.name))@$([Uri]::EscapeDataString($PackageManifest.version))"
$Bom = [ordered]@{
    '$schema' = "https://cyclonedx.org/schema/bom-1.6.schema.json"
    bomFormat = "CycloneDX"
    specVersion = "1.6"
    version = 1
    metadata = [ordered]@{
        component = [ordered]@{
            type = "application"
            "bom-ref" = $ProjectPurl
            name = $PackageManifest.name
            version = $PackageManifest.version
            purl = $ProjectPurl
        }
        properties = @(
            [ordered]@{
                name = "columnia:source:package-lock:sha256"
                value = Get-Sha256 $PackageLockPath
            },
            [ordered]@{
                name = "columnia:source:cargo-lock:sha256"
                value = Get-Sha256 $CargoLockPath
            }
        )
    }
    components = $Components
}

$OutputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$Json = $Bom | ConvertTo-Json -Depth 12
$Utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText($OutputPath, ($Json + "`n"), $Utf8WithoutBom)

Write-Host "SBOM CycloneDX generado: $($Components.Count) componentes."
