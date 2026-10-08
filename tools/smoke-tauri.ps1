param(
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 180
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "app-data-guard.ps1")
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace ColumniaDesktopSmoke {
    public static class NativeMethods {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateJobObject(IntPtr attributes, string name);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool TerminateJobObject(IntPtr job, uint exitCode);

        [DllImport("kernel32.dll")]
        public static extern bool CloseHandle(IntPtr handle);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool IsWindowVisible(IntPtr window);
    }
}
"@

$ProjectRoot = Split-Path -Parent $PSScriptRoot
# OPS-24: the common evidence header (commit, tree, version, times).
Import-Module (Join-Path $PSScriptRoot "evidence.psm1") -Force
$EvidenceStartedAt = [DateTimeOffset]::UtcNow.ToString("o")
$DebugExecutable = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "src-tauri\target\debug\columnia.exe"))
$RunStartedAt = [DateTimeOffset]::UtcNow
$Timestamp = $RunStartedAt.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/desktop-smoke/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$StdoutPath = Join-Path $EvidenceDirectory "stdout.log"
$StderrPath = Join-Path $EvidenceDirectory "stderr.log"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$RootProcess = $null
$JobHandle = [IntPtr]::Zero
$TrackedProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$OwnedListenerProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$OwnedDesktopProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$ViteReady = $false
$DesktopReady = $false
$ProjectsPanelContractStatus = "not_checked"
$ProjectsPanelRuntimeStatus = "not_checked"
$ProjectsPanelWindowName = $null
$ProjectsPanelWindowDescendantCount = 0
$ProjectsPanelWindowNote = $null
$CleanupConfirmed = $false
$CleanupAttempts = 0
$CleanupRetryCount = 0
$CleanupElapsedMs = 0
$SmokeStatus = "failed"
$FailureMessage = $null
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$Milestones = [ordered]@{
    viteReady = [ordered]@{
        reached = $false
        elapsedMs = $null
    }
    desktopProcessReady = [ordered]@{
        reached = $false
        elapsedMs = $null
    }
    windowVisible = [ordered]@{
        reached = $false
        elapsedMs = $null
    }
}

function Get-PortListeners {
    @(Get-NetTCPConnection -State Listen -LocalPort 1420 -ErrorAction SilentlyContinue)
}

function Get-DebugAppProcesses {
    @(
        Get-CimInstance Win32_Process -Filter "Name = 'columnia.exe'" -ErrorAction SilentlyContinue |
            Where-Object {
                $_.ExecutablePath -and
                [string]::Equals(
                    [System.IO.Path]::GetFullPath($_.ExecutablePath),
                    $DebugExecutable,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            }
    )
}

function Mark-Milestone {
    param([ValidateSet("viteReady", "desktopProcessReady", "windowVisible")][string]$Name)

    if (-not $Milestones[$Name]["reached"]) {
        $Milestones[$Name]["reached"] = $true
        $Milestones[$Name]["elapsedMs"] = $Timer.ElapsedMilliseconds
    }
}

function Test-VisibleDesktopProcess {
    param([int]$Id)

    $Candidate = Get-Process -Id $Id -ErrorAction SilentlyContinue
    if ($null -eq $Candidate -or $Candidate.MainWindowHandle -eq [IntPtr]::Zero) {
        return $false
    }
    return [ColumniaDesktopSmoke.NativeMethods]::IsWindowVisible($Candidate.MainWindowHandle)
}

# QA-22: the projects flow is checked by its real tests, not by looking for
# strings in the source.
function Test-ProjectsPanelContract {
    $Npx = (Get-Command npx.cmd -ErrorAction Stop).Source
    & $Npx vitest run src/features/projects --reporter=dot | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Preflight de ProjectsPanel falló: las pruebas de src/features/projects no pasan."
    }
}

