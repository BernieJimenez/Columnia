# OPS-17: helpers of tools/check-supply-chain.ps1, kept apart so they can be tested.

# The advisories that src-tauri/deny.toml ignores. That file is the single
# list, and each entry must say why it is ignored.
function Get-DenyIgnoredAdvisories {
    param([string]$DenyToml)

    $Ignored = foreach ($Match in [regex]::Matches($DenyToml, '\{\s*id\s*=\s*"(RUSTSEC-\d{4}-\d{4})"\s*(?:,\s*reason\s*=\s*"([^"]*)")?\s*\}')) {
        if ([string]::IsNullOrWhiteSpace($Match.Groups[2].Value)) {
            throw "deny.toml ignora $($Match.Groups[1].Value) sin un motivo (reason)."
        }
        $Match.Groups[1].Value
    }
    @($Ignored)
}

# "passed-with-skips" when every check that ran passed but a tool was missing,
# so an omission never reads the same as a full audit.
function Get-SupplyChainStatus {
    param(
        [System.Collections.IDictionary]$Results,
        [string]$FailureMessage
    )

    $Failed = @($Results.Keys | Where-Object { $Results[$_].status -eq "failed" })
    $Skipped = @($Results.Keys | Where-Object { $Results[$_].status -eq "unavailable" })
    $Status = if (-not [string]::IsNullOrEmpty($FailureMessage) -or $Failed.Count -gt 0) {
        "failed"
    }
    elseif ($Skipped.Count -gt 0) {
        "passed-with-skips"
    }
    else {
        "passed"
    }
    [ordered]@{ status = $Status; failedChecks = $Failed; skippedChecks = $Skipped }
}

Export-ModuleMember -Function Get-DenyIgnoredAdvisories, Get-SupplyChainStatus
