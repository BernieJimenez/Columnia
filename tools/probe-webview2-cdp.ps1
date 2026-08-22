param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 9222,
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 120,
    [switch]$RunPlaywright
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
                $VersionPayload = $snapshot.version
                $TargetsPayload = $snapshot.targets
                $DesktopStarted = $true
                if (-not $RunPlaywright) {
                    $Status = "supported"
                    break
                }

                Invoke-PlaywrightProbe
                if ($PlaywrightStatus -eq "passed") {
                    $Status = "supported"
                    break
                }

                # WebView2 can expose a provisional about:blank target before
                # the Tauri page has committed. Keep the CDP listener alive
                # and let Playwright retry until the shell DOM is ready.
                if ($PlaywrightStatus -ne "not_ready") {
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

    if ($Status -eq "failed" -and $DesktopStarted) {
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
        cdpListenerObserved = $CdpListenerObserved
        viteListenerReady = $ViteStarted
        desktopProcessReady = $DesktopStarted
        endpoint = if ($null -ne $VersionPayload) { [string]$VersionPayload.webSocketDebuggerUrl } else { $null }
        browser = if ($null -ne $VersionPayload) { [string]$VersionPayload.Browser } else { $null }
        targetCount = if ($null -ne $TargetsPayload) { @($TargetsPayload).Count } else { 0 }
        cleanupConfirmed = $CleanupConfirmed
        command = "npm run tauri dev"
        environmentVariable = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"
        evidenceDirectory = $EvidenceRelativePath
        error = $FailureMessage
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
}

if ($Status -eq "supported" -and $PlaywrightStatus -eq "passed") {
    if ($RunPlaywright) {
        Write-Host "WebView2 CDP y Playwright connectOverCDP aprobados en http://127.0.0.1:$Port; se verificaron primer render, landmarks y foco, sin mutar datos."
    }
    else {
        Write-Host "WebView2 CDP detectado en http://127.0.0.1:$Port; no se ejecutaron comandos CDP ni interacciones DOM."
    }
    Write-Host "Evidencia: $EvidenceRelativePath"
    exit 0
}

Write-Error "Probe WebView2/CDP: $Status (Playwright: $PlaywrightStatus). $FailureMessage Evidencia: $EvidenceRelativePath"
exit 1