function Get-ProjectsPanelRuntimeEvidence {
    param([int[]]$ProcessIds)

    $script:ProjectsPanelRuntimeStatus = "not_available"
    $script:ProjectsPanelWindowName = $null
    $script:ProjectsPanelWindowDescendantCount = 0
    $script:ProjectsPanelWindowNote = "WebView2 no expone de forma estable el DOM de React mediante UI Automation; no se simulan clics ni se activan capacidades de depuración."

    try {
        Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes -ErrorAction Stop
    }
    catch {
        $script:ProjectsPanelWindowNote = "UI Automation del sistema no está disponible en este entorno; se conserva únicamente el preflight de contrato."
        return
    }

    foreach ($processId in $ProcessIds) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($null -eq $process -or $process.MainWindowHandle -eq [IntPtr]::Zero) {
            continue
        }

        try {
            $window = [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
            if ($null -eq $window) {
                continue
            }
            $script:ProjectsPanelWindowName = $window.Current.Name
            $descendants = $window.FindAll(
                [System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.Condition]::TrueCondition
            )
            $script:ProjectsPanelWindowDescendantCount = $descendants.Count
            if ([string]::Equals($window.Current.Name, "Columnia", [StringComparison]::Ordinal)) {
                $script:ProjectsPanelRuntimeStatus = "window_ready"
            }
            return
        }
        catch {
            $script:ProjectsPanelWindowNote = "La ventana debug inició, pero UI Automation no pudo leer su árbol de accesibilidad."
            return
        }
    }
}

function Add-ProcessTree {
    param([int]$RootId)

    $Processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
    $Pending = [System.Collections.Generic.Queue[int]]::new()
    $Pending.Enqueue($RootId)
    while ($Pending.Count -gt 0) {
        $ParentId = $Pending.Dequeue()
        foreach ($Child in $Processes | Where-Object { [int]$_.ParentProcessId -eq $ParentId }) {
            $ChildId = [int]$Child.ProcessId
            if ($TrackedProcessIds.Add($ChildId)) {
                $Pending.Enqueue($ChildId)
            }
        }
    }
}

function Test-OwnedProcess {
    param([int]$Id)

    if ($JobHandle -eq [IntPtr]::Zero) {
        return $false
    }
    $Candidate = Get-Process -Id $Id -ErrorAction SilentlyContinue
    if ($null -eq $Candidate) {
        return $false
    }

    $BelongsToJob = $false
    if (-not [ColumniaDesktopSmoke.NativeMethods]::IsProcessInJob(
        $Candidate.Handle,
        $JobHandle,
        [ref]$BelongsToJob
    )) {
        return $false
    }
    return $BelongsToJob
}

