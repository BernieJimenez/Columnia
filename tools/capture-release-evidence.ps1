[CmdletBinding()]
param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 9230,
    [ValidateRange(15, 900)]
    [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$EvidenceStamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$EvidenceRelativePath = ".local/validation/release-evidence/$EvidenceStamp"
$EvidenceDirectory = Join-Path $ProjectRoot ($EvidenceRelativePath -replace "/", "\")
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"
$StdoutPath = Join-Path $EvidenceDirectory "stdout.log"
$StderrPath = Join-Path $EvidenceDirectory "stderr.log"
$ReleaseExecutable = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "src-tauri\target\release\columnia.exe"))
$FixturePath = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "fixtures\automation\input.csv"))
$RootProcess = $null
$JobHandle = [IntPtr]::Zero
$TrackedProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$PreviousWebViewArguments = $null
$HadWebViewArguments = Test-Path Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$CleanupConfirmed = $false

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace ColumniaReleaseEvidence {
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

function Get-ProcessSnapshot {
    @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
}

function Add-ProcessTree {
    param([int]$RootId)

    $processes = Get-ProcessSnapshot
    $pending = [System.Collections.Generic.Queue[int]]::new()
    if ($TrackedProcessIds.Add($RootId)) { $pending.Enqueue($RootId) }
    while ($pending.Count -gt 0) {
        $parentId = $pending.Dequeue()
        foreach ($child in $processes | Where-Object { [int]$_.ParentProcessId -eq $parentId }) {
            $childId = [int]$child.ProcessId
            if ($TrackedProcessIds.Add($childId)) { $pending.Enqueue($childId) }
        }
    }
}

function Test-OwnedProcess {
    param([int]$Id)

    if ($JobHandle -eq [IntPtr]::Zero) { return $false }
    $candidate = Get-Process -Id $Id -ErrorAction SilentlyContinue
    if ($null -eq $candidate) { return $false }
    $belongsToJob = $false
    if (-not [ColumniaReleaseEvidence.NativeMethods]::IsProcessInJob($candidate.Handle, $JobHandle, [ref]$belongsToJob)) {
        return $false
    }
    return $belongsToJob
}

function Stop-CreatedProcesses {
    foreach ($id in @($TrackedProcessIds)) {
        if (Test-OwnedProcess -Id $id) {
            Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
        }
    }
    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaReleaseEvidence.NativeMethods]::TerminateJobObject($JobHandle, 1)
    }
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(5)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $remaining = @($TrackedProcessIds | Where-Object { Test-OwnedProcess -Id ([int]$_) })
        if ($remaining.Count -eq 0) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return @($TrackedProcessIds | Where-Object { Test-OwnedProcess -Id ([int]$_) }).Count -eq 0
}

function Get-ListenerOwner {
    @(Get-NetTCPConnection -State Listen -LocalAddress 127.0.0.1 -LocalPort $Port -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess)
}

function Get-Sha256 {
    param([string]$Path)

    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $stream.Dispose()
        $algorithm.Dispose()
    }
}

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null

