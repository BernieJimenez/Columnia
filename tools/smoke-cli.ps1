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
    $StartInfo = New-Object System.Diagnostics.ProcessStartInfo
    $StartInfo.FileName = $script:CliPath
    $StartInfo.WorkingDirectory = $ProjectRoot
    $StartInfo.UseShellExecute = $false
    $StartInfo.CreateNoWindow = $true
    $StartInfo.RedirectStandardOutput = $true
    $StartInfo.RedirectStandardError = $true
    if ($StartInfo.PSObject.Properties.Name -contains "ArgumentList") {
        foreach ($Argument in $Arguments) {
            [void]$StartInfo.ArgumentList.Add($Argument)
        }
    }
    else {
        $StartInfo.Arguments = ($Arguments | ForEach-Object {
            if ($_ -notmatch '[\s"]') {
                $_
            }
            else {
                $Escaped = $_ -replace '(\\*)"', '$1$1\"'
                $Escaped = $Escaped -replace '(\\+)$', '$1$1'
                '"' + $Escaped + '"'
            }
        }) -join " "
    }

    $Process = New-Object System.Diagnostics.Process
    $Process.StartInfo = $StartInfo
    [void]$Process.Start()
    $StdoutTask = $Process.StandardOutput.ReadToEndAsync()
    $StderrTask = $Process.StandardError.ReadToEndAsync()
    $Process.WaitForExit()
    $Stdout = $StdoutTask.Result
    $Stderr = $StderrTask.Result
    $ExitCode = $Process.ExitCode
    $Process.Dispose()
    [System.IO.File]::WriteAllText($StdoutPath, $Stdout)
    [System.IO.File]::WriteAllText($StderrPath, $Stderr)

    if ($ShouldSucceed -and $ExitCode -ne 0) {
        throw "$Label falló con código $ExitCode."
    }
    if (-not $ShouldSucceed -and $ExitCode -eq 0) {
        throw "$Label debía fallar pero terminó correctamente."
    }
    $Checks.Add($Label)
    if ($null -eq $Stdout) { $Stdout = "" }
    if ($null -eq $Stderr) { $Stderr = "" }
    if ($Stderr.IndexOf($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 -or
        $Stderr.IndexOf($EvidenceDirectory, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "$Label expuso una ruta absoluta por stderr."
    }
    if ($Stderr -match "NativeCommandError|FullyQualifiedErrorId|CategoryInfo") {
        throw "$Label capturó metadatos de PowerShell en vez del stderr nativo."
    }
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

function New-DeterministicWorkbook {
    param([string]$Destination)

    $Staging = Join-Path $WorkDirectory "xlsx-source"
    $Archive = Join-Path $WorkDirectory "input.zip"
    New-Item -ItemType Directory -Path (Join-Path $Staging "_rels"), (Join-Path $Staging "xl\_rels"), (Join-Path $Staging "xl\worksheets") -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $Staging "[Content_Types].xml") -Encoding utf8 -Value '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>'
    Set-Content -LiteralPath (Join-Path $Staging "_rels\.rels") -Encoding utf8 -Value '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>'
    Set-Content -LiteralPath (Join-Path $Staging "xl\workbook.xml") -Encoding utf8 -Value '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>'
    Set-Content -LiteralPath (Join-Path $Staging "xl\_rels\workbook.xml.rels") -Encoding utf8 -Value '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>'
    Set-Content -LiteralPath (Join-Path $Staging "xl\worksheets\sheet1.xml") -Encoding utf8 -Value '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>name</t></is></c><c r="B1" t="inlineStr"><is><t>amount</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Alice</t></is></c><c r="B2"><v>10</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Bob</t></is></c><c r="B3"><v>20</v></c></row></sheetData></worksheet>'
    Compress-Archive -Path (Join-Path $Staging "*") -DestinationPath $Archive -CompressionLevel Optimal
    Move-Item -LiteralPath $Archive -Destination $Destination
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
    if ($Help.stdout -notmatch "(?i)inspect" -or $Help.stdout -notmatch "(?i)transform" -or $Help.stdout -notmatch "(?i)validate" -or $Help.stdout -notmatch "(?i)batch") {
        throw "La ayuda debe anunciar inspect, transform, validate y batch."
    }

    $InputRelative = "fixtures/automation/input.csv"
    $RecipeRelative = "fixtures/automation/recipe-v1.json"
    $CsvOutputRelative = "$EvidenceRelativePath/work/output.csv"
    $ParquetOutputRelative = "$EvidenceRelativePath/work/output.parquet"
    $WorkbookRelative = "$EvidenceRelativePath/work/input.xlsx"
    $WorkbookPath = Join-Path $WorkDirectory "input.xlsx"
    New-DeterministicWorkbook -Destination $WorkbookPath
    $BatchWorkDirectory = Join-Path $WorkDirectory "batch"
    New-Item -ItemType Directory -Path $BatchWorkDirectory -Force | Out-Null
    Copy-Item -LiteralPath @(
        (Join-Path $FixturesRoot "input.csv"),
        (Join-Path $FixturesRoot "recipe-v1.json"),
        (Join-Path $FixturesRoot "recipe-incompatible-v1.json"),
        (Join-Path $FixturesRoot "batch-success-v1.json"),
        (Join-Path $FixturesRoot "batch-invalid-v1.json"),
        (Join-Path $FixturesRoot "batch-collision-v1.json"),
        (Join-Path $FixturesRoot "batch-partial-v1.json")
    ) -Destination $BatchWorkDirectory

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

    $InspectWorkbook = Read-JsonOutput `
        -Label "inspect workbook" `
        -Result (Invoke-Cli -Label "inspect-workbook" -Arguments @(
            "inspect", "--input", $WorkbookRelative, "--sheet", "Data", "--header", "first-row"
        ))
    Assert-JsonFixture -Actual $InspectWorkbook -FixtureName "expected-inspect-workbook.json"

    $InspectWorkbookGenerated = Read-JsonOutput `
        -Label "inspect workbook generated headers" `
        -Result (Invoke-Cli -Label "inspect-workbook-generated" -Arguments @(
            "inspect", "--input", $WorkbookRelative, "--sheet", "Data", "--header", "generated"
        ))
    Assert-JsonFixture -Actual $InspectWorkbookGenerated -FixtureName "expected-inspect-workbook-generated.json"

    [void](Invoke-Cli -Label "workbook-missing-selection" -ShouldSucceed $false -Arguments @(
        "inspect", "--input", $WorkbookRelative
    ))

    $WorkbookOutputRelative = "$EvidenceRelativePath/work/workbook-output.csv"
    $WorkbookTransform = Read-JsonOutput `
        -Label "transform workbook" `
        -Result (Invoke-Cli -Label "transform-workbook" -Arguments @(
            "transform", "--input", $WorkbookRelative, "--sheet", "Data", "--header", "first-row",
            "--recipe", $RecipeRelative, "--output", $WorkbookOutputRelative, "--format", "csv"
        ))
    $WorkbookOutputPath = Join-Path $ProjectRoot ($WorkbookOutputRelative -replace "/", "\")
    Assert-DeepEqual `
        -Expected ([pscustomobject][ordered]@{
            schemaVersion = 1; command = "transform"; outputFileName = "workbook-output.csv"
            fileSizeBytes = (Get-Item $WorkbookOutputPath).Length; format = "CSV"; changed = $true
            summary = [pscustomobject][ordered]@{
                inputRowCount = 2; outputRowCount = 2; inputColumnCount = 2; outputColumnCount = 2
            }
        }) `
        -Actual $WorkbookTransform

    $BadWorkbookOutputRelative = "$EvidenceRelativePath/work/bad-workbook-output.csv"
    [void](Invoke-Cli -Label "workbook-invalid-sheet" -ShouldSucceed $false -Arguments @(
        "transform", "--input", $WorkbookRelative, "--sheet", "Missing", "--header", "generated",
        "--recipe", $RecipeRelative, "--output", $BadWorkbookOutputRelative, "--format", "csv"
    ))
    if (Test-Path -LiteralPath (Join-Path $ProjectRoot ($BadWorkbookOutputRelative -replace "/", "\"))) {
        throw "Una selección de hoja inválida dejó un archivo de salida parcial."
    }

    $QualityInputRelative = "fixtures/automation/quality-input.csv"
    $QualityPassRelative = "fixtures/automation/quality-pass-v1.json"
    $QualityFailRelative = "fixtures/automation/quality-fail-v1.json"
    $ValidatePass = Read-JsonOutput `
        -Label "validate pass" `
        -Result (Invoke-Cli -Label "validate-pass" -Arguments @(
            "validate", "--input", $QualityInputRelative, "--rules", $QualityPassRelative
        ))
    Assert-JsonFixture -Actual $ValidatePass -FixtureName "expected-validate-pass.json"

    $ValidateWorkbook = Read-JsonOutput `
        -Label "validate workbook" `
        -Result (Invoke-Cli -Label "validate-workbook" -Arguments @(
            "validate", "--input", $WorkbookRelative, "--sheet", "Data", "--header", "first-row",
            "--rules", $QualityPassRelative
        ))
    Assert-DeepEqual `
        -Expected ([pscustomobject][ordered]@{
            schemaVersion = 1; command = "validate"; passed = $true; rowCount = 2
            totalRules = 1; passedRules = 1; failedRules = 0; totalInvalidCount = 0
        }) `
        -Actual $ValidateWorkbook

    $ValidateFailResult = Invoke-Cli -Label "validate-fail" -ShouldSucceed $false -Arguments @(
        "validate", "--input", $QualityInputRelative, "--rules", $QualityFailRelative
    )
    if ($ValidateFailResult.exitCode -ne 2) {
        throw "Un contrato que no pasa debe terminar con código 2."
    }
    $ValidateFail = Read-JsonOutput -Label "validate fail" -Result $ValidateFailResult
    Assert-JsonFixture -Actual $ValidateFail -FixtureName "expected-validate-fail.json"

    $InvalidRulesRelative = "$EvidenceRelativePath/work/invalid-rules.json"
    Set-Content -LiteralPath (Join-Path $WorkDirectory "invalid-rules.json") -Value '{"version":2,"rules":[]}' -Encoding ascii
    $InvalidRulesResult = Invoke-Cli -Label "validate-invalid-rules" -ShouldSucceed $false -Arguments @(
        "validate", "--input", $QualityInputRelative, "--rules", $InvalidRulesRelative
    )
    if ($InvalidRulesResult.exitCode -ne 1 -or $InvalidRulesResult.stdout) {
        throw "Un contrato inválido debe ser error de uso/carga sin JSON parcial."
    }

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

    $BatchRelativeRoot = "$EvidenceRelativePath/work/batch"
    $BatchSuccess = Read-JsonOutput `
        -Label "batch success" `
        -Result (Invoke-Cli -Label "batch-success" -Arguments @(
            "batch", "--manifest", "$BatchRelativeRoot/batch-success-v1.json"
        ))
    Assert-JsonFixture -Actual $BatchSuccess -FixtureName "expected-batch-success.json"
    if (-not (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "batch-first.csv") -PathType Leaf) -or
        -not (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "batch-second.parquet") -PathType Leaf)) {
        throw "El batch exitoso no publicó todas sus salidas."
    }

    $BatchInvalid = Invoke-Cli -Label "batch-invalid-manifest" -ShouldSucceed $false -Arguments @(
        "batch", "--manifest", "$BatchRelativeRoot/batch-invalid-v1.json"
    )
    if ($BatchInvalid.exitCode -ne 1 -or $BatchInvalid.stdout -or
        (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "must-not-exist.csv"))) {
        throw "Un manifiesto batch inválido debe terminar con código 1 sin JSON ni outputs."
    }

    $BatchCollision = Invoke-Cli -Label "batch-output-collision" -ShouldSucceed $false -Arguments @(
        "batch", "--manifest", "$BatchRelativeRoot/batch-collision-v1.json"
    )
    if ($BatchCollision.exitCode -ne 1 -or $BatchCollision.stdout -or
        (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "collision.csv"))) {
        throw "Una colisión batch debe detectarse en preflight sin crear outputs."
    }

    $BatchPartialResult = Invoke-Cli -Label "batch-partial-failure" -ShouldSucceed $false -Arguments @(
        "batch", "--manifest", "$BatchRelativeRoot/batch-partial-v1.json"
    )
    if ($BatchPartialResult.exitCode -ne 2) {
        throw "Un trabajo batch fallido después del preflight debe terminar con código 2."
    }
    $BatchPartial = Read-JsonOutput -Label "batch partial failure" -Result $BatchPartialResult
    Assert-JsonFixture -Actual $BatchPartial -FixtureName "expected-batch-partial.json"
    if (-not (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "partial-completed.csv") -PathType Leaf) -or
        (Test-Path -LiteralPath (Join-Path $BatchWorkDirectory "partial-failed.csv"))) {
        throw "El fallo tardío batch debe conservar outputs completados y omitir el trabajo fallido."
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

Write-Host "Smoke CLI aprobado: help, inspect/transform de libros, CSV, Parquet, validate y batch con preflight y fallo parcial."
Write-Host "Evidencia: $EvidenceRelativePath"
