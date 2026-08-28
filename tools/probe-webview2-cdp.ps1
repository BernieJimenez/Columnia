param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 9222,
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 120,
    [switch]$RunPlaywright,
    [switch]$RunProjects,
    [switch]$RunProjectMutations,
    [switch]$RunNativeSelectors,
    [ValidateRange(1, 5)]
    [int]$NativeSustainedRuns = 3,
    [ValidateSet("normal", "restart-prepare", "restart-verify")]
    [string]$ProjectProbeMode = "normal",
    [ValidateRange(64, 4096)]
    [int]$MemoryWorkingSetBudgetMiB = 512,
    [ValidateRange(64, 4096)]
    [int]$MemoryPrivateBudgetMiB = 256
)

$ErrorActionPreference = "Stop"

# WebView2 reads WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS when its browser
# process is created. The probe only adds a loopback CDP listener and restores
# the caller's environment before returning.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace ColumniaWebView2CdpProbe {
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
$Timestamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/webview2-cdp/$Timestamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$StdoutPath = Join-Path $EvidenceDirectory "stdout.log"
$StderrPath = Join-Path $EvidenceDirectory "stderr.log"
$VersionPath = Join-Path $EvidenceDirectory "version.json"
$TargetsPath = Join-Path $EvidenceDirectory "targets.json"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$DebugExecutable = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "src-tauri\target\debug\columnia.exe"))
$RootProcess = $null
$JobHandle = [IntPtr]::Zero
$TrackedProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$DesktopStarted = $false
$ViteStarted = $false
$CdpListenerObserved = $false
$Status = "failed"
$FailureMessage = $null
$VersionPayload = $null
$TargetsPayload = $null
$PlaywrightStatus = if ($RunPlaywright) { "pending" } else { "not_requested" }
$PlaywrightPayload = $null
$ProjectsStatus = if ($RunProjects) { "pending" } else { "not_requested" }
$ProjectsPayload = $null
$NativeSelectorsStatus = if ($RunNativeSelectors) { "pending" } else { "not_requested" }
$NativeSelectorsPayload = $null
$ProcessProfile = [ordered]@{
    sampleCount = 0
    peakProcessCount = 0
    processNames = @()
    firstWorkingSetBytes = $null
    peakWorkingSetBytes = 0L
    lastWorkingSetBytes = $null
    peakPrivateMemoryBytes = 0L
}
$PerformanceBudget = $null
$StartedAt = [DateTimeOffset]::UtcNow
$Timer = [System.Diagnostics.Stopwatch]::StartNew()
$PreviousWebViewArguments = $null
$HadWebViewArguments = Test-Path Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
if ($HadWebViewArguments) {
    $PreviousWebViewArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
}

