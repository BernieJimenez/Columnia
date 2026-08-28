param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("open", "save")]
    [string]$Mode,
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [ValidateRange(5, 120)]
    [int]$TimeoutSeconds = 90
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
Add-Type -AssemblyName UIAutomationTypes -ErrorAction Stop
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class ColumniaNativeDialogMethods {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr FindWindow(string className, string windowName);

    public delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

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
    public static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool IsWindowEnabled(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extraInfo);

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyboardInput {
        public ushort virtualKey;
        public ushort scanCode;
        public uint flags;
        public uint time;
        public UIntPtr extraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Input {
        public uint type;
        public KeyboardInput keyboard;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint inputCount, Input[] inputs, int inputSize);

    public static void SendControlA() {
        keybd_event(0x11, 0, 0, UIntPtr.Zero);
        keybd_event(0x41, 0, 0, UIntPtr.Zero);
        keybd_event(0x41, 0, 2, UIntPtr.Zero);
        keybd_event(0x11, 0, 2, UIntPtr.Zero);
    }

    public static void SendUnicodeText(string text) {
        foreach (char character in text) {
            var inputs = new Input[2];
            inputs[0].type = 1;
            inputs[0].keyboard.scanCode = character;
            inputs[0].keyboard.flags = 4;
            inputs[1].type = 1;
            inputs[1].keyboard.scanCode = character;
            inputs[1].keyboard.flags = 6;
            SendInput(2, inputs, Marshal.SizeOf(typeof(Input)));
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

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    public static IntPtr FindVisibleWindowForProcess(
        string className,
        string windowName,
        uint processId,
        IntPtr owner
    ) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((window, _) => {
            if (!IsWindowVisible(window)) {
                return true;
            }
            uint candidateProcessId;
            GetWindowThreadProcessId(window, out candidateProcessId);
            if (candidateProcessId != processId && GetWindow(window, 4) != owner) {
                return true;
            }
            var candidateClass = new StringBuilder(256);
            var candidateName = new StringBuilder(256);
            GetClassName(window, candidateClass, candidateClass.Capacity);
            GetWindowText(window, candidateName, candidateName.Capacity);
            if (string.Equals(candidateClass.ToString(), className, StringComparison.Ordinal)
                && string.Equals(candidateName.ToString(), windowName, StringComparison.Ordinal)) {
                result = window;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
"@

$TargetPath = [System.IO.Path]::GetFullPath($Path)
$TargetDirectory = [System.IO.Path]::GetDirectoryName($TargetPath)
$TargetFileName = [System.IO.Path]::GetFileName($TargetPath)
$Deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$Stage = "preflight"
$ActionDiagnostics = [System.Collections.Generic.List[string]]::new()

function Get-ColumniaWindow {
    return [ColumniaNativeDialogMethods]::FindWindow("Tauri Window", "Columnia")
}

function Get-WindowProcessId {
    param([IntPtr]$Window)

    if ($Window -eq [IntPtr]::Zero) {
        return 0
    }
    [uint32]$processId = 0
    [void][ColumniaNativeDialogMethods]::GetWindowThreadProcessId($Window, [ref]$processId)
    return [int]$processId
}

function Get-NativeFileDialogElement {
    try {
        $titles = if ($Mode -eq "open") {
            @("Abrir", "Open", "Abrir archivo", "Open File")
        }
        else {
            @("Guardar como", "Save As", "Guardar", "Save")
        }

        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $windows = $root.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        for ($index = 0; $index -lt $windows.Count; $index++) {
            $window = $windows.Item($index)
            $current = $window.Current
            if ($current.ClassName -ne "#32770" -or $titles -notcontains $current.Name) {
                continue
            }
            $columniaProcessId = Get-WindowProcessId -Window (Get-ColumniaWindow)
            if ($columniaProcessId -ne 0 -and $current.ProcessId -ne $columniaProcessId) {
                [void]$ActionDiagnostics.Add("native_dialog_process_mismatch")
                continue
            }
            return $window
        }
    }
    catch {
        # UI Automation can briefly expose a stale shell tree while a common
        # dialog refreshes. The Win32 title/handle path remains authoritative.
    }
    return $null
}

function Get-NativeFileDialog {
    $titles = if ($Mode -eq "open") {
        @("Abrir", "Open", "Abrir archivo", "Open File")
    }
    else {
        @("Guardar como", "Save As", "Guardar", "Save")
    }
    $columniaWindow = Get-ColumniaWindow
    $columniaProcessId = Get-WindowProcessId -Window $columniaWindow
    foreach ($title in $titles) {
        $window = [ColumniaNativeDialogMethods]::FindVisibleWindowForProcess(
            "#32770",
            $title,
            [uint32]$columniaProcessId,
            $columniaWindow
        )
        if ($window -ne [IntPtr]::Zero) {
            return $window
        }
    }

    $automationDialog = Get-NativeFileDialogElement
    if ($null -ne $automationDialog) {
        return [IntPtr]$automationDialog.Current.NativeWindowHandle
    }
    return [IntPtr]::Zero
}

function Focus-ColumniaWindow {
    $window = Get-ColumniaWindow
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
    # The Windows Save As dialog uses FileNameControlHost/Edit id 1001,
    # whereas Open uses the legacy ComboBox/Edit id 1148.
    $saveEditorHost = [ColumniaNativeDialogMethods]::GetDlgItem($Window, 1001)
    if ($saveEditorHost -ne [IntPtr]::Zero) {
        $saveEditor = [ColumniaNativeDialogMethods]::FindWindowEx($saveEditorHost, [IntPtr]::Zero, "Edit", $null)
        if ($saveEditor -ne [IntPtr]::Zero) {
            return $saveEditor
        }
        return $saveEditorHost
    }
    return [ColumniaNativeDialogMethods]::FindWindowEx($Window, [IntPtr]::Zero, "Edit", $null)
}

function Find-ActionButton {
    param([object]$Window)

    $windowHandle = [IntPtr]$Window
    if ($windowHandle -eq [IntPtr]::Zero) {
        return [IntPtr]::Zero
    }

    $knownButton = [ColumniaNativeDialogMethods]::GetDlgItem($windowHandle, 1)
    if ($knownButton -ne [IntPtr]::Zero) {
        return $knownButton
    }
    return [IntPtr]::Zero
}

function Invoke-ActionWin32Button {
    param([object]$Window)

    try {
        $windowHandle = [IntPtr]$Window
        if ($windowHandle -eq [IntPtr]::Zero) {
            [void]$ActionDiagnostics.Add("win32_window_missing")
            return $false
        }
        $button = Find-ActionButton -Window $windowHandle
        if ($button -eq [IntPtr]::Zero) {
            [void]$ActionDiagnostics.Add("win32_button_missing")
            return $false
        }
        if (-not [ColumniaNativeDialogMethods]::IsWindowEnabled($button)) {
            [void]$ActionDiagnostics.Add("win32_button_disabled")
            return $false
        }
        [void][ColumniaNativeDialogMethods]::SetFocus($button)
        [void][ColumniaNativeDialogMethods]::PostMessage($button, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)
        [void][ColumniaNativeDialogMethods]::SendMessage($button, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)
        [void][ColumniaNativeDialogMethods]::SendMessage($windowHandle, 0x0111, [IntPtr]::new(1), $button)
        [void]$ActionDiagnostics.Add("win32_button_invoked")
        return $true
    }
    catch {
        [void]$ActionDiagnostics.Add("win32_button_exception")
        return $false
    }
}

function Find-FileNameAutomationElement {
    param([System.Windows.Automation.AutomationElement]$Window)

    if ($null -eq $Window) {
        return $null
    }
    $elements = $Window.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    for ($index = 0; $index -lt $elements.Count; $index++) {
        $element = $elements.Item($index)
        $current = $element.Current
        if (
            $current.ControlType.ProgrammaticName -eq "ControlType.Edit" -and
            ($current.AutomationId -eq "1148" -or $current.AutomationId -eq "1001")
        ) {
            return $element
        }
    }
    return $null
}

function Set-FileNameAutomationValue {
    param(
        [System.Windows.Automation.AutomationElement]$Editor,
        [string]$Value
    )

    if ($null -eq $Editor) {
        return $false
    }
    try {
        $pattern = $Editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $pattern.SetValue($Value)
        return $true
    }
    catch {
        return $false
    }
}

function Invoke-ActionAutomationButton {
    param([System.Windows.Automation.AutomationElement]$Window)

    if ($null -eq $Window) {
        [void]$ActionDiagnostics.Add("uia_button_window_missing")
        return $false
    }
    try {
        $elements = $Window.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        for ($index = 0; $index -lt $elements.Count; $index++) {
            $element = $elements.Item($index)
            $current = $element.Current
            if ($current.ControlType.ProgrammaticName -ne "ControlType.Button" -or $current.AutomationId -ne "1") {
                continue
            }
            if (-not $current.IsEnabled) {
                [void]$ActionDiagnostics.Add("uia_button_disabled")
                return $false
            }
            try {
                $pattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                $pattern.Invoke()
                [void]$ActionDiagnostics.Add("uia_button_invoked")
                return $true
            }
            catch {
                [void]$ActionDiagnostics.Add("uia_button_invoke_exception")
                return $false
            }
        }
    }
    catch {
        [void]$ActionDiagnostics.Add("uia_button_tree_exception")
        return $false
    }
    return $false
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
        diagnostics = @($ActionDiagnostics)
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
    $focusAttempt = 0
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        $Stage = "find_window"
        $dialog = Get-NativeFileDialog
        if ($dialog -eq [IntPtr]::Zero) {
            if (($focusAttempt % 4) -eq 0) {
                Focus-ColumniaWindow
            }
            $focusAttempt++
            Start-Sleep -Milliseconds 150
            continue
        }

        $dialogElement = Get-NativeFileDialogElement

        $Stage = "find_filename_editor"
        $automationEditor = Find-FileNameAutomationElement -Window $dialogElement
        $editor = Find-FileNameEditor -Window $dialog
        if ($editor -eq [IntPtr]::Zero -and $null -ne $automationEditor) {
            $editor = [IntPtr]$automationEditor.Current.NativeWindowHandle
        }
        if ($editor -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 150
            continue
        }

        $Stage = "set_filename"
        [void][ColumniaNativeDialogMethods]::SetForegroundWindow($dialog)
        Start-Sleep -Milliseconds 250
        [void][ColumniaNativeDialogMethods]::SetFocus($editor)

        # UI Automation updates the common-dialog model, not only the HWND text.
        # WM_SETTEXT alone can leave Open/Save logically disabled or uncommitted.
        $textSet = Set-FileNameAutomationValue -Editor $automationEditor -Value $TargetPath
        if (-not $textSet) {
            $textSet = [ColumniaNativeDialogMethods]::SetWindowText($editor, $TargetPath)
        }
        if (-not $textSet) {
            [ColumniaNativeDialogMethods]::SendControlA()
            [ColumniaNativeDialogMethods]::SendUnicodeText($TargetPath)
        }
        Start-Sleep -Milliseconds 250

        $Stage = "invoke_action_button"
        # Updating the filename causes the shell dialog to rebuild parts of its
        # UI tree. Reacquire the root element before locating the command.
        $currentDialog = Get-NativeFileDialog
        if ($currentDialog -ne [IntPtr]::Zero) {
            $dialog = $currentDialog
        }
        $currentDialogElement = Get-NativeFileDialogElement
        $win32Invoked = Invoke-ActionWin32Button -Window $dialog
        $automationInvoked = $false
        if (-not $win32Invoked) {
            $automationInvoked = Invoke-ActionAutomationButton -Window $currentDialogElement
        }
        if (-not $automationInvoked -and -not $win32Invoked) {
            [ColumniaNativeDialogMethods]::SendEnter()
        }
        Start-Sleep -Milliseconds 400
        if ((Get-NativeFileDialog) -ne [IntPtr]::Zero) {
            $Stage = "invoke_action_button_fallback"
            $currentDialog = Get-NativeFileDialog
            if ($currentDialog -ne [IntPtr]::Zero) {
                $dialog = $currentDialog
            }
            $currentDialogElement = Get-NativeFileDialogElement
            $win32Invoked = Invoke-ActionWin32Button -Window $dialog
            if (-not $win32Invoked) {
                [void](Invoke-ActionAutomationButton -Window $currentDialogElement)
            }
            Start-Sleep -Milliseconds 250
            if ((Get-NativeFileDialog) -ne [IntPtr]::Zero) {
                [ColumniaNativeDialogMethods]::SendEnter()
            }
        }

        $Stage = "wait_for_result"
        $waitDeadline = [DateTimeOffset]::UtcNow.AddSeconds(15)
        while ([DateTimeOffset]::UtcNow -lt $waitDeadline) {
            $dialogStillOpen = (Get-NativeFileDialog) -ne [IntPtr]::Zero
            $targetExists = Test-Path -LiteralPath $TargetPath -PathType Leaf
            if (-not $dialogStillOpen -and $targetExists) {
                $successPhase = if ($Mode -eq "open") { "native_dialog_opened" } else { "native_dialog_saved" }
                Write-Result -Status "passed" -Phase $successPhase
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
    $exceptionType = $_.Exception.GetType().Name
    $errorCode = if ($_.Exception.Message -match "^[a-z0-9_]+$") {
        $_.Exception.Message
    }
    else {
        "{0}_{1}" -f $Stage, $exceptionType
    }
    Write-Result -Status "failed" -Phase "native_dialog_failed" -ErrorCode $errorCode
    exit 1
}