try {
    if ($env:OS -ne "Windows_NT") { throw "La evidencia de release requiere Windows/WebView2." }
    if (@(git -C $ProjectRoot status --porcelain).Count -gt 0) { throw "La evidencia de release exige un árbol Git limpio; registra primero el commit de release." }
    if (-not (Test-Path -LiteralPath $FixturePath -PathType Leaf)) { throw "No existe la fixture sintética estable." }
    if (@(Get-ListenerOwner).Count -gt 0) { throw "El puerto CDP $Port ya está ocupado." }

    Push-Location $ProjectRoot
    try {
        & npm run tauri build -- --no-bundle
        if ($LASTEXITCODE -ne 0) { throw "El build Tauri release falló." }
    }
    finally { Pop-Location }

    if (-not (Test-Path -LiteralPath $ReleaseExecutable -PathType Leaf)) {
        throw "No se encontró el binario release: $ReleaseExecutable"
    }

    $package = Get-Content -LiteralPath (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json
    $binary = Get-Item -LiteralPath $ReleaseExecutable
    $fixture = Get-Item -LiteralPath $FixturePath
    $binarySha256 = Get-Sha256 $ReleaseExecutable
    $fixtureSha256 = Get-Sha256 $FixturePath
    $nodeCommand = (Get-Command node.exe -ErrorAction Stop).Source
    $runnerPath = Join-Path $ProjectRoot "tools\capture-release-evidence.mjs"
    $jobHandle = [ColumniaReleaseEvidence.NativeMethods]::CreateJobObject([IntPtr]::Zero, $null)
    if ($jobHandle -eq [IntPtr]::Zero) { throw "No se pudo crear el Job Object de evidencia release." }
    $JobHandle = $jobHandle

    if ($HadWebViewArguments) { $PreviousWebViewArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS }
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $RootProcess = Start-Process -FilePath $ReleaseExecutable -WorkingDirectory $ProjectRoot -WindowStyle Hidden -RedirectStandardOutput $StdoutPath -RedirectStandardError $StderrPath -PassThru
    if (-not [ColumniaReleaseEvidence.NativeMethods]::AssignProcessToJobObject($JobHandle, $RootProcess.Handle)) {
        throw "No se pudo aislar el binario release en su Job Object."
    }
    [void]$TrackedProcessIds.Add($RootProcess.Id)

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $endpointReady = $false
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        Add-ProcessTree -RootId $RootProcess.Id
        $listeners = @(Get-ListenerOwner)
        $foreign = @($listeners | Where-Object { -not (Test-OwnedProcess -Id ([int]$_)) })
        if ($foreign.Count -gt 0) { throw "Apareció un listener ajeno en el puerto CDP $Port." }
        if ($listeners.Count -gt 0) {
            try {
                $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2
                $version = $response.Content | ConvertFrom-Json
                if (-not [string]::IsNullOrWhiteSpace([string]$version.webSocketDebuggerUrl)) {
                    $endpointReady = $true
                    break
                }
            }
            catch { }
        }
        $RootProcess.Refresh()
        if ($RootProcess.HasExited) { throw "El binario release terminó antes de exponer CDP." }
        Start-Sleep -Milliseconds 500
    }
    if (-not $endpointReady) { throw "El binario release no expuso CDP en $TimeoutSeconds segundos." }

    & $nodeCommand $runnerPath --port $Port --output $SummaryPath --project-version $package.version --binary-path "src-tauri/target/release/columnia.exe" --binary-sha256 $binarySha256 --binary-size $binary.Length --fixture-path "fixtures/automation/input.csv" --fixture-sha256 $fixtureSha256 --fixture-size $fixture.Length
    if ($LASTEXITCODE -ne 0) { throw "La captura Playwright del binario release falló." }
}
catch {
    $message = $_.Exception.Message
    Write-Error $message
    if (-not (Test-Path -LiteralPath $SummaryPath -PathType Leaf)) {
        [ordered]@{ schemaVersion = 1; captureVersion = 1; status = "failed"; source = "tauri-release-binary"; generatedAt = [DateTimeOffset]::UtcNow.ToString("o"); evidenceDirectory = $EvidenceRelativePath; error = $message } |
            ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $SummaryPath -Encoding utf8
    }
    exit 1
}
finally {
    $CleanupConfirmed = Stop-CreatedProcesses
    if ($JobHandle -ne [IntPtr]::Zero) {
        [void][ColumniaReleaseEvidence.NativeMethods]::CloseHandle($JobHandle)
        $JobHandle = [IntPtr]::Zero
    }
    if ($HadWebViewArguments) { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $PreviousWebViewArguments }
    else { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue }
}

if (-not $CleanupConfirmed) { throw "La evidencia release no confirmó el cleanup de sus procesos." }
Write-Host "Evidencia release completada: $EvidenceRelativePath"
