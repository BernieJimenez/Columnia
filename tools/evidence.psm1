Import-Module (Join-Path $PSScriptRoot "git-state.psm1") -Force

# OPS-24: the one header every summary.json carries, so a gate can tell which
# commit, tree and version a piece of evidence measured.
function New-EvidenceHeader {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$StartedAt,
        [string]$StartCommit,
        [Nullable[bool]]$StartDirty
    )

    if ([string]::IsNullOrWhiteSpace($StartCommit)) {
        $StartCommit = (git -C $Root rev-parse HEAD 2>$null)
        if ($null -ne $StartCommit) { $StartCommit = $StartCommit.Trim() }
    }
    if ($null -eq $StartDirty) {
        $StartDirty = @(git -C $Root status --porcelain --untracked-files=all 2>$null).Count -gt 0
    }
    $Branch = (git -C $Root branch --show-current 2>$null)
    $Git = if ([string]::IsNullOrWhiteSpace($StartCommit)) {
        [ordered]@{ commit = $null; branch = $null; dirty = $true; changedDuringRun = $false; commitAtEnd = $null }
    }
    else {
        Get-RunGitState -Root $Root -StartCommit $StartCommit -Branch ([string]$Branch).Trim() -StartDirty ([bool]$StartDirty)
    }
    $Version = $null
    $PackagePath = Join-Path $Root "package.json"
    if (Test-Path -LiteralPath $PackagePath) {
        $Version = (Get-Content -Encoding UTF8 -LiteralPath $PackagePath -Raw | ConvertFrom-Json).version
    }
    [ordered]@{
        contract = "columnia-evidence-header"
        schemaVersion = 1
        commit = $Git.commit
        branch = $Git.branch
        dirty = $Git.dirty
        changedDuringRun = $Git.changedDuringRun
        version = $Version
        startedAt = $StartedAt
        finishedAt = [DateTimeOffset]::UtcNow.ToString("o")
    }
}

# Adds (or replaces) the `evidence` header of a summary on its way to JSON:
#   $Summary | Add-EvidenceHeader -Root $ProjectRoot -StartedAt $EvidenceStartedAt | ConvertTo-Json
function Add-EvidenceHeader {
    param(
        [Parameter(Mandatory = $true, ValueFromPipeline = $true)]$Summary,
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$StartedAt
    )

    process {
        $Header = New-EvidenceHeader -Root $Root -StartedAt $StartedAt
        if ($Summary -is [System.Collections.IDictionary]) {
            $Summary["evidence"] = $Header
        }
        else {
            $Summary | Add-Member -NotePropertyName "evidence" -NotePropertyValue $Header -Force
        }
        $Summary
    }
}

# The reason a summary does not count for `HeadCommit`, or $null when it does.
function Get-EvidenceCommitProblem {
    param(
        [Parameter(Mandatory = $true)]$Document,
        [Parameter(Mandatory = $true)][string]$HeadCommit
    )

    $Header = $Document.evidence
    if ($null -eq $Header -or [string]::IsNullOrWhiteSpace([string]$Header.commit)) {
        return "La evidencia no tiene la cabecera común con su commit; vuelve a medir."
    }
    $Commit = [string]$Header.commit
    if ($Commit -ne $HeadCommit) {
        return "Evidencia de otro commit ($($Commit.Substring(0, [Math]::Min(7, $Commit.Length)))); vuelve a medir en $($HeadCommit.Substring(0, [Math]::Min(7, $HeadCommit.Length)))."
    }
    if ($Header.dirty -eq $true) {
        return "La evidencia se midió con cambios sin commit; vuelve a medir con el árbol limpio."
    }
    return $null
}

Export-ModuleMember -Function New-EvidenceHeader, Add-EvidenceHeader, Get-EvidenceCommitProblem
