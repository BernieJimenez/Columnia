# Backs up and restores the real Columnia app data around native smokes.
# The smokes drive the installed-profile app, which writes to
# %APPDATA%\app.columnia.desktop; an aborted run used to leave synthetic
# projects or tasks in the person's catalog. The backup lives in the system
# temp directory, never under the repository, and is deleted after restoring.
# Restore only after every app process created by the smoke has exited.
#
# OPS-06: a pointer file records the pending backup, so a run killed before
# its restore is noticed by the next one instead of backing up the
# contaminated store as the "original". Restoring never deletes the store
# before the copy is in place, and every failure names the backup path.

function Get-ColumniaAppDataGuardPointer {
    Join-Path ([System.IO.Path]::GetTempPath()) "columnia-appdata-guard.pending.json"
}

function Get-DirectoryFingerprint {
    param([string]$Path)
    $files = @(Get-ChildItem -LiteralPath $Path -Recurse -File -Force)
    $bytes = ($files | Measure-Object -Property Length -Sum).Sum
    return "$($files.Count) archivos, $([long]$bytes) bytes"
}

function Backup-ColumniaAppData {
    $source = Join-Path $env:APPDATA "app.columnia.desktop"
    $pointer = Get-ColumniaAppDataGuardPointer
    if (Test-Path -LiteralPath $pointer -PathType Leaf) {
        $pending = Get-Content -Encoding UTF8 -LiteralPath $pointer -Raw | ConvertFrom-Json
        if ($pending.source -eq $source -and (-not $pending.existed -or (Test-Path -LiteralPath $pending.backup))) {
            $fix = if ($pending.existed) { "la copia original está en '$($pending.backup)': restáurala a mano en '$source'" } else { "'$source' no existía antes: bórrala" }
            throw "Una ejecución anterior no restauró los datos de Columnia; $fix y borra '$pointer'."
        }
        Remove-Item -LiteralPath $pointer -Force
    }
    $running = @(Get-Process -Name "columnia" -ErrorAction SilentlyContinue | Where-Object {
        # Builds under src-tauri\target are the smokes' own; the preflight stops them.
        $_.Path -and $_.Path -notlike "*\src-tauri\target\*"
    })
    if ($running.Count -gt 0) {
        throw "Columnia está abierta ($($running[0].Path)); ciérrala antes del smoke para que la copia de sus datos sea coherente."
    }
    $backup = Join-Path ([System.IO.Path]::GetTempPath()) ("columnia-appdata-" + [Guid]::NewGuid().ToString("N"))
    $existed = Test-Path -LiteralPath $source -PathType Container
    if ($existed) {
        Copy-Item -LiteralPath $source -Destination $backup -Recurse -Force
        $expected = Get-DirectoryFingerprint $source
        $copied = Get-DirectoryFingerprint $backup
        if ($copied -ne $expected) {
            Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue
            throw "La copia de seguridad de los datos de Columnia quedó incompleta ($copied de $expected); no se ejecuta el smoke."
        }
    }
    [ordered]@{ source = $source; backup = $backup; existed = $existed } |
        ConvertTo-Json | Set-Content -LiteralPath $pointer -Encoding utf8
    return [pscustomobject]@{
        Source = $source
        Backup = $backup
        Existed = $existed
    }
}

