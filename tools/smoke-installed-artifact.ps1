[CmdletBinding()]
param(
    [string]$InstallerPath,

    [ValidateRange(15, 600)]
    [int]$TimeoutSeconds = 90,

    [string]$ReportPath
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Package = Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json
$RunStartedAt = [DateTimeOffset]::UtcNow
$Stamp = $RunStartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/installer-smoke/$Stamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    Join-Path $EvidenceDirectory "summary.json"
}
elseif ([System.IO.Path]::IsPathRooted($ReportPath)) {
    $ReportPath
}
else {
    Join-Path $ProjectRoot $ReportPath
}
$InstallerFullPath = $null
$ScenarioRoot = $null
$ScenarioId = [Guid]::NewGuid().ToString("N")
$InstallRoot = $null
$DataRoot = $null
$LocalDataRoot = $null
$TempRoot = $null
$AppPath = $null
$UninstallerPath = $null
$SentinelPath = $null
$StartedProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$SmokeStatus = "failed"
$FailureMessage = $null
$CleanupConfirmed = $false
$IsAdministrator = $false
$IdentityName = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$InstallEvidence = [ordered]@{
    status = "not_started"
    durationMs = $null
    exitCode = $null
    pathUsesUnicodeAndSpaces = $false
}
$FirstOpenEvidence = [ordered]@{
    status = "not_started"
    durationMs = $null
    processId = $null
    windowTitle = $null
}
$SecondInstanceEvidence = [ordered]@{
    status = "not_started"
    durationMs = $null
    firstProcessId = $null
    secondProcessId = $null
    secondInvocationExitCode = $null
    distinctProcesses = $false
    bothWindowsVisible = $false
    duplicateProcessPrevented = $false
    firstProcessStillRunning = $false
}
$UninstallEvidence = [ordered]@{
    status = "not_started"
    durationMs = $null
    exitCode = $null
    executableRemoved = $false
}
$RetentionEvidence = [ordered]@{
    sentinelCreated = $false
    sentinelSurvivedUninstall = $false
    dataPolicy = "user-data-retained-by-default"
    dataScope = "current-user app data (app.columnia.desktop)"
}

function Get-ProcessSnapshot {
    @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
}

function Get-DescendantProcessIds {
    param([int[]]$RootIds)

    $processes = Get-ProcessSnapshot
    $seen = [System.Collections.Generic.HashSet[int]]::new()
    $pending = [System.Collections.Generic.Queue[int]]::new()
    foreach ($rootId in $RootIds) {
        if ($seen.Add([int]$rootId)) { $pending.Enqueue([int]$rootId) }
    }
    while ($pending.Count -gt 0) {
        $parentId = $pending.Dequeue()
        foreach ($child in $processes | Where-Object { [int]$_.ParentProcessId -eq $parentId }) {
            $childId = [int]$child.ProcessId
            if ($seen.Add($childId)) { $pending.Enqueue($childId) }
        }
    }
    @($seen.GetEnumerator() | ForEach-Object { [int]$_ })
}

function Wait-VisibleWindow {
    param(
        [System.Diagnostics.Process]$Process,
        [string]$Label
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        try {
            $Process.Refresh()
            if ($Process.HasExited) {
                throw "$Label terminó antes de mostrar una ventana (código $($Process.ExitCode))."
            }
            if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
                return [ordered]@{
                    processId = $Process.Id
                    windowTitle = $Process.MainWindowTitle
                }
            }
        }
        catch {
            if ($_.Exception.Message -like "$Label terminó*") { throw }
        }
        Start-Sleep -Milliseconds 250
    }
    throw "$Label no mostró una ventana dentro de $TimeoutSeconds segundos."
}

function Start-IsolatedColumnia {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $AppPath
    $startInfo.WorkingDirectory = $InstallRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.EnvironmentVariables["APPDATA"] = $DataRoot
    $startInfo.EnvironmentVariables["LOCALAPPDATA"] = $LocalDataRoot
    $startInfo.EnvironmentVariables["TEMP"] = $TempRoot
    $startInfo.EnvironmentVariables["TMP"] = $TempRoot
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "No se pudo iniciar el ejecutable instalado."
    }
    [void]$StartedProcessIds.Add($process.Id)
    return $process
}

function Stop-IsolatedProcesses {
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $ids = @(Get-DescendantProcessIds -RootIds @($StartedProcessIds.GetEnumerator() | ForEach-Object { [int]$_ }))
        foreach ($id in ($ids | Sort-Object -Descending)) {
            Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
        }
        $remaining = @($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        if ($remaining.Count -eq 0) { return $true }
        Start-Sleep -Milliseconds 200
    }
    $finalIds = @(Get-DescendantProcessIds -RootIds @($StartedProcessIds.GetEnumerator() | ForEach-Object { [int]$_ }))
    return @($finalIds | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }).Count -eq 0
}