function Get-PortListeners {
    @(Get-NetTCPConnection -State Listen -LocalAddress 127.0.0.1 -LocalPort $Port -ErrorAction SilentlyContinue)
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

function Update-ProcessProfile {
    param([int[]]$ProcessIds)

    $Processes = @($ProcessIds | Sort-Object -Unique | ForEach-Object {
        Get-Process -Id ([int]$_) -ErrorAction SilentlyContinue
    })
    if ($Processes.Count -eq 0) {
        return
    }

    $WorkingSetBytes = [int64](($Processes | Measure-Object -Property WorkingSet64 -Sum).Sum)
    $PrivateMemoryBytes = [int64](($Processes | Measure-Object -Property PrivateMemorySize64 -Sum).Sum)
    if ($script:ProcessProfile.sampleCount -eq 0) {
        $script:ProcessProfile.firstWorkingSetBytes = $WorkingSetBytes
    }
    $script:ProcessProfile.sampleCount++
    if ($Processes.Count -gt $script:ProcessProfile.peakProcessCount) {
        $script:ProcessProfile.peakProcessCount = $Processes.Count
    }
    $script:ProcessProfile.processNames = @(
        @($script:ProcessProfile.processNames) + @($Processes | ForEach-Object { $_.ProcessName }) |
            Sort-Object -Unique
    )
    $script:ProcessProfile.lastWorkingSetBytes = $WorkingSetBytes
    if ($WorkingSetBytes -gt $script:ProcessProfile.peakWorkingSetBytes) {
        $script:ProcessProfile.peakWorkingSetBytes = $WorkingSetBytes
    }
    if ($PrivateMemoryBytes -gt $script:ProcessProfile.peakPrivateMemoryBytes) {
        $script:ProcessProfile.peakPrivateMemoryBytes = $PrivateMemoryBytes
    }
}

function Get-PerformanceBudget {
    $WorkingSetBudgetBytes = [int64]$MemoryWorkingSetBudgetMiB * 1MB
    $PrivateMemoryBudgetBytes = [int64]$MemoryPrivateBudgetMiB * 1MB
    $Observed = $script:ProcessProfile.sampleCount -gt 0
    $WorkingSetWithinBudget = -not $Observed -or $script:ProcessProfile.peakWorkingSetBytes -le $WorkingSetBudgetBytes
    $PrivateMemoryWithinBudget = -not $Observed -or $script:ProcessProfile.peakPrivateMemoryBytes -le $PrivateMemoryBudgetBytes
    $BudgetStatus = if (-not $Observed) {
        "not_observed"
    }
    elseif ($WorkingSetWithinBudget -and $PrivateMemoryWithinBudget) {
        "within_budget"
    }
    else {
        "exceeded"
    }

    return [ordered]@{
        schemaVersion = 1
        status = $BudgetStatus
        enforced = $true
        sampleCount = $script:ProcessProfile.sampleCount
        workingSetBudgetBytes = $WorkingSetBudgetBytes
        privateMemoryBudgetBytes = $PrivateMemoryBudgetBytes
        peakWorkingSetBytes = $script:ProcessProfile.peakWorkingSetBytes
        peakPrivateMemoryBytes = $script:ProcessProfile.peakPrivateMemoryBytes
        workingSetWithinBudget = $WorkingSetWithinBudget
        privateMemoryWithinBudget = $PrivateMemoryWithinBudget
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
    if (-not [ColumniaWebView2CdpProbe.NativeMethods]::IsProcessInJob(
        $Candidate.Handle,
        $JobHandle,
        [ref]$BelongsToJob
    )) {
        return $false
    }
    return $BelongsToJob
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

function Get-AppProcessTreeIds {
    $Processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
    $Roots = @(Get-DebugAppProcesses | ForEach-Object { [int]$_.ProcessId })
    $Ids = [System.Collections.Generic.HashSet[int]]::new()
    $Pending = [System.Collections.Generic.Queue[int]]::new()
    foreach ($RootId in $Roots) {
        if ($Ids.Add($RootId)) {
            $Pending.Enqueue($RootId)
        }
    }
    while ($Pending.Count -gt 0) {
        $ParentId = $Pending.Dequeue()
        foreach ($Child in $Processes | Where-Object { [int]$_.ParentProcessId -eq $ParentId }) {
            $ChildId = [int]$Child.ProcessId
            if ($Ids.Add($ChildId)) {
                $Pending.Enqueue($ChildId)
            }
        }
    }
    return @($Ids | ForEach-Object { [int]$_ })
}

function Get-CdpSnapshot {
    param([int[]]$OwnedProcessIds)

    $listeners = @(Get-PortListeners)
    $ownedListeners = @($listeners | Where-Object { $OwnedProcessIds -contains [int]$_.OwningProcess })
    if ($ownedListeners.Count -eq 0) {
        return $null
    }
    $script:CdpListenerObserved = $true

    try {
        $versionResponse = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2
        $versionResponse.Content | Set-Content -LiteralPath $VersionPath -Encoding utf8
        $version = $versionResponse.Content | ConvertFrom-Json
        if ([string]::IsNullOrWhiteSpace([string]$version.webSocketDebuggerUrl)) {
            return $null
        }

        $targetsResponse = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 2
        $targetsResponse.Content | Set-Content -LiteralPath $TargetsPath -Encoding utf8
        $targets = $targetsResponse.Content | ConvertFrom-Json
        $targetCount = @($targets).Count
        return [ordered]@{
            version = $version
            targets = $targets
            targetCount = $targetCount
            listenerProcessIds = @($ownedListeners | ForEach-Object { [int]$_.OwningProcess })
        }
    }
    catch {
        return $null
    }
}

function Stop-CreatedProcesses {
    if ($null -ne $RootProcess) {
        [void]$TrackedProcessIds.Add($RootProcess.Id)
        Add-ProcessTree -RootId $RootProcess.Id
    }

    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaWebView2CdpProbe.NativeMethods]::TerminateJobObject($JobHandle, 1)
    }
    foreach ($ProcessId in @($TrackedProcessIds | Sort-Object -Descending)) {
        Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
    }

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(7)
    do {
        $remainingTracked = @($TrackedProcessIds | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        $remainingListeners = @(Get-PortListeners | Where-Object { $TrackedProcessIds.Contains([int]$_.OwningProcess) })
        if ($remainingTracked.Count -eq 0 -and $remainingListeners.Count -eq 0) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    } while ([DateTimeOffset]::UtcNow -lt $deadline)
    return $false
}

function Invoke-PlaywrightProbe {
    $RunnerPath = Join-Path $ProjectRoot "tools\probe-webview2-playwright.mjs"
    if (-not (Test-Path -LiteralPath $RunnerPath -PathType Leaf)) {
        $script:PlaywrightStatus = "failed"
        $script:PlaywrightPayload = [ordered]@{
            status = "failed"
            error = "No se encontró tools/probe-webview2-playwright.mjs."
        }
        return
    }

    $NodeCommand = (Get-Command node.exe -ErrorAction Stop).Source
    $Output = @(& $NodeCommand $RunnerPath "--port" $Port 2>&1)
    $OutputText = [string]::Join([Environment]::NewLine, @($Output | ForEach-Object { [string]$_ }))
    $LastJsonLine = @($Output | Where-Object { ([string]$_).TrimStart().StartsWith("{") } | Select-Object -Last 1)
    if ($LastJsonLine.Count -eq 0) {
        $script:PlaywrightStatus = "failed"
        $script:PlaywrightPayload = [ordered]@{
            status = "failed"
            error = if ($OutputText) { $OutputText } else { "El runner no produjo evidencia JSON." }
        }
        return
    }

    try {
        $script:PlaywrightPayload = [string]$LastJsonLine[0] | ConvertFrom-Json
        $script:PlaywrightStatus = [string]$script:PlaywrightPayload.status
    }
    catch {
        $script:PlaywrightStatus = "failed"
        $script:PlaywrightPayload = [ordered]@{
            status = "failed"
            error = "El runner produjo una respuesta JSON inválida: $OutputText"
        }
    }
}

function Invoke-ProjectsProbe {
    $RunnerPath = Join-Path $ProjectRoot "tools\probe-webview2-projects.mjs"
    if (-not (Test-Path -LiteralPath $RunnerPath -PathType Leaf)) {
        $script:ProjectsStatus = "failed"
        $script:ProjectsPayload = [ordered]@{
            status = "failed"
            error = "No se encontró tools/probe-webview2-projects.mjs."
        }
        return
    }

    $NodeCommand = (Get-Command node.exe -ErrorAction Stop).Source
    $RunnerArguments = @($RunnerPath, "--port", $Port)
    if ($RunProjectMutations) {
        $RunnerArguments += "--mutate"
        $RunnerArguments += "--sustained-runs"
        $RunnerArguments += $NativeSustainedRuns
    }
    if ($ProjectProbeMode -eq "restart-prepare") {
        $RunnerArguments += "--restart-prepare"
    }
    elseif ($ProjectProbeMode -eq "restart-verify") {
        $RunnerArguments += "--restart-verify"
    }
    $Output = @(& $NodeCommand @RunnerArguments 2>&1)
    $OutputText = [string]::Join([Environment]::NewLine, @($Output | ForEach-Object { [string]$_ }))
    $LastJsonLine = @($Output | Where-Object { ([string]$_).TrimStart().StartsWith("{") } | Select-Object -Last 1)
    if ($LastJsonLine.Count -eq 0) {
        $script:ProjectsStatus = "failed"
        $script:ProjectsPayload = [ordered]@{
            status = "failed"
            error = if ($OutputText) { $OutputText } else { "El runner ProjectsPanel no produjo evidencia JSON." }
        }
        return
    }

    try {
        $script:ProjectsPayload = [string]$LastJsonLine[0] | ConvertFrom-Json
        $script:ProjectsStatus = [string]$script:ProjectsPayload.status
    }
    catch {
        $script:ProjectsStatus = "failed"
        $script:ProjectsPayload = [ordered]@{
            status = "failed"
            error = "El runner ProjectsPanel produjo una respuesta JSON inválida: $OutputText"
        }
    }
}

function Invoke-NativeSelectorsProbe {
    $RunnerPath = Join-Path $ProjectRoot "tools\probe-webview2-native-selectors.mjs"
    $DriverPath = Join-Path $ProjectRoot "tools\automate-native-file-dialog.ps1"
    if (-not (Test-Path -LiteralPath $RunnerPath -PathType Leaf) -or -not (Test-Path -LiteralPath $DriverPath -PathType Leaf)) {
        $script:NativeSelectorsStatus = "failed"
        $script:NativeSelectorsPayload = [ordered]@{
            status = "failed"
            error = "No se encontró el runner o driver de selectores nativos."
        }
        return
    }

    $NodeCommand = (Get-Command node.exe -ErrorAction Stop).Source
    $RequestPath = Join-Path $EvidenceDirectory "native-selector-request.json"
    $RunnerStdoutPath = Join-Path $EvidenceDirectory "native-selectors.stdout.log"
    $RunnerStderrPath = Join-Path $EvidenceDirectory "native-selectors.stderr.log"
    Remove-Item -LiteralPath $RequestPath -Force -ErrorAction SilentlyContinue
    $RunnerProcess = Start-Process `
        -FilePath $NodeCommand `
        -ArgumentList @(
            "`"$RunnerPath`"",
            "--port",
            $Port,
            "--request-file",
            "`"$RequestPath`""
        ) `
        -WorkingDirectory $ProjectRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $RunnerStdoutPath `
        -RedirectStandardError $RunnerStderrPath `
        -PassThru
    $ProbeDeadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $HandledRequestId = $null
    try {
        while ([DateTimeOffset]::UtcNow -lt $ProbeDeadline) {
            if (Test-Path -LiteralPath $RequestPath -PathType Leaf) {
                try {
                    $Request = Get-Content -LiteralPath $RequestPath -Raw | ConvertFrom-Json
                }
                catch {
                    $Request = $null
                }
                if ($null -ne $Request -and $Request.status -eq "pending" -and $Request.requestId -ne $HandledRequestId) {
                    $HandledRequestId = [string]$Request.requestId
                    $DriverOutputPath = Join-Path $EvidenceDirectory ("native-dialog-driver-{0}.stdout.log" -f $HandledRequestId)
                    $DriverErrorPath = Join-Path $EvidenceDirectory ("native-dialog-driver-{0}.stderr.log" -f $HandledRequestId)
                    $DriverProcess = $null
                    $DriverOutput = @()
                    $DriverTimedOut = $false
                    try {
                        $DriverProcess = Start-Process `
                            -FilePath "powershell.exe" `
                            -ArgumentList @(
                                "-NoProfile",
                                "-ExecutionPolicy",
                                "Bypass",
                                "-File",
                                "`"$DriverPath`"",
                                "-Mode",
                                ([string]$Request.mode),
                                "-Path",
                                "`"$([string]$Request.targetPath)`"",
                                "-TimeoutSeconds",
                                "30"
                            ) `
                            -WorkingDirectory $ProjectRoot `
                            -WindowStyle Hidden `
                            -RedirectStandardOutput $DriverOutputPath `
                            -RedirectStandardError $DriverErrorPath `
                            -PassThru
                        if (-not $DriverProcess.WaitForExit(35 * 1000)) {
                            $DriverTimedOut = $true
                            Stop-Process -Id $DriverProcess.Id -Force -ErrorAction SilentlyContinue
                        }
                        if (-not $DriverTimedOut -and (Test-Path -LiteralPath $DriverOutputPath -PathType Leaf)) {
                            $DriverOutput = @(Get-Content -LiteralPath $DriverOutputPath)
                        }
                    }
                    catch {
                        $DriverTimedOut = $false
                    }
                    finally {
                        if ($null -ne $DriverProcess) {
                            $DriverProcess.Refresh()
                            if (-not $DriverProcess.HasExited) {
                                Stop-Process -Id $DriverProcess.Id -Force -ErrorAction SilentlyContinue
                            }
                        }
                    }
                    $DriverJsonLine = @(
                        $DriverOutput |
                            Where-Object { ([string]$_).TrimStart().StartsWith("{") } |
                            Select-Object -Last 1
                    )
                    $DriverStatus = "failed"
                    $DriverErrorCode = if ($DriverTimedOut) { "native_dialog_driver_timeout" } else { "native_dialog_driver_failed" }
                    $DriverDiagnostics = @()
                    if ($DriverJsonLine.Count -gt 0) {
                        try {
                            $DriverResult = [string]$DriverJsonLine[0] | ConvertFrom-Json
                            $DriverStatus = [string]$DriverResult.status
                            if ($null -ne $DriverResult.diagnostics) {
                                $DriverDiagnostics = @($DriverResult.diagnostics | ForEach-Object { [string]$_ })
                            }
                            if ($DriverStatus -eq "passed") {
                                $DriverErrorCode = $null
                            }
                            elseif ($null -ne $DriverResult.errorCode) {
                                $DriverErrorCode = [string]$DriverResult.errorCode
                            }
                        }
                        catch {
                            $DriverErrorCode = "native_dialog_driver_invalid_result"
                        }
                    }
                    $Response = [ordered]@{
                        requestId = $HandledRequestId
                        status = if ($DriverStatus -eq "passed") { "passed" } else { "failed" }
                        errorCode = $DriverErrorCode
                        diagnostics = $DriverDiagnostics
                    } | ConvertTo-Json -Compress
                    [System.IO.File]::WriteAllText(
                        $RequestPath,
                        $Response,
                        [System.Text.UTF8Encoding]::new($false)
                    )
                }
            }
            $RunnerProcess.Refresh()
            if ($RunnerProcess.HasExited) {
                break
            }
            Start-Sleep -Milliseconds 150
        }

        $RunnerProcess.Refresh()
        if (-not $RunnerProcess.HasExited) {
            Stop-Process -Id $RunnerProcess.Id -Force -ErrorAction SilentlyContinue
            $script:NativeSelectorsStatus = "failed"
            $script:NativeSelectorsPayload = [ordered]@{
                status = "failed"
                phase = "native_file_selectors_failed"
                errorCode = "native_selectors_timeout"
                interactions = @()
            }
            return
        }

        $Output = @()
        if (Test-Path -LiteralPath $RunnerStdoutPath -PathType Leaf) {
            $Output = @(Get-Content -LiteralPath $RunnerStdoutPath)
        }
        $LastJsonLine = @($Output | Where-Object { ([string]$_).TrimStart().StartsWith("{") } | Select-Object -Last 1)
        if ($LastJsonLine.Count -eq 0) {
            $script:NativeSelectorsStatus = "failed"
            $script:NativeSelectorsPayload = [ordered]@{
                status = "failed"
                phase = "native_file_selectors_failed"
                errorCode = "native_selectors_runner_no_result"
                interactions = @()
            }
            return
        }
        $script:NativeSelectorsPayload = [string]$LastJsonLine[0] | ConvertFrom-Json
        $script:NativeSelectorsStatus = [string]$script:NativeSelectorsPayload.status
    }
    catch {
        $script:NativeSelectorsStatus = "failed"
        $script:NativeSelectorsPayload = [ordered]@{
            status = "failed"
            phase = "native_file_selectors_failed"
            errorCode = "native_selectors_runner_failed"
            interactions = @()
        }
    }
    finally {
        if ($null -ne $RunnerProcess) {
            $RunnerProcess.Refresh()
            if (-not $RunnerProcess.HasExited) {
                Stop-Process -Id $RunnerProcess.Id -Force -ErrorAction SilentlyContinue
            }
        }
        Remove-Item -LiteralPath $RequestPath -Force -ErrorAction SilentlyContinue
    }
}

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null

