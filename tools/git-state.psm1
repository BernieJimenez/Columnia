# OPS-20: the Git state a validation report records. It is read again when
# the run ends, because a gate takes minutes and the tree can change meanwhile.
function Get-RunGitState {
    param(
        [string]$Root,
        [string]$StartCommit,
        [string]$Branch,
        [bool]$StartDirty
    )

    $EndCommit = (git -C $Root rev-parse HEAD).Trim()
    $EndDirty = @(git -C $Root status --porcelain --untracked-files=all).Count -gt 0
    $Stale = $EndCommit -ne $StartCommit
    [ordered]@{
        commit = $StartCommit
        branch = $Branch
        # Dirty if it was dirty at either end, or if HEAD moved during the run.
        dirty = $StartDirty -or $EndDirty -or $Stale
        changedDuringRun = $Stale -or ($EndDirty -and -not $StartDirty)
        commitAtEnd = $EndCommit
    }
}

Export-ModuleMember -Function Get-RunGitState
