param()

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot "src-tauri"
$FixturesRoot = Join-Path $ProjectRoot "fixtures\automation"
$StartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $StartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/cli-smoke/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$WorkDirectory = Join-Path $EvidenceDirectory "work"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$Status = "failed"
$FailureMessage = $null
$Checks = [System.Collections.Generic.List[string]]::new()
$Timer = [System.Diagnostics.Stopwatch]::StartNew()

function Assert-DeepEqual {
    param(
        $Expected,
        $Actual,
        [string]$Path = "`$"
    )

    if ($null -eq $Expected -or $null -eq $Actual) {
        if ($null -ne $Expected -or $null -ne $Actual) {
            throw "JSON distinto en $Path."
        }
        return
    }

    if ($Expected -is [System.Management.Automation.PSCustomObject]) {
        if ($Actual -isnot [System.Management.Automation.PSCustomObject]) {
            throw "JSON distinto en ${Path}: se esperaba un objeto."
        }
        $ExpectedNames = @($Expected.PSObject.Properties.Name | Sort-Object)
        $ActualNames = @($Actual.PSObject.Properties.Name | Sort-Object)
        if (($ExpectedNames -join "|") -cne ($ActualNames -join "|")) {
            throw "JSON distinto en ${Path}: campos esperados $($ExpectedNames -join ', '), recibidos $($ActualNames -join ', ')."
        }
        foreach ($Name in $ExpectedNames) {
            Assert-DeepEqual -Expected $Expected.$Name -Actual $Actual.$Name -Path "$Path.$Name"
        }
        return
    }

    if ($Expected -is [System.Array]) {
        if ($Actual -isnot [System.Array] -or $Expected.Count -ne $Actual.Count) {
            throw "JSON distinto en ${Path}: longitud de arreglo inesperada."
        }
        for ($Index = 0; $Index -lt $Expected.Count; $Index++) {
            Assert-DeepEqual -Expected $Expected[$Index] -Actual $Actual[$Index] -Path "$Path[$Index]"
        }
        return
    }

    if ($Expected -is [ValueType] -and $Actual -is [ValueType]) {
        if ([decimal]$Expected -ne [decimal]$Actual) {
            throw "JSON distinto en ${Path}: esperado $Expected, recibido $Actual."
        }
        return
    }

    if ([string]$Expected -cne [string]$Actual) {
        throw "JSON distinto en ${Path}: esperado '$Expected', recibido '$Actual'."
    }
}

function Assert-NoPaths {
    param(
        $Value,
        [string]$Path = "`$"
    )

    if ($null -eq $Value) { return }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        foreach ($Property in $Value.PSObject.Properties) {
            Assert-NoPaths -Value $Property.Value -Path "$Path.$($Property.Name)"
        }
        return
    }
    if ($Value -is [System.Array]) {
        for ($Index = 0; $Index -lt $Value.Count; $Index++) {
            Assert-NoPaths -Value $Value[$Index] -Path "$Path[$Index]"
        }
        return
    }
    if ($Value -is [string] -and (
        [System.IO.Path]::IsPathRooted($Value) -or
        $Value.IndexOf($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase) -ge 0
    )) {
        throw "La respuesta JSON expone una ruta en $Path."
    }
}

function Invoke-Cli {
    param(
        [string]$Label,
        [string[]]$Arguments,
        [bool]$ShouldSucceed = $true
    )

    $SafeLabel = $Label -replace "[^a-zA-Z0-9-]", "-"
    $StdoutPath = Join-Path $EvidenceDirectory "$SafeLabel.stdout.log"
    $StderrPath = Join-Path $EvidenceDirectory "$SafeLabel.stderr.log"
    Push-Location $ProjectRoot
    $PreviousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $script:CliPath @Arguments 1> $StdoutPath 2> $StderrPath
        $ExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
        Pop-Location
    }

    if ($ShouldSucceed -and $ExitCode -ne 0) {
        throw "$Label falló con código $ExitCode."
    }
    if (-not $ShouldSucceed -and $ExitCode -eq 0) {
        throw "$Label debía fallar pero terminó correctamente."
    }
    $Checks.Add($Label)
    $Stdout = [string](Get-Content -LiteralPath $StdoutPath -Raw)
    $Stderr = [string](Get-Content -LiteralPath $StderrPath -Raw)
    if ($null -eq $Stdout) { $Stdout = "" }
    if ($null -eq $Stderr) { $Stderr = "" }
    return [ordered]@{
        stdout = $Stdout.Trim()
        stderr = $Stderr.Trim()
        exitCode = $ExitCode
    }
}

