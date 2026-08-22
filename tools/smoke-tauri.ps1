param(
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 180
)

$ErrorActionPreference = "Stop"
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
    }
}
"@

$ProjectRoot = Split-Path -Parent $PSScriptRoot
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
$SmokeStatus = "failed"
$FailureMessage = $null
$Timer = [System.Diagnostics.Stopwatch]::StartNew()

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

function Test-ProjectsPanelContract {
    $checks = [ordered]@{
        "ProjectsPanel.tsx" = @(
            'export function ProjectsPanel',
            'id="projects-title"',
            'id="project-name"',
            'onSave',
            'onDeleteRequest',
            'onDeleteConfirm'
        )
        "App.tsx" = @(
            '<ProjectsPanel',
            'onSave={(name)',
            'onOpen={(projectId)',
            'onDeleteConfirm={() => void projects.confirmDelete()}'
        )
        "useProjectsController.ts" = @(
            'listProjects()',
            'saveProject(',
            'openProject(projectId)',
            'deleteProject(target.id)',
            'setCatalog({ kind: "ready"'
        )
    }

    $missing = [System.Collections.Generic.List[string]]::new()
    foreach ($entry in $checks.GetEnumerator()) {
        $sourcePath = Join-Path $ProjectRoot (Join-Path "src\features\projects" $entry.Key)
        if ($entry.Key -eq "App.tsx") {
            $sourcePath = Join-Path $ProjectRoot "src\App.tsx"
        }
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            [void]$missing.Add($entry.Key)
            continue
        }
        $source = Get-Content -LiteralPath $sourcePath -Raw
        foreach ($needle in $entry.Value) {
            if (-not $source.Contains($needle)) {
                [void]$missing.Add("$($entry.Key):$needle")
            }
        }
    }

    if ($missing.Count -gt 0) {
        throw "Preflight de ProjectsPanel falló: faltan contratos esperados ($($missing -join ', '))."
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
    if ($null -ne $RootProcess) {
        [void]$TrackedProcessIds.Add($RootProcess.Id)
        Add-ProcessTree -RootId $RootProcess.Id
    }

    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaDesktopSmoke.NativeMethods]::TerminateJobObject($JobHandle, 1)
    }
    $OrderedIds = @($TrackedProcessIds) | Sort-Object -Descending
    foreach ($ProcessId in $OrderedIds) {
        Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
    }

    $CleanupDeadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
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

    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaDesktopSmoke.NativeMethods]::CloseHandle($JobHandle)
        $script:JobHandle = [IntPtr]::Zero
    }
    return $false
}

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
            foreach ($DesktopProcess in $OwnedDesktopProcesses) {
                [void]$OwnedDesktopProcessIds.Add([int]$DesktopProcess.ProcessId)
                [void]$TrackedProcessIds.Add([int]$DesktopProcess.ProcessId)
            }
            if ($ProjectsPanelRuntimeStatus -eq "not_checked") {
                Get-ProjectsPanelRuntimeEvidence -ProcessIds @($OwnedDesktopProcessIds)
            }
        }

        if ($ViteReady -and $DesktopReady) {
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
        projectsPanel = [ordered]@{
            contractPreflight = $ProjectsPanelContractStatus
            runtimeWindow = $ProjectsPanelRuntimeStatus
            windowName = $ProjectsPanelWindowName
            windowDescendantCount = $ProjectsPanelWindowDescendantCount
            interaction = "not_available"
            note = $ProjectsPanelWindowNote
        }
        cleanupConfirmed = $CleanupConfirmed
        command = "npm run tauri dev"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($SmokeStatus -ne "passed") {
    Write-Error "$FailureMessage Evidencia: $EvidenceRelativePath"
    exit 1
}

Write-Host "Smoke desktop aprobado; Vite y Columnia debug iniciaron, el preflight de ProjectsPanel pasó y el cleanup fue confirmado."
Write-Host "ProjectsPanel UI: no se simularon clics porque WebView2 no expone el DOM de React de forma estable mediante UI Automation."
Write-Host "Evidencia: $EvidenceRelativePath"