function Wait-PathState {
    param(
        [string]$Path,
        [bool]$ExpectedPresent
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ((Test-Path -LiteralPath $Path) -eq $ExpectedPresent) { return $true }
        Start-Sleep -Milliseconds 250
    }
    return (Test-Path -LiteralPath $Path) -eq $ExpectedPresent
}

try {
    if ($env:OS -ne "Windows_NT") { throw "La prueba del instalador requiere Windows." }
    $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    $IsAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($IsAdministrator) {
        throw "La prueba debe ejecutarse con un usuario sin privilegios administrativos; no se permite falsear ese resultado."
    }

    if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
        $InstallerPath = Join-Path $ProjectRoot "src-tauri\target\release\bundle\nsis\Columnia_$($Package.version)_x64-setup.exe"
    }
    $InstallerFullPath = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $InstallerPath -ErrorAction Stop).Path)
    if ([System.IO.Path]::GetExtension($InstallerFullPath).ToLowerInvariant() -ne ".exe") {
        throw "El smoke instalado requiere el instalador NSIS .exe; no se ejecuta MSI sin una ruta separada de validación."
    }

    $ScenarioRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("Columnia Installer Smoke é " + $ScenarioId)
    $InstallRoot = Join-Path $ScenarioRoot "Instalación con espacios ñ"
    $DataRoot = Join-Path $ScenarioRoot "AppData aislada á"
    $LocalDataRoot = Join-Path $ScenarioRoot "LocalAppData aislada á"
    $TempRoot = Join-Path $ScenarioRoot "Temporales"
    $AppPath = Join-Path $InstallRoot "Columnia.exe"
    $UninstallerPath = Join-Path $InstallRoot "uninstall.exe"
    $SentinelPath = Join-Path (Join-Path $env:APPDATA "app.columnia.desktop") ".columnia-installer-smoke-$ScenarioId.txt"
    New-Item -ItemType Directory -Path $ScenarioRoot, $DataRoot, $LocalDataRoot, $TempRoot -Force | Out-Null
    $InstallEvidence.pathUsesUnicodeAndSpaces = $InstallRoot -match "[^\u0000-\u007F]" -and $InstallRoot -match "\s"

    $installTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $installerProcess = Start-Process -FilePath $InstallerFullPath -ArgumentList @("/S", "/D=$InstallRoot") -WindowStyle Hidden -Wait -PassThru
    $installTimer.Stop()
    $InstallEvidence.durationMs = $installTimer.ElapsedMilliseconds
    $InstallEvidence.exitCode = $installerProcess.ExitCode
    if ($installerProcess.ExitCode -ne 0) { throw "El instalador NSIS terminó con código $($installerProcess.ExitCode)." }
    if (-not (Wait-PathState -Path $AppPath -ExpectedPresent $true)) { throw "El instalador no creó Columnia.exe en la ruta Unicode esperada." }
    if (-not (Test-Path -LiteralPath $UninstallerPath -PathType Leaf)) { throw "El instalador no creó el desinstalador esperado." }
    $InstallEvidence.status = "passed"

    $firstTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $firstProcess = Start-IsolatedColumnia
    $firstWindow = Wait-VisibleWindow -Process $firstProcess -Label "La primera instancia"
    $firstTimer.Stop()
    $FirstOpenEvidence.status = "passed"
    $FirstOpenEvidence.durationMs = $firstTimer.ElapsedMilliseconds
    $FirstOpenEvidence.processId = $firstWindow.processId
    $FirstOpenEvidence.windowTitle = $firstWindow.windowTitle

    $AppDataDirectory = Split-Path -Parent $SentinelPath
    if (-not (Test-Path -LiteralPath $AppDataDirectory -PathType Container)) {
        throw "La primera apertura no creó el directorio de datos de usuario esperado: $AppDataDirectory"
    }
    Set-Content -LiteralPath $SentinelPath -Value "installer-smoke-retention" -Encoding utf8
    $RetentionEvidence.sentinelCreated = Test-Path -LiteralPath $SentinelPath -PathType Leaf
    if (-not $RetentionEvidence.sentinelCreated) { throw "No se pudo crear el sentinel de retención aislado." }

    $secondTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $secondProcess = Start-IsolatedColumnia
    $secondExited = $secondProcess.WaitForExit($TimeoutSeconds * 1000)
    $secondTimer.Stop()
    if (-not $secondExited) { throw "La segunda invocación no terminó dentro de $TimeoutSeconds segundos." }
    $firstProcess.Refresh()
    $firstProcessStillRunning = -not $firstProcess.HasExited -and $firstProcess.MainWindowHandle -ne [IntPtr]::Zero
    $SecondInstanceEvidence.status = "passed"
    $SecondInstanceEvidence.durationMs = $secondTimer.ElapsedMilliseconds
    $SecondInstanceEvidence.firstProcessId = $firstProcess.Id
    $SecondInstanceEvidence.secondProcessId = $secondProcess.Id
    $SecondInstanceEvidence.secondInvocationExitCode = $secondProcess.ExitCode
    $SecondInstanceEvidence.bothWindowsVisible = $firstProcessStillRunning
    $SecondInstanceEvidence.duplicateProcessPrevented = $secondProcess.HasExited -and $firstProcessStillRunning
    $SecondInstanceEvidence.firstProcessStillRunning = $firstProcessStillRunning
    if (-not $SecondInstanceEvidence.duplicateProcessPrevented) {
        throw "La segunda invocación no fue absorbida limpiamente por la política de instancia única."
    }

    if (-not (Stop-IsolatedProcesses)) { throw "No se pudieron cerrar únicamente los procesos creados por el smoke antes de desinstalar." }

    $uninstallTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $uninstallerProcess = Start-Process -FilePath $UninstallerPath -ArgumentList @("/S") -WindowStyle Hidden -Wait -PassThru
    $uninstallTimer.Stop()
    $UninstallEvidence.durationMs = $uninstallTimer.ElapsedMilliseconds
    $UninstallEvidence.exitCode = $uninstallerProcess.ExitCode
    if ($uninstallerProcess.ExitCode -ne 0) { throw "El desinstalador NSIS terminó con código $($uninstallerProcess.ExitCode)." }
    $UninstallEvidence.executableRemoved = Wait-PathState -Path $AppPath -ExpectedPresent $false
    if (-not $UninstallEvidence.executableRemoved) { throw "La desinstalación no retiró el ejecutable instalado." }
    $RetentionEvidence.sentinelSurvivedUninstall = Test-Path -LiteralPath $SentinelPath -PathType Leaf
    if (-not $RetentionEvidence.sentinelSurvivedUninstall) { throw "La desinstalación eliminó el sentinel de datos de usuario; la retención esperada no se cumplió." }
    $UninstallEvidence.status = "passed"

    $SmokeStatus = "passed"
}
catch {
    $FailureMessage = $_.Exception.Message
}
finally {
    $CleanupConfirmed = Stop-IsolatedProcesses
    if ($SentinelPath -and (Test-Path -LiteralPath $SentinelPath)) {
        Remove-Item -LiteralPath $SentinelPath -Force -ErrorAction SilentlyContinue
    }
    $scenarioStillExists = $false
    if ($ScenarioRoot -and (Test-Path -LiteralPath $ScenarioRoot)) {
        try {
            Remove-Item -LiteralPath $ScenarioRoot -Recurse -Force -ErrorAction Stop
        }
        catch {
            $scenarioStillExists = $true
        }
    }
    if (-not $CleanupConfirmed) {
        $SmokeStatus = "failed"
        $FailureMessage = if ($FailureMessage) { "$FailureMessage Cleanup incompleto de procesos." } else { "Cleanup incompleto de procesos." }
    }
    if ($scenarioStillExists) {
        $SmokeStatus = "failed"
        $FailureMessage = if ($FailureMessage) { "$FailureMessage Cleanup incompleto de la carpeta temporal." } else { "Cleanup incompleto de la carpeta temporal." }
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $SummaryPath) -Force | Out-Null
    [ordered]@{
        schemaVersion = 1
        status = $SmokeStatus
        generatedAt = [DateTimeOffset]::UtcNow.ToString("o")
        projectVersion = $Package.version
        artifact = [ordered]@{
            path = if ($InstallerFullPath) { $InstallerFullPath } else { $InstallerPath }
            kind = "nsis"
            sizeBytes = if ($InstallerFullPath -and (Test-Path -LiteralPath $InstallerFullPath)) { (Get-Item -LiteralPath $InstallerFullPath).Length } else { $null }
            sha256 = if ($InstallerFullPath -and (Test-Path -LiteralPath $InstallerFullPath)) { (Get-FileHash -LiteralPath $InstallerFullPath -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
        }
        identity = [ordered]@{
            user = $IdentityName
            isAdministrator = $IsAdministrator
            os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription.Trim()
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
        }
        install = $InstallEvidence
        firstOpen = $FirstOpenEvidence
        secondInstance = $SecondInstanceEvidence
        uninstall = $UninstallEvidence
        retention = $RetentionEvidence
        cleanup = [ordered]@{
            processesConfirmed = $CleanupConfirmed
            scenarioDirectoryRemoved = -not $scenarioStillExists
            sentinelRemoved = -not ($SentinelPath -and (Test-Path -LiteralPath $SentinelPath))
        }
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($SmokeStatus -ne "passed") {
    Write-Error "Smoke del instalador falló: $FailureMessage. Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Smoke del instalador aprobado: instalación, primera/segunda invocación, ruta Unicode, desinstalación y retención de datos de usuario confirmadas."
Write-Host "Evidencia: $EvidenceRelativePath"
