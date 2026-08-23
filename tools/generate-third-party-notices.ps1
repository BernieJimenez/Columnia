[CmdletBinding()]
param(
    [string]$OutputPath = "THIRD_PARTY_NOTICES.md",
    [switch]$Check
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$ProjectRootUri = [Uri]::new("$ProjectRoot\")
if (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot $OutputPath
}

function Normalize-License {
    param($License)
    if ($null -eq $License) { return "UNKNOWN" }
    if ($License -is [string]) { return $License }
    if ($License.type) { return [string]$License.type }
    if ($License.name) { return [string]$License.name }
    return "UNKNOWN"
}

function Invoke-Captured {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList,
        [string]$WorkingDirectory,
        [switch]$AllowFailure
    )
    $Token = [Guid]::NewGuid().ToString("N")
    $StdoutPath = Join-Path $env:TEMP "columnia-notices-$Token.out"
    $StderrPath = Join-Path $env:TEMP "columnia-notices-$Token.err"
    try {
        $Process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -WorkingDirectory $WorkingDirectory -Wait -PassThru -WindowStyle Hidden -RedirectStandardOutput $StdoutPath -RedirectStandardError $StderrPath
        $Output = if (Test-Path -LiteralPath $StdoutPath) { Get-Content -LiteralPath $StdoutPath -Raw } else { "" }
        if ($Process.ExitCode -ne 0 -and -not $AllowFailure) {
            throw "$FilePath terminó con código $($Process.ExitCode): $((Get-Content -LiteralPath $StderrPath -Raw).Trim())"
        }
        return $Output
    }
    finally {
        Remove-Item -LiteralPath $StdoutPath, $StderrPath -Force -ErrorAction SilentlyContinue
    }
}

$Rows = [System.Collections.Generic.List[object]]::new()
$NpmOutput = Invoke-Captured -FilePath "npm.cmd" -ArgumentList @("ls", "--all", "--json", "--omit=optional", "--include=dev") -WorkingDirectory $ProjectRoot -AllowFailure
if ([string]::IsNullOrWhiteSpace($NpmOutput)) {
    throw "npm ls no pudo producir el inventario de dependencias."
}
$NpmTree = $NpmOutput | ConvertFrom-Json

function Add-NpmNode {
    param($Node, [string]$FallbackName = "")
    if ($null -eq $Node) { return }
    $Name = if ($Node.name) { [string]$Node.name } else { $FallbackName }
    if ($Name -and $Node.version -and $Name -ne "columnia") {
        $Rows.Add([ordered]@{
                ecosystem = "npm"
                name = $Name
                version = [string]$Node.version
                license = Normalize-License $Node.license
                source = if ($Node.resolved) { [string]$Node.resolved } else { "package-lock.json" }
            })
    }
    foreach ($Property in @($Node.dependencies.PSObject.Properties)) {
        Add-NpmNode -Node $Property.Value -FallbackName $Property.Name
    }
}
Add-NpmNode -Node $NpmTree

$CargoOutput = Invoke-Captured -FilePath "cargo.exe" -ArgumentList @("metadata", "--format-version", "1", "--locked", "--manifest-path", (Join-Path $TauriRoot "Cargo.toml")) -WorkingDirectory $TauriRoot
$CargoMetadataPath = Join-Path $env:TEMP "columnia-cargo-metadata-$([Guid]::NewGuid().ToString('N')).json"
[System.IO.File]::WriteAllText($CargoMetadataPath, $CargoOutput, [System.Text.UTF8Encoding]::new($false))
try {
    $CargoRowsJson = & node.exe (Join-Path $PSScriptRoot "extract-cargo-notice-packages.mjs") $CargoMetadataPath
    if ($LASTEXITCODE -ne 0) { throw "node no pudo leer cargo metadata." }
    $CargoPackages = $CargoRowsJson | ConvertFrom-Json
}
finally {
    Remove-Item -LiteralPath $CargoMetadataPath -Force -ErrorAction SilentlyContinue
}
foreach ($Package in @($CargoPackages)) {
    $Rows.Add([ordered]@{
            ecosystem = "cargo"
            name = [string]$Package.name
            version = [string]$Package.version
            license = Normalize-License $Package.license
            source = if ($Package.source) { [string]$Package.source } else { "Cargo.lock" }
        })
}

$UniqueRows = @($Rows | Sort-Object ecosystem, name, version, source)
$Lines = [System.Collections.Generic.List[string]]::new()
$Lines.Add("# Third-party notices")
$Lines.Add("")
$Lines.Add("Este indice se genera desde `package-lock.json` y `src-tauri/Cargo.lock`. No contiene datos de usuario ni secretos.")
$Lines.Add("Los paquetes conservan sus avisos y licencias segun sus metadatos de distribucion; antes de publicar se debe revisar cualquier licencia marcada como `UNKNOWN`.")
$Lines.Add("")
$Lines.Add("| Ecosistema | Paquete | Version | Licencia | Fuente |")
$Lines.Add("| --- | --- | --- | --- | --- |")
foreach ($Row in $UniqueRows) {
    $Lines.Add("| $($Row.ecosystem) | $($Row.name) | $($Row.version) | $($Row.license) | $($Row.source) |")
}
$Lines.Add("")
$Lines.Add("Total: $($UniqueRows.Count) dependencias de terceros.")
$Expected = ($Lines -join "`n") + "`n"

if ($Check) {
    if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
        throw "Falta el inventario $($ProjectRootUri.MakeRelativeUri([Uri]$OutputPath).ToString()). Ejecuta generate-third-party-notices.ps1."
    }
    $Actual = [System.IO.File]::ReadAllText($OutputPath)
    if ($Actual -cne $Expected) {
        throw "THIRD_PARTY_NOTICES.md no coincide con los lockfiles actuales."
    }
    Write-Host "Third-party notices aprobados: $($UniqueRows.Count) dependencias."
    exit 0
}

[System.IO.File]::WriteAllText($OutputPath, $Expected, [System.Text.UTF8Encoding]::new($false))
Write-Host "Third-party notices generados: $($ProjectRootUri.MakeRelativeUri([Uri]$OutputPath).ToString()) ($($UniqueRows.Count) dependencias)."
