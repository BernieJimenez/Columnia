$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Import-Module (Join-Path $PSScriptRoot "performance-matrix.psm1") -Force
$Definition = Get-PerformanceScaleMatrixDefinition -Path (Join-Path $ProjectRoot "fixtures\performance\dataset-scale-matrix-v1.json")
$TempRoot = [System.IO.Path]::GetTempPath()
$TestDirectory = Join-Path $TempRoot ("columnia-performance-matrix-" + [Guid]::NewGuid().ToString("N"))
$TargetBytes = 64KB
$Failures = [System.Collections.Generic.List[string]]::new()
New-Item -ItemType Directory -Path $TestDirectory -Force | Out-Null

try {
    foreach ($Profile in $Definition.profiles) {
        $CsvPath = Join-Path $TestDirectory ($Profile.id + ".csv")
        $Info = Write-PerformanceScaleCsv -Destination $CsvPath -TargetBytes $TargetBytes -Profile $Profile
        $Rows = @(Import-Csv -LiteralPath $CsvPath)
        $ActualColumnCount = if ($Rows.Count -eq 0) { 0 } else { @($Rows[0].PSObject.Properties.Name).Count }
        $UniqueIds = @($Rows.id | Sort-Object -Unique).Count
        $UniqueNames = @($Rows.name | Sort-Object -Unique).Count
        $UniqueNotes = @($Rows.notes | Sort-Object -Unique).Count
        $NoteByteCounts = @($Rows.notes | ForEach-Object { [System.Text.Encoding]::UTF8.GetByteCount([string]$_) } | Sort-Object -Unique)
        $ExpectedNameCardinality = if ([string]$Profile.nameCardinality -eq "row") {
            $Rows.Count
        }
        else {
            [math]::Min($Rows.Count, [int]$Profile.nameCardinality)
        }

        if ($Info.profileId -ne $Profile.id) { [void]$Failures.Add("$($Profile.id): profileId no coincide.") }
        if ($Info.rowCount -ne $Rows.Count -or $Rows.Count -le 0) { [void]$Failures.Add("$($Profile.id): rowCount no coincide con el CSV.") }
        if ($Info.sizeBytes -lt $TargetBytes) { [void]$Failures.Add("$($Profile.id): tamaño menor al objetivo.") }
        if ($Info.columnCount -ne $Profile.columnCount -or $ActualColumnCount -ne $Profile.columnCount) { [void]$Failures.Add("$($Profile.id): ancho incorrecto.") }
        if ($Info.nameDistinctCount -ne $ExpectedNameCardinality -or $UniqueNames -ne $ExpectedNameCardinality) { [void]$Failures.Add("$($Profile.id): cardinalidad de nombre incorrecta.") }
        if ($Info.rowCount -ne $UniqueIds) { [void]$Failures.Add("$($Profile.id): los identificadores no son únicos.") }
        if ($Info.notesDistinctCount -ne $UniqueNotes -or $UniqueNotes -ne 1) { [void]$Failures.Add("$($Profile.id): cardinalidad del texto incorrecta.") }
        if ($NoteByteCounts.Count -ne 1 -or $NoteByteCounts[0] -ne $Profile.notesBytesPerRow -or $Info.notesBytesPerRow -ne $Profile.notesBytesPerRow) {
            [void]$Failures.Add("$($Profile.id): longitud UTF-8 de notes incorrecta.")
        }
        if ($Info.generatedColumnCount -ne ($Profile.columnCount - 4)) { [void]$Failures.Add("$($Profile.id): conteo de columnas generadas incorrecto.") }
    }
}
finally {
    $ResolvedTempRoot = [System.IO.Path]::GetFullPath($TempRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    $ResolvedTestDirectory = [System.IO.Path]::GetFullPath($TestDirectory)
    if (-not $ResolvedTestDirectory.StartsWith($ResolvedTempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Cleanup de prueba rechazado: el destino quedó fuera del directorio temporal."
    }
    Remove-Item -LiteralPath $ResolvedTestDirectory -Recurse -Force
}

if ($Failures.Count -gt 0) {
    throw ($Failures -join [Environment]::NewLine)
}

Write-Host "Synthetic scale generation passed: width, cardinality, UTF-8 text bytes, size, and counts validated in $($Definition.profiles.Count) profiles."
