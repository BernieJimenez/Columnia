# OPS-21: ends a process and every child it started. Process.Kill($true) does
# not exist in Windows PowerShell 5.1, and Kill() alone left the columnia_lib
# test binary running after a benchmark timeout.
function Stop-ProcessTree {
    param([System.Diagnostics.Process]$Process)

    if ($null -eq $Process) { return }
    try {
        # taskkill writes to stderr when the process already exited; under
        # -ErrorAction Stop that must not replace the caller's own error.
        $null = & taskkill.exe /T /F /PID $Process.Id 2>&1
    }
    catch {
    }
    try { $Process.WaitForExit(5000) | Out-Null } catch { }
}

Export-ModuleMember -Function Stop-ProcessTree