function Restore-ColumniaAppData {
    param([object]$Guard)

    if ($null -eq $Guard) {
        return $false
    }
    $displaced = $null
    if (Test-Path -LiteralPath $Guard.Source) {
        $displaced = "$($Guard.Source).restore-" + [Guid]::NewGuid().ToString("N")
        try {
            Rename-Item -LiteralPath $Guard.Source -NewName (Split-Path -Leaf $displaced) -ErrorAction Stop
        }
        catch {
            throw "No se pudieron restaurar los datos de Columnia: '$($Guard.Source)' está en uso ($($_.Exception.Message)). La copia original sigue en '$($Guard.Backup)'."
        }
    }
    if ($Guard.Existed) {
        try {
            Copy-Item -LiteralPath $Guard.Backup -Destination $Guard.Source -Recurse -Force -ErrorAction Stop
            $expected = Get-DirectoryFingerprint $Guard.Backup
            $restored = Get-DirectoryFingerprint $Guard.Source
            if ($restored -ne $expected) { throw "copia incompleta ($restored de $expected)" }
        }
        catch {
            $reason = $_.Exception.Message
            if (Test-Path -LiteralPath $Guard.Source) {
                Remove-Item -LiteralPath $Guard.Source -Recurse -Force -ErrorAction SilentlyContinue
            }
            if ($displaced -and -not (Test-Path -LiteralPath $Guard.Source)) {
                Rename-Item -LiteralPath $displaced -NewName (Split-Path -Leaf $Guard.Source) -ErrorAction SilentlyContinue
            }
            throw "No se pudieron restaurar los datos de Columnia ($reason). La copia original sigue en '$($Guard.Backup)'."
        }
    }
    if ($displaced) {
        Remove-Item -LiteralPath $displaced -Recurse -Force -ErrorAction SilentlyContinue
    }
    if ($Guard.Existed) {
        Remove-Item -LiteralPath $Guard.Backup -Recurse -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath (Get-ColumniaAppDataGuardPointer) -Force -ErrorAction SilentlyContinue
    return $true
}

# OPS-02: the NSIS installer writes per-user keys and shortcuts that every
# Columnia install shares; the smoke must leave them as it found them.
$ColumniaRegistryKeys = @(
    "HKCU:\Software\columnia",
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Columnia"
)
$ColumniaShortcuts = @(
    (Join-Path ([Environment]::GetFolderPath("Programs")) "Columnia.lnk"),
    (Join-Path ([Environment]::GetFolderPath("Desktop")) "Columnia.lnk")
)

function Get-ColumniaRegistrySnapshot {
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($root in $ColumniaRegistryKeys) {
        if (-not (Test-Path -LiteralPath $root)) { continue }
        foreach ($key in @(Get-Item -LiteralPath $root) + @(Get-ChildItem -LiteralPath $root -Recurse)) {
            $lines.Add($key.Name)
            foreach ($valueName in $key.GetValueNames()) {
                $lines.Add("$($key.Name)\[$valueName]=$($key.GetValue($valueName))")
            }
        }
    }
    foreach ($shortcut in $ColumniaShortcuts) {
        if (Test-Path -LiteralPath $shortcut) { $lines.Add("shortcut:$shortcut") }
    }
    return $lines.ToArray()
}

# OPS-25: what a smoke must leave as it found it: the per-user registration,
# the shortcuts and %APPDATA%\app.columnia.desktop (path, size and time of
# every file, so a change of the same size is seen too).
function Get-ColumniaMachineState {
    $appData = Join-Path $env:APPDATA "app.columnia.desktop"
    $files = @()
    if (Test-Path -LiteralPath $appData -PathType Container) {
        $files = @(Get-ChildItem -LiteralPath $appData -Recurse -File -Force | ForEach-Object {
            "appdata:$($_.FullName.Substring($appData.Length).TrimStart('\'))|$($_.Length)|$($_.LastWriteTimeUtc.Ticks)"
        })
    }
    return [pscustomobject]@{
        Lines = @(Get-ColumniaRegistrySnapshot) + $files
    }
}

function Compare-ColumniaMachineState {
    param([object]$Before, [object]$After)
    @($After.Lines | Where-Object { $Before.Lines -notcontains $_ } | ForEach-Object { "+ $_" }) +
        @($Before.Lines | Where-Object { $After.Lines -notcontains $_ } | ForEach-Object { "- $_" })
}

function Assert-NoInstalledColumniaRunning {
    $running = @(Get-Process -Name "columnia" -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -and $_.Path -notlike "*\src-tauri\target\*"
    })
    if ($running.Count -gt 0) {
        throw "Columnia está abierta ($($running[0].Path)); ciérrala antes del smoke."
    }
}
