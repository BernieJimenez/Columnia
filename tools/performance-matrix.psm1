function Get-PerformanceScaleMatrixDefinition {
    param([string]$Path = (Join-Path (Split-Path -Parent $PSScriptRoot) "fixtures\performance\dataset-scale-matrix-v1.json"))

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No existe la matriz de escala: $Path"
    }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Get-PerformanceScaleProfile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProfileId,
        [Parameter(Mandatory = $true)]
        [object]$Definition
    )

    $Profile = @($Definition.profiles | Where-Object { $_.id -eq $ProfileId } | Select-Object -First 1)
    if ($Profile.Count -ne 1) {
        throw "El perfil '$ProfileId' no existe en la matriz de escala versionada."
    }
    return $Profile[0]
}

function Write-PerformanceScaleCsv {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Destination,
        [Parameter(Mandatory = $true)]
        [long]$TargetBytes,
        [Parameter(Mandatory = $true)]
        [object]$Profile
    )

    $Encoding = [System.Text.UTF8Encoding]::new($false)
    $Stream = [System.IO.FileStream]::new(
        $Destination,
        [System.IO.FileMode]::Create,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None,
        1024 * 1024,
        [System.IO.FileOptions]::SequentialScan
    )
    $Writer = [System.IO.StreamWriter]::new($Stream, $Encoding, 1024 * 1024)
    $Rows = 0
    $ColumnCount = [int]$Profile.columnCount
    $BaseNote = "Columnia benchmark deterministic payload 0123456789 abcdefghijklmnopqrstuvwxyz"
    $BaseNoteBytes = [System.Text.Encoding]::UTF8.GetByteCount($BaseNote)
    $NotesBytesPerRow = [int]$Profile.notesBytesPerRow
    if ($NotesBytesPerRow -lt $BaseNoteBytes) {
        $Writer.Dispose()
        $Stream.Dispose()
        throw "La longitud de texto del perfil no puede ser menor que el payload sintético base."
    }
    $Notes = $BaseNote + ("x" * ($NotesBytesPerRow - $BaseNoteBytes))
    $Headers = [System.Collections.Generic.List[string]]::new()
    foreach ($Header in @("id", "name", "amount", "notes")) {
        [void]$Headers.Add($Header)
    }
    for ($Column = 5; $Column -le $ColumnCount; $Column++) {
        [void]$Headers.Add(("dimension_{0:d2}" -f $Column))
    }
    $RowsPerBatch = [math]::Min(8192, [math]::Max(1, [math]::Floor(1MB / [math]::Max(1, $NotesBytesPerRow + ($ColumnCount * 12) + 48))))

    try {
        $Writer.WriteLine(($Headers -join ","))
        while ($Stream.Length -lt $TargetBytes) {
            for ($Index = 0; $Index -lt $RowsPerBatch -and $Stream.Length -lt $TargetBytes; $Index++) {
                $Rows++
                $Writer.Write("row-")
                $Writer.Write($Rows)
                $Writer.Write(",Synthetic name ")
                if ([string]$Profile.nameCardinality -eq "row") {
                    $Writer.Write($Rows)
                }
                else {
                    $Writer.Write(($Rows % [int]$Profile.nameCardinality))
                }
                $Writer.Write(",123.45,")
                $Writer.Write($Notes)
                for ($Column = 5; $Column -le $ColumnCount; $Column++) {
                    $Writer.Write(",value-")
                    $Writer.Write(($Rows % 16))
                }
                $Writer.WriteLine()
            }
            $Writer.Flush()
        }
    }
    finally {
        $Writer.Dispose()
        $Stream.Dispose()
    }

    $NameDistinctCount = if ([string]$Profile.nameCardinality -eq "row") {
        $Rows
    }
    else {
        [math]::Min($Rows, [int]$Profile.nameCardinality)
    }
    return [ordered]@{
        profileId = [string]$Profile.id
        rowCount = $Rows
        sizeBytes = (Get-Item -LiteralPath $Destination).Length
        columnCount = $ColumnCount
        nameDistinctCount = $NameDistinctCount
        notesDistinctCount = if ($Rows -gt 0) { 1 } else { 0 }
        notesBytesPerRow = $NotesBytesPerRow
        generatedColumnCount = $ColumnCount - 4
    }
}

Export-ModuleMember -Function Get-PerformanceScaleMatrixDefinition, Get-PerformanceScaleProfile, Write-PerformanceScaleCsv