function Read-JsonOutput {
    param($Result, [string]$Label)

    try {
        $Document = $Result.stdout | ConvertFrom-Json
    }
    catch {
        throw "$Label no emitió un único documento JSON válido por stdout."
    }
    Assert-NoPaths -Value $Document
    return $Document
}

function Assert-JsonFixture {
    param($Actual, [string]$FixtureName)

    $Expected = Get-Content -LiteralPath (Join-Path $FixturesRoot $FixtureName) -Raw | ConvertFrom-Json
    Assert-DeepEqual -Expected $Expected -Actual $Actual
}

function New-ExpectedTransform {
    param(
        [string]$OutputFileName,
        [long]$FileSizeBytes,
        [string]$Format
    )

    [pscustomobject][ordered]@{
        schemaVersion = 1
        command = "transform"
        outputFileName = $OutputFileName
        fileSizeBytes = $FileSizeBytes
        format = $Format
        changed = $true
        summary = [pscustomobject][ordered]@{
            inputRowCount = 2
            outputRowCount = 2
            inputColumnCount = 3
            outputColumnCount = 3
        }
    }
}

New-Item -ItemType Directory -Path $WorkDirectory -Force | Out-Null

try {
    $BuildStdout = Join-Path $EvidenceDirectory "build.stdout.log"
    $BuildStderr = Join-Path $EvidenceDirectory "build.stderr.log"
    Push-Location $TauriRoot
    $PreviousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & cargo build --quiet --bin columnia-cli 1> $BuildStdout 2> $BuildStderr
        if ($LASTEXITCODE -ne 0) {
            throw "No se pudo compilar columnia-cli; revisa la evidencia local."
        }
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
        Pop-Location
    }

    $ExecutableName = if ($env:OS -eq "Windows_NT") { "columnia-cli.exe" } else { "columnia-cli" }
    $script:CliPath = Join-Path $TauriRoot "target\debug\$ExecutableName"
    if (-not (Test-Path -LiteralPath $script:CliPath -PathType Leaf)) {
        throw "La compilación no produjo columnia-cli."
    }

    $Help = Invoke-Cli -Label "help" -Arguments @("--help")
    if ($Help.stdout -notmatch "(?i)inspect" -or $Help.stdout -notmatch "(?i)transform") {
        throw "La ayuda debe anunciar inspect y transform."
    }

    $InputRelative = "fixtures/automation/input.csv"
    $RecipeRelative = "fixtures/automation/recipe-v1.json"
    $CsvOutputRelative = "$EvidenceRelativePath/work/output.csv"
    $ParquetOutputRelative = "$EvidenceRelativePath/work/output.parquet"

    $InspectInput = Read-JsonOutput `
        -Label "inspect input" `
        -Result (Invoke-Cli -Label "inspect-input" -Arguments @("inspect", "--input", $InputRelative))
    Assert-JsonFixture -Actual $InspectInput -FixtureName "expected-inspect-input.json"

    $TransformCsv = Read-JsonOutput `
        -Label "transform CSV" `
        -Result (Invoke-Cli -Label "transform-csv" -Arguments @(
            "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
            "--output", $CsvOutputRelative, "--format", "csv"
        ))
    $CsvOutputPath = Join-Path $ProjectRoot ($CsvOutputRelative -replace "/", "\")
    if (-not (Test-Path -LiteralPath $CsvOutputPath -PathType Leaf)) {
        throw "transform CSV no creó su archivo de salida."
    }
    Assert-DeepEqual `
        -Expected (New-ExpectedTransform -OutputFileName "output.csv" -FileSizeBytes (Get-Item $CsvOutputPath).Length -Format "CSV") `
        -Actual $TransformCsv

    $ExpectedCsv = (Get-Content -LiteralPath (Join-Path $FixturesRoot "expected-output.csv") -Raw) -replace "`r`n", "`n"
    $ActualCsv = (Get-Content -LiteralPath $CsvOutputPath -Raw) -replace "`r`n", "`n"
    if ($ActualCsv -cne $ExpectedCsv) {
        throw "El CSV transformado no coincide con el fixture, incluida la neutralización de fórmula."
    }
    if ($ActualCsv -notmatch "(?m)^Alice,'=2\+2,10$") {
        throw "El CSV transformado no neutralizó explícitamente la celda de fórmula."
    }

    $InspectCsv = Read-JsonOutput `
        -Label "inspect output CSV" `
        -Result (Invoke-Cli -Label "inspect-output-csv" -Arguments @("inspect", "--input", $CsvOutputRelative))
    Assert-JsonFixture -Actual $InspectCsv -FixtureName "expected-inspect-output-csv.json"

    $TransformParquet = Read-JsonOutput `
        -Label "transform Parquet" `
        -Result (Invoke-Cli -Label "transform-parquet" -Arguments @(
            "transform", "--input", $InputRelative, "--recipe", $RecipeRelative,
            "--output", $ParquetOutputRelative, "--format", "parquet"
        ))
    $ParquetOutputPath = Join-Path $ProjectRoot ($ParquetOutputRelative -replace "/", "\")
    if (-not (Test-Path -LiteralPath $ParquetOutputPath -PathType Leaf) -or (Get-Item $ParquetOutputPath).Length -eq 0) {
        throw "transform Parquet no creó un archivo válido no vacío."
    }
    Assert-DeepEqual `
        -Expected (New-ExpectedTransform -OutputFileName "output.parquet" -FileSizeBytes (Get-Item $ParquetOutputPath).Length -Format "Parquet") `
        -Actual $TransformParquet

    $InspectParquet = Read-JsonOutput `
        -Label "inspect output Parquet" `
        -Result (Invoke-Cli -Label "inspect-output-parquet" -Arguments @("inspect", "--input", $ParquetOutputRelative))
    Assert-JsonFixture -Actual $InspectParquet -FixtureName "expected-inspect-output-parquet.json"

    $InvalidRecipeRelative = "$EvidenceRelativePath/work/invalid-recipe.json"
    Set-Content -LiteralPath (Join-Path $WorkDirectory "invalid-recipe.json") -Value '{"version":999}' -Encoding ascii
    $InvalidRecipeOutputRelative = "$EvidenceRelativePath/work/invalid-recipe-output.csv"
    [void](Invoke-Cli -Label "invalid-recipe" -ShouldSucceed $false -Arguments @(
        "transform", "--input", $InputRelative, "--recipe", $InvalidRecipeRelative,
        "--output", $InvalidRecipeOutputRelative, "--format", "csv"
    ))
    if (Test-Path -LiteralPath (Join-Path $ProjectRoot ($InvalidRecipeOutputRelative -replace "/", "\"))) {
        throw "Una receta inválida dejó un archivo de salida parcial."
    }

    $MissingInputOutputRelative = "$EvidenceRelativePath/work/missing-input-output.csv"
    [void](Invoke-Cli -Label "missing-input" -ShouldSucceed $false -Arguments @(
        "transform", "--input", "fixtures/automation/missing.csv", "--recipe", $RecipeRelative,
        "--output", $MissingInputOutputRelative, "--format", "csv"
    ))
    if (Test-Path -LiteralPath (Join-Path $ProjectRoot ($MissingInputOutputRelative -replace "/", "\"))) {
        throw "Un input inexistente dejó un archivo de salida parcial."
    }

    $Status = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
    $_ | Format-List * -Force | Out-String | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "failure.log") -Encoding utf8
}
finally {
    if (Test-Path -LiteralPath $WorkDirectory) {
        $ResolvedWork = [System.IO.Path]::GetFullPath($WorkDirectory)
        $ResolvedEvidence = [System.IO.Path]::GetFullPath($EvidenceDirectory)
        if (-not $ResolvedWork.StartsWith($ResolvedEvidence + [System.IO.Path]::DirectorySeparatorChar)) {
            $Status = "failed"
            $FailureMessage = "Cleanup rechazado porque el directorio temporal salió del área de evidencia."
        }
        else {
            Remove-Item -LiteralPath $ResolvedWork -Recurse -Force
        }
    }
    $Timer.Stop()
    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        durationMs = $Timer.ElapsedMilliseconds
        command = "columnia-cli"
        evidenceDirectory = $EvidenceRelativePath
        checks = @($Checks)
        cleanupConfirmed = -not (Test-Path -LiteralPath $WorkDirectory)
        error = $FailureMessage
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Smoke CLI aprobado: help, inspect, CSV, Parquet y errores sin outputs parciales."
Write-Host "Evidencia: $EvidenceRelativePath"
