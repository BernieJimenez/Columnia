# Backs up and restores the real Columnia app data around native smokes.
# The smokes drive the installed-profile app, which writes to
# %APPDATA%\app.columnia.desktop; an aborted run used to leave synthetic
# projects or tasks in the person's catalog. The backup lives in the system
# temp directory, never under the repository, and is deleted after restoring.
# Restore only after every app process created by the smoke has exited.

function Backup-ColumniaAppData {
    $source = Join-Path $env:APPDATA "app.columnia.desktop"
    $backup = Join-Path ([System.IO.Path]::GetTempPath()) ("columnia-appdata-" + [Guid]::NewGuid().ToString("N"))
    $existed = Test-Path -LiteralPath $source -PathType Container
    if ($existed) {
        Copy-Item -LiteralPath $source -Destination $backup -Recurse -Force
    }
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
    if (Test-Path -LiteralPath $Guard.Source) {
        Remove-Item -LiteralPath $Guard.Source -Recurse -Force
    }
    if ($Guard.Existed) {
        Copy-Item -LiteralPath $Guard.Backup -Destination $Guard.Source -Recurse -Force
        Remove-Item -LiteralPath $Guard.Backup -Recurse -Force -ErrorAction SilentlyContinue
    }
    return $true
}