function Stop-CreatedProcesses {
    $CleanupTimer = [System.Diagnostics.Stopwatch]::StartNew()
    $AttemptWindowsSeconds = @(4, 3)
    try {
        if ($null -ne $RootProcess) {
            [void]$TrackedProcessIds.Add($RootProcess.Id)
            Add-ProcessTree -RootId $RootProcess.Id
        }

        $OrderedIds = @($TrackedProcessIds) | Sort-Object -Descending
        for ($Attempt = 1; $Attempt -le $AttemptWindowsSeconds.Count; $Attempt++) {
            $script:CleanupAttempts = $Attempt
            if ($Attempt -gt 1) {
                $script:CleanupRetryCount++
            }

            if ($JobHandle -ne [IntPtr]::Zero) {
                [void][ColumniaDesktopSmoke.NativeMethods]::TerminateJobObject($JobHandle, 1)
            }
            foreach ($ProcessId in $OrderedIds) {
                Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
            }

            $CleanupDeadline = [DateTimeOffset]::UtcNow.AddSeconds($AttemptWindowsSeconds[$Attempt - 1])
            do {
                $RemainingTracked = @($TrackedProcessIds | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
                $RemainingOwnedListeners = @(
                    Get-PortListeners |
                        Where-Object { $OwnedListenerProcessIds.Contains([int]$_.OwningProcess) }
                )
                $RemainingOwnedApps = @(
                    Get-DebugAppProcesses |
                        Where-Object { $OwnedDesktopProcessIds.Contains([int]$_.ProcessId) }
                )
                if (
                    $RemainingTracked.Count -eq 0 -and
                    $RemainingOwnedListeners.Count -eq 0 -and
                    $RemainingOwnedApps.Count -eq 0
                ) {
                    if ($JobHandle -ne [IntPtr]::Zero) {
                        [void][ColumniaDesktopSmoke.NativeMethods]::CloseHandle($JobHandle)
                        $script:JobHandle = [IntPtr]::Zero
                    }
                    return $true
                }
                Start-Sleep -Milliseconds 250
            } while ([DateTimeOffset]::UtcNow -lt $CleanupDeadline)
        }

        return $false
    }
    finally {
        $CleanupTimer.Stop()
        $script:CleanupElapsedMs = $CleanupTimer.ElapsedMilliseconds
        if ($JobHandle -ne [IntPtr]::Zero) {
            [void][ColumniaDesktopSmoke.NativeMethods]::CloseHandle($JobHandle)
            $script:JobHandle = [IntPtr]::Zero
        }
    }
}

$AppDataGuard = $null
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null

try {
    if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
        throw "El smoke de escritorio está disponible únicamente en Windows."
    }
    try {
        Test-ProjectsPanelContract
        $ProjectsPanelContractStatus = "passed"
    }
    catch {
        $ProjectsPanelContractStatus = "failed"
        throw
    }
    if (@(Get-PortListeners).Count -gt 0) {
        throw "Preflight falló: el puerto de desarrollo 1420 ya tiene un listener activo."
    }
    if (@(Get-DebugAppProcesses).Count -gt 0) {
        throw "Preflight falló: la aplicación debug de Columnia ya está activa."
    }
    # QA-22: the debug app opens the real catalog; it is restored afterwards.
    $AppDataGuard = Backup-ColumniaAppData

    $NpmCommand = (Get-Command npm.cmd -ErrorAction Stop).Source
    $NodeCommand = (Get-Command node.exe -ErrorAction Stop).Source
    $NpmCli = Join-Path (Split-Path -Parent $NpmCommand) "node_modules\npm\bin\npm-cli.js"
    if (-not (Test-Path -LiteralPath $NpmCli -PathType Leaf)) {
        throw "No se encontró el CLI local de npm necesario para ejecutar npm run tauri dev."
    }
    $JobHandle = [ColumniaDesktopSmoke.NativeMethods]::CreateJobObject([IntPtr]::Zero, $null)
    if ($JobHandle -eq [IntPtr]::Zero) {
        throw "No se pudo crear el Job Object aislado para el smoke."
    }
    $RootProcess = Start-Process `
        -FilePath $NodeCommand `
        -ArgumentList @("`"$NpmCli`"", "run", "tauri", "dev") `
        -WorkingDirectory $ProjectRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $StdoutPath `
        -RedirectStandardError $StderrPath `
        -PassThru
    if (-not [ColumniaDesktopSmoke.NativeMethods]::AssignProcessToJobObject(
        $JobHandle,
        $RootProcess.Handle
    )) {
        throw "No se pudo aislar el proceso raíz del smoke en su Job Object."
    }
    [void]$TrackedProcessIds.Add($RootProcess.Id)

    $Deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        Add-ProcessTree -RootId $RootProcess.Id

        $Listeners = @(Get-PortListeners)
        if ($Listeners.Count -gt 0) {
            # El listener puede aparecer justo después de la primera instantánea del árbol.
            Add-ProcessTree -RootId $RootProcess.Id
        }
        $OwnedListeners = @(
            $Listeners |
                Where-Object { Test-OwnedProcess -Id ([int]$_.OwningProcess) }
        )
        $ForeignListeners = @(
            $Listeners |
                Where-Object { -not (Test-OwnedProcess -Id ([int]$_.OwningProcess)) }
        )
        if ($ForeignListeners.Count -gt 0) {
            throw "Apareció un listener ajeno en el puerto 1420 durante el smoke; no se terminará ese proceso."
        }
        if ($OwnedListeners.Count -gt 0) {
            $ViteReady = $true
            Mark-Milestone -Name "viteReady"
            foreach ($Listener in $OwnedListeners) {
                [void]$OwnedListenerProcessIds.Add([int]$Listener.OwningProcess)
                [void]$TrackedProcessIds.Add([int]$Listener.OwningProcess)
            }
        }

        $DesktopProcesses = @(Get-DebugAppProcesses)
        if ($DesktopProcesses.Count -gt 0) {
            # La aplicación puede arrancar entre la captura anterior y esta consulta.
            Add-ProcessTree -RootId $RootProcess.Id
        }
        $OwnedDesktopProcesses = @(
            $DesktopProcesses |
                Where-Object { Test-OwnedProcess -Id ([int]$_.ProcessId) }
        )
        $ForeignDesktopProcesses = @(
            $DesktopProcesses |
                Where-Object { -not (Test-OwnedProcess -Id ([int]$_.ProcessId)) }
        )
        if ($ForeignDesktopProcesses.Count -gt 0) {
            throw "Apareció una instancia debug ajena de Columnia durante el smoke; no se terminará ese proceso."
        }
        if ($OwnedDesktopProcesses.Count -gt 0) {
            $DesktopReady = $true
            Mark-Milestone -Name "desktopProcessReady"
            foreach ($DesktopProcess in $OwnedDesktopProcesses) {
                [void]$OwnedDesktopProcessIds.Add([int]$DesktopProcess.ProcessId)
                [void]$TrackedProcessIds.Add([int]$DesktopProcess.ProcessId)
            }

            $VisibleDesktopProcesses = @(
                $OwnedDesktopProcesses |
                    Where-Object { Test-VisibleDesktopProcess -Id ([int]$_.ProcessId) }
            )
            if ($VisibleDesktopProcesses.Count -gt 0) {
                Mark-Milestone -Name "windowVisible"
            }
            if ($ProjectsPanelRuntimeStatus -eq "not_checked" -and $VisibleDesktopProcesses.Count -gt 0) {
                Get-ProjectsPanelRuntimeEvidence -ProcessIds @($OwnedDesktopProcessIds)
            }
        }

        if ($ViteReady -and $DesktopReady -and $Milestones["windowVisible"]["reached"]) {
            $SmokeStatus = "passed"
            break
        }

        $RootProcess.Refresh()
        if ($RootProcess.HasExited) {
            throw "El comando npm run tauri dev terminó antes de levantar Vite y Columnia debug."
        }
        Start-Sleep -Milliseconds 500
    }

    if ($SmokeStatus -ne "passed") {
        throw "El smoke excedió el timeout configurable de $TimeoutSeconds segundos."
    }
}
catch {
    $FailureMessage = $_.Exception.Message
}
finally {
    $CleanupConfirmed = Stop-CreatedProcesses
    $AppDataRestored = $false
    if ($CleanupConfirmed -and $null -ne $AppDataGuard) {
        $AppDataRestored = Restore-ColumniaAppData -Guard $AppDataGuard
    }
    $Timer.Stop()
    if (-not $CleanupConfirmed) {
        $SmokeStatus = "failed"
        $CleanupError = "Cleanup incompleto: quedó activo un PID creado, el listener 1420 o Columnia debug."
        $FailureMessage = if ($FailureMessage) { "$FailureMessage $CleanupError" } else { $CleanupError }
    }

    [ordered]@{
        schemaVersion = 1
        status = $SmokeStatus
        startedAt = $RunStartedAt.ToString("o")
        durationMs = $Timer.ElapsedMilliseconds
        timeoutSeconds = $TimeoutSeconds
        viteListenerReady = $ViteReady
        desktopProcessReady = $DesktopReady
        milestones = $Milestones
        projectsPanel = [ordered]@{
            contractPreflight = $ProjectsPanelContractStatus
            runtimeWindow = $ProjectsPanelRuntimeStatus
            windowName = $ProjectsPanelWindowName
            windowDescendantCount = $ProjectsPanelWindowDescendantCount
            interaction = "not_available"
            note = $ProjectsPanelWindowNote
        }
        cleanupConfirmed = $CleanupConfirmed
        appDataRestored = $AppDataRestored
        cleanup = [ordered]@{
            confirmed = $CleanupConfirmed
            attempts = $CleanupAttempts
            retries = $CleanupRetryCount
            elapsedMs = $CleanupElapsedMs
            maxAttempts = 2
            attemptWindowsSeconds = @(4, 3)
        }
        command = "npm run tauri dev"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | Add-EvidenceHeader -Root $ProjectRoot -StartedAt $EvidenceStartedAt | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($SmokeStatus -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Smoke desktop aprobado; Vite y Columnia debug iniciaron con la ventana visible, los datos reales se restauraron, el preflight de ProjectsPanel pasó y el cleanup fue confirmado."
Write-Host "ProjectsPanel UI: no se simularon clics porque WebView2 no expone el DOM de React de forma estable mediante UI Automation."
Write-Host "Evidencia: $EvidenceRelativePath"
