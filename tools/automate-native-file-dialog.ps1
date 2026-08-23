param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("open", "save")]
    [string]$Mode,
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [ValidateRange(5, 120)]
    [int]$TimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class ColumniaNativeDialogMethods {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr FindWindow(string className, string windowName);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string className, string windowName);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr GetDlgItem(IntPtr window, int controlId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern int GetClassName(IntPtr window, StringBuilder className, int capacity);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern int GetWindowText(IntPtr window, StringBuilder text, int capacity);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool SetWindowText(IntPtr window, string text);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extraInfo);

    public static void SendControlA() {
        keybd_event(0x11, 0, 0, UIntPtr.Zero);
        keybd_event(0x41, 0, 0, UIntPtr.Zero);
        keybd_event(0x41, 0, 2, UIntPtr.Zero);
        keybd_event(0x11, 0, 2, UIntPtr.Zero);
    }

    public static void SendUnicodeText(string text) {
        foreach (char character in text) {
            keybd_event(0, (byte)character, 4, UIntPtr.Zero);
            keybd_event(0, (byte)character, 6, UIntPtr.Zero);
        }
    }

    public static void SendEnter() {
        keybd_event(0x0D, 0, 0, UIntPtr.Zero);
        keybd_event(0x0D, 0, 2, UIntPtr.Zero);
    }

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool SetForegroundWindow(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SetFocus(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool ShowWindow(IntPtr window, int command);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr GetWindow(IntPtr window, uint command);
}
"@

$TargetPath = [System.IO.Path]::GetFullPath($Path)
$TargetDirectory = [System.IO.Path]::GetDirectoryName($TargetPath)
$TargetFileName = [System.IO.Path]::GetFileName($TargetPath)
$Deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$Stage = "preflight"

function Get-NativeFileDialog {
    $titles = if ($Mode -eq "open") {
        @("Abrir", "Open", "Abrir archivo", "Open File")
    }
    else {
        @("Guardar como", "Save As", "Guardar", "Save")
    }
    foreach ($title in $titles) {
        $window = [ColumniaNativeDialogMethods]::FindWindow("#32770", $title)
        if ($window -ne [IntPtr]::Zero) {
            return $window
        }
    }
    return $null
}

function Focus-ColumniaWindow {
    $window = [ColumniaNativeDialogMethods]::FindWindow("Tauri Window", "Columnia")
    if ($window -ne [IntPtr]::Zero) {
        [void][ColumniaNativeDialogMethods]::ShowWindow($window, 5)
        [void][ColumniaNativeDialogMethods]::SetForegroundWindow($window)
    }
}

function Find-FileNameEditor {
    param([IntPtr]$Window)

    $combo = [ColumniaNativeDialogMethods]::GetDlgItem($Window, 1148)
    if ($combo -ne [IntPtr]::Zero) {
        $comboControl = [ColumniaNativeDialogMethods]::FindWindowEx($combo, [IntPtr]::Zero, "ComboBox", $null)
        if ($comboControl -eq [IntPtr]::Zero) {
            return [IntPtr]::Zero
        }
        $editor = [ColumniaNativeDialogMethods]::FindWindowEx($comboControl, [IntPtr]::Zero, "Edit", $null)
        if ($editor -ne [IntPtr]::Zero) {
            return $editor
        }
        return [IntPtr]::Zero
    }
    return [ColumniaNativeDialogMethods]::FindWindowEx($Window, [IntPtr]::Zero, "Edit", $null)
}

function Find-ActionButton {
    param([IntPtr]$Window)

    $knownButton = [ColumniaNativeDialogMethods]::GetDlgItem($Window, 1)
    if ($knownButton -ne [IntPtr]::Zero) {
        return $knownButton
    }
    return $null
}

function Close-Dialog {
    try {
        [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
    }
    catch {
        # El proceso principal se limpiará mediante el Job Object si el diálogo
        # deja de responder; no se registra el mensaje del sistema.
    }
}

function Write-Result {
    param(
        [string]$Status,
        [string]$Phase,
        [string]$ErrorCode = $null
    )

    [ordered]@{
        status = $Status
        phase = $Phase
        mode = $Mode
        fileName = $TargetFileName
        errorCode = $ErrorCode
    } | ConvertTo-Json -Compress
}

try {
    if ([string]::IsNullOrWhiteSpace($TargetDirectory) -or [string]::IsNullOrWhiteSpace($TargetFileName)) {
        throw "invalid_target"
    }
    if ($Mode -eq "open" -and -not (Test-Path -LiteralPath $TargetPath -PathType Leaf)) {
        throw "source_missing"
    }
    if ($Mode -eq "save" -and -not (Test-Path -LiteralPath $TargetDirectory -PathType Container)) {
        throw "destination_directory_missing"
    }

    Focus-ColumniaWindow
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        $Stage = "find_window"
        $dialog = Get-NativeFileDialog
        if ($dialog -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 150
            continue
        }

        $Stage = "find_filename_editor"
        $editor = Find-FileNameEditor -Window $dialog
        if ($editor -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 150
            continue
        }

        $Stage = "set_filename"
        [void][ColumniaNativeDialogMethods]::SetForegroundWindow($dialog)
        Start-Sleep -Milliseconds 250
        [void][ColumniaNativeDialogMethods]::SetFocus($editor)

        # The file name field is a ComboBoxEx32. Setting its child text alone
        # does not commit the selection in the common dialog; keyboard input
        # followed by Enter does.
        [ColumniaNativeDialogMethods]::SendControlA()
        [ColumniaNativeDialogMethods]::SendUnicodeText($TargetPath)
        [ColumniaNativeDialogMethods]::SendEnter()
        Start-Sleep -Milliseconds 100
        if ((Get-NativeFileDialog) -ne [IntPtr]::Zero) {
            [void][ColumniaNativeDialogMethods]::SetFocus($editor)
            [System.Windows.Forms.SendKeys]::SendWait("^a")
            [System.Windows.Forms.SendKeys]::SendWait($TargetPath)
            [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
        }
        Start-Sleep -Milliseconds 100
        if ((Get-NativeFileDialog) -ne [IntPtr]::Zero) {
            $Stage = "find_action_button"
            $button = Find-ActionButton -Window $dialog
            if ($button -eq [IntPtr]::Zero) {
                throw "action_button_not_found"
            }
            $Stage = "invoke_action_button"
            [void][ColumniaNativeDialogMethods]::SendMessage($button, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)
            Start-Sleep -Milliseconds 100
            if ((Get-NativeFileDialog) -ne [IntPtr]::Zero) {
                [ColumniaNativeDialogMethods]::SendEnter()
            }
        }

        $Stage = "wait_for_result"
        $waitDeadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
        while ([DateTimeOffset]::UtcNow -lt $waitDeadline) {
            $dialogStillOpen = (Get-NativeFileDialog) -ne [IntPtr]::Zero
            $targetExists = Test-Path -LiteralPath $TargetPath -PathType Leaf
            if (-not $dialogStillOpen -and $targetExists) {
                Write-Result -Status "passed" -Phase (if ($Mode -eq "open") { "native_dialog_opened" } else { "native_dialog_saved" })
                exit 0
            }
            if (-not $dialogStillOpen) {
                if ($Mode -eq "open") {
                    throw "open_dialog_closed_without_selection"
                }
                throw "save_dialog_closed_without_output"
            }
            Start-Sleep -Milliseconds 150
        }
        throw "native_dialog_action_timeout"
    }
    throw "native_dialog_timeout"
}
catch {
    Close-Dialog
    $errorCode = if ($_.Exception.Message -match "^[a-z0-9_]+$") {
        $_.Exception.Message
    }
    else {
        $Stage
    }
    Write-Result -Status "failed" -Phase "native_dialog_failed" -ErrorCode $errorCode
    exit 1
}