try {
    if ($env:OS -ne "Windows_NT") {
        throw "El probe CDP solo está implementado para Windows/WebView2 en esta versión."
    }
    if (@(Get-PortListeners).Count -gt 0) {
        throw "Preflight falló: el puerto CDP $Port ya tiene un listener; no se inspeccionará un proceso ajeno."
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

    $JobHandle = [ColumniaWebView2CdpProbe.NativeMethods]::CreateJobObject([IntPtr]::Zero, $null)
    if ($JobHandle -eq [IntPtr]::Zero) {
        throw "No se pudo crear el Job Object aislado para el probe."
    }

    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $RootProcess = Start-Process `
        -FilePath $NodeCommand `
        -ArgumentList @("`"$NpmCli`"", "run", "tauri", "dev") `
        -WorkingDirectory $ProjectRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $StdoutPath `
        -RedirectStandardError $StderrPath `
        -PassThru
    if (-not [ColumniaWebView2CdpProbe.NativeMethods]::AssignProcessToJobObject(
        $JobHandle,
        $RootProcess.Handle
    )) {
        throw "No se pudo aislar el proceso raíz del probe en su Job Object."
    }
    [void]$TrackedProcessIds.Add($RootProcess.Id)

    $Deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        Add-ProcessTree -RootId $RootProcess.Id
        $Listeners = @(Get-PortListeners)
        $ForeignListeners = @($Listeners | Where-Object { -not (Test-OwnedProcess -Id ([int]$_.OwningProcess)) })
        if ($ForeignListeners.Count -gt 0) {
            throw "Apareció un listener ajeno en el puerto CDP $Port; no se leerá su endpoint."
        }
        $OwnedListeners = @($Listeners | Where-Object { Test-OwnedProcess -Id ([int]$_.OwningProcess) })
        if ($OwnedListeners.Count -gt 0) {
            foreach ($Listener in $OwnedListeners) {
                [void]$TrackedProcessIds.Add([int]$Listener.OwningProcess)
            }
            $snapshot = Get-CdpSnapshot -OwnedProcessIds @($OwnedListeners | ForEach-Object { [int]$_.OwningProcess })
            if ($null -ne $snapshot) {
                Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
                $VersionPayload = $snapshot.version
                $TargetsPayload = $snapshot.targets
                $DesktopStarted = $true
                if (-not $RunPlaywright -and -not $RunProjects -and -not $RunNativeSelectors) {
                    $Status = "supported"
                    break
                }

                if ($RunPlaywright -and ($PlaywrightStatus -eq "pending" -or $PlaywrightStatus -eq "not_ready")) {
                    Invoke-PlaywrightProbe
                    Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
                }
                if ($RunPlaywright -and $PlaywrightStatus -eq "passed") {
                    if (-not $RunProjects -and -not $RunNativeSelectors) {
                        $Status = "supported"
                        break
                    }

                    if ($ProjectsStatus -eq "pending" -or $ProjectsStatus -eq "not_ready") {
                        Invoke-ProjectsProbe
                        Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
                    }
                    if ($ProjectsStatus -eq "passed" -and -not $RunNativeSelectors) {
                        $Status = "supported"
                        break
                    }
                    if ($ProjectsStatus -eq "failed") {
                        $Status = "failed"
                        $FailureMessage = if ($RunProjectMutations) {
                            "El runner ProjectsPanel no pudo verificar las mutaciones IPC nativas."
                        }
                        else {
                            "El runner ProjectsPanel no pudo verificar el contrato nativo de solo lectura."
                        }
                        break
                    }
                }

                if (-not $RunPlaywright -and $RunProjects) {
                    if ($ProjectsStatus -eq "pending" -or $ProjectsStatus -eq "not_ready") {
                        Invoke-ProjectsProbe
                        Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
                    }
                    if ($ProjectsStatus -eq "passed" -and -not $RunNativeSelectors) {
                        $Status = "supported"
                        break
                    }
                    if ($ProjectsStatus -eq "failed") {
                        $Status = "failed"
                        $FailureMessage = if ($RunProjectMutations) {
                            "El runner ProjectsPanel no pudo verificar las mutaciones IPC nativas."
                        }
                        else {
                            "El runner ProjectsPanel no pudo verificar el contrato nativo de solo lectura."
                        }
                        break
                    }
                }

                if ($RunNativeSelectors -and (-not $RunPlaywright -or $PlaywrightStatus -eq "passed") -and ($NativeSelectorsStatus -eq "pending" -or $NativeSelectorsStatus -eq "not_ready")) {
                    Invoke-NativeSelectorsProbe
                    Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
                }
                if ($RunNativeSelectors -and $NativeSelectorsStatus -eq "passed") {
                    $Status = "supported"
                    break
                }
                if ($RunNativeSelectors -and $NativeSelectorsStatus -eq "failed") {
                    $Status = "failed"
                    $FailureMessage = "El runner de selectores nativos no pudo verificar los diálogos de archivo de Windows."
                    break
                }

                # WebView2 can expose a provisional about:blank target before
                # the Tauri page has committed. Keep the CDP listener alive
                # and let Playwright retry until the shell DOM is ready.
                if ($RunPlaywright -and $PlaywrightStatus -eq "failed") {
                    $Status = "failed"
                    $FailureMessage = "El runner Playwright no pudo inspeccionar el shell nativo."
                    break
                }
            }
        }

        if (@(Get-PortListeners | Where-Object { $_.LocalPort -eq 1420 -and (Test-OwnedProcess -Id ([int]$_.OwningProcess)) }).Count -gt 0) {
            $ViteStarted = $true
        }
        if (@(Get-DebugAppProcesses | Where-Object { Test-OwnedProcess -Id ([int]$_.ProcessId) }).Count -gt 0) {
            $DesktopStarted = $true
        }

        $RootProcess.Refresh()
        if ($RootProcess.HasExited) {
            throw "npm run tauri dev terminó antes de abrir el endpoint CDP."
        }
        Start-Sleep -Milliseconds 500
    }

    if ($Status -eq "failed" -and $DesktopStarted -and $null -eq $FailureMessage) {
        $Status = "not_supported"
        $FailureMessage = "Tauri/WebView2 inició, pero no expuso /json/version en el puerto $Port durante $TimeoutSeconds segundos."
    }
    elseif ($Status -eq "failed" -and $null -eq $FailureMessage) {
        $FailureMessage = "No se pudo confirmar que la aplicación debug iniciara antes del timeout."
    }
}
catch {
    $FailureMessage = $_.Exception.Message
}
finally {
    Update-ProcessProfile -ProcessIds (Get-AppProcessTreeIds)
    $PerformanceBudget = Get-PerformanceBudget
    if ($Status -eq "supported" -and $PerformanceBudget.status -eq "exceeded") {
        $Status = "failed"
        $FailureMessage = "El perfil de memoria excedió el presupuesto configurado: working set <= $MemoryWorkingSetBudgetMiB MiB y memoria privada <= $MemoryPrivateBudgetMiB MiB."
    }
    $CleanupConfirmed = Stop-CreatedProcesses
    $Timer.Stop()
    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaWebView2CdpProbe.NativeMethods]::CloseHandle($JobHandle)
        $script:JobHandle = [IntPtr]::Zero
    }
    if ($HadWebViewArguments) {
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $PreviousWebViewArguments
    }
    else {
        Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    }

    [ordered]@{
        schemaVersion = 1
        status = $Status
        startedAt = $StartedAt.ToString("o")
        durationMs = $Timer.ElapsedMilliseconds
        timeoutSeconds = $TimeoutSeconds
        cdpPort = $Port
        playwrightRequested = [bool]$RunPlaywright
        playwrightStatus = $PlaywrightStatus
        playwright = $PlaywrightPayload
        projectsRequested = [bool]$RunProjects
        projectsMutationRequested = [bool]$RunProjectMutations
        nativeSustainedRunsRequested = if ($RunProjectMutations) { $NativeSustainedRuns } else { 0 }
        projectProbeMode = $ProjectProbeMode
        projectsStatus = $ProjectsStatus
        projects = $ProjectsPayload
        nativeSelectorsRequested = [bool]$RunNativeSelectors
        nativeSelectorsStatus = $NativeSelectorsStatus
        nativeSelectors = $NativeSelectorsPayload
        cdpListenerObserved = $CdpListenerObserved
        viteListenerReady = $ViteStarted
        desktopProcessReady = $DesktopStarted
        endpoint = if ($null -ne $VersionPayload) { [string]$VersionPayload.webSocketDebuggerUrl } else { $null }
        browser = if ($null -ne $VersionPayload) { [string]$VersionPayload.Browser } else { $null }
        targetCount = if ($null -ne $TargetsPayload) { @($TargetsPayload).Count } else { 0 }
        processProfile = $ProcessProfile
        performanceBudget = $PerformanceBudget
        cleanupConfirmed = $CleanupConfirmed
        command = "npm run tauri dev"
        environmentVariable = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -eq "supported" -and (-not $RunPlaywright -or $PlaywrightStatus -eq "passed") -and (-not $RunProjects -or $ProjectsStatus -eq "passed") -and (-not $RunNativeSelectors -or $NativeSelectorsStatus -eq "passed")) {
    if ($RunPlaywright) {
        if ($RunProjects -and $RunNativeSelectors) {
            Write-Host "WebView2 CDP, ProjectsPanel y selectores nativos aprobados en http://127.0.0.1:$Port; se verificaron IPC de proyectos y los diálogos de abrir/guardar sin exponer rutas."
        }
        elseif ($RunProjects) {
            if ($RunProjectMutations) {
                Write-Host "WebView2 CDP, Playwright connectOverCDP y ProjectsPanel aprobados en http://127.0.0.1:$Port; se verificaron primer render, landmarks, foco y mutaciones IPC nativas con cleanup del proyecto de prueba."
            }
            else {
                Write-Host "WebView2 CDP, Playwright connectOverCDP y ProjectsPanel aprobados en http://127.0.0.1:$Port; se verificaron primer render, landmarks, foco y contrato de solo lectura, sin mutar datos."
            }
        }
        elseif ($RunNativeSelectors) {
            Write-Host "WebView2 CDP y selectores nativos aprobados en http://127.0.0.1:$Port; se verificaron abrir dataset, guardar/cargar receta y exportar con diálogos de Windows."
        }
        else {
            Write-Host "WebView2 CDP y Playwright connectOverCDP aprobados en http://127.0.0.1:$Port; se verificaron primer render, landmarks y foco, sin mutar datos."
        }
    }
    else {
        if ($RunProjects -and $RunNativeSelectors) {
            Write-Host "WebView2 CDP, ProjectsPanel y selectores nativos aprobados en http://127.0.0.1:$Port; se verificaron proyectos y diálogos de abrir/guardar sin exponer rutas."
        }
        elseif ($RunProjects) {
            Write-Host "WebView2 CDP y ProjectsPanel aprobados en http://127.0.0.1:$Port; se verificó el contrato nativo solicitado."
        }
        elseif ($RunNativeSelectors) {
            Write-Host "WebView2 CDP y selectores nativos aprobados en http://127.0.0.1:$Port; se verificaron abrir dataset, guardar/cargar receta y exportar con diálogos de Windows."
        }
        else {
            Write-Host "WebView2 CDP detectado en http://127.0.0.1:$Port; no se ejecutaron comandos CDP ni interacciones DOM."
        }
    }
    Write-Host "Evidencia: $EvidenceRelativePath"
    exit 0
}

Write-Error "Probe WebView2/CDP: $Status (Playwright: $PlaywrightStatus). $FailureMessage Evidencia: $EvidenceRelativePath"
exit 1
