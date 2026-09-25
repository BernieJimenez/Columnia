param(
    # Base64 of the UTF-8 text: Windows PowerShell 5.1 does not keep non-ASCII
    # characters such as "conexión" intact on its command line.
    [Parameter(Mandatory = $true)]
    [string]$TitleBase64,
    [Parameter(Mandatory = $true)]
    [string]$ButtonBase64,
    [ValidateRange(5, 120)]
    [int]$TimeoutSeconds = 30
)

# Presses one button of a native Columnia message dialog (for example the
# confirmation of a remote connection) through UI Automation, the same way a
# person would. Prints one JSON line with the result.

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
Add-Type -AssemblyName UIAutomationTypes -ErrorAction Stop
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class ColumniaMessageDialogMethods {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr FindWindow(string className, string windowName);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
}
"@

$Title = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($TitleBase64))
$Button = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($ButtonBase64))
$Deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$Stage = "find_window"
$Diagnostics = [System.Collections.Generic.List[string]]::new()

function Get-ColumniaProcessId {
    $window = [ColumniaMessageDialogMethods]::FindWindow("Tauri Window", "Columnia")
    if ($window -eq [IntPtr]::Zero) {
        return 0
    }
    [uint32]$processId = 0
    [void][ColumniaMessageDialogMethods]::GetWindowThreadProcessId($window, [ref]$processId)
    return [int]$processId
}

function Get-MessageDialog {
    $processId = Get-ColumniaProcessId
    if ($processId -eq 0) {
        return $null
    }
    try {
        $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        for ($index = 0; $index -lt $windows.Count; $index++) {
            $window = $windows.Item($index)
            $current = $window.Current
            if ($current.ProcessId -eq $processId -and $current.ClassName -eq "#32770" -and $current.Name -eq $Title) {
                return $window
            }
        }
    }
    catch {
        [void]$Diagnostics.Add("uia_tree_exception")
    }
    return $null
}

# Class and whether the title matched, never the title text itself.
function Add-CandidateDiagnostics {
    $processId = Get-ColumniaProcessId
    [void]$Diagnostics.Add("columnia_process_$([int]($processId -ne 0))")
    try {
        $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        for ($index = 0; $index -lt $windows.Count; $index++) {
            $current = $windows.Item($index).Current
            if ($current.ProcessId -eq $processId -or $current.Name -eq $Title) {
                [void]$Diagnostics.Add("window_$($current.ClassName)_own_$([int]($current.ProcessId -eq $processId))_title_match_$([int]($current.Name -eq $Title))")
            }
        }
    }
    catch {
        [void]$Diagnostics.Add("uia_candidates_exception")
    }
    $win32 = [ColumniaMessageDialogMethods]::FindWindow($null, $Title)
    [void]$Diagnostics.Add("win32_title_found_$([int]($win32 -ne [IntPtr]::Zero))")
}

function Invoke-DialogButton {
    param([System.Windows.Automation.AutomationElement]$Dialog)

    $condition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )
    $buttons = $Dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    for ($index = 0; $index -lt $buttons.Count; $index++) {
        $candidate = $buttons.Item($index)
        if ($candidate.Current.Name -ne $Button) {
            continue
        }
        $pattern = $candidate.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        return $true
    }
    return $false
}

function Write-Result {
    param([string]$Status, [string]$ErrorCode = $null)

    [ordered]@{
        status = $Status
        phase = if ($Status -eq "passed") { "native_message_dialog_answered" } else { "native_message_dialog_failed" }
        button = $Button
        errorCode = $ErrorCode
        diagnostics = @($Diagnostics)
    } | ConvertTo-Json -Compress
}

try {
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        $Stage = "find_window"
        $dialog = Get-MessageDialog
        if ($null -eq $dialog) {
            Start-Sleep -Milliseconds 150
            continue
        }
        $Stage = "invoke_button"
        if (-not (Invoke-DialogButton -Dialog $dialog)) {
            [void]$Diagnostics.Add("button_not_found")
            Start-Sleep -Milliseconds 150
            continue
        }
        $Stage = "wait_for_close"
        $closeDeadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
        while ([DateTimeOffset]::UtcNow -lt $closeDeadline) {
            if ($null -eq (Get-MessageDialog)) {
                Write-Result -Status "passed"
                exit 0
            }
            Start-Sleep -Milliseconds 150
        }
        throw "message_dialog_still_open"
    }
    Add-CandidateDiagnostics
    throw "message_dialog_timeout"
}
catch {
    $errorCode = if ($_.Exception.Message -match "^[a-z0-9_]+$") {
        $_.Exception.Message
    }
    else {
        "{0}_{1}" -f $Stage, $_.Exception.GetType().Name
    }
    Write-Result -Status "failed" -ErrorCode $errorCode
    exit 1
}
