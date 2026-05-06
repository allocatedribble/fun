param(
    [string]$RepoRoot = "",
    [string]$CategoryPath = ".dx12_change_category",
    [string[]]$SummaryPath = @(),
    [ValidateSet("human", "json", "json-pretty")]
    [string]$Format = "human",
    [ValidateSet("off", "summary", "verbose")]
    [string]$Diagnostics = "off",
    [string]$Output = "",
    [string]$ReportPath = "",
    [string]$JsonOut = "",
    [switch]$SelfTest,
    [switch]$FailOnWarning,
    [switch]$Explain
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\scripts\FunBench.Compat.ps1")

$bound = @{}
foreach ($entry in $PSBoundParameters.GetEnumerator()) {
    $bound[$entry.Key] = $entry.Value
}
if (-not [string]::IsNullOrWhiteSpace($RepoRoot)) {
    $bound["FunRoot"] = $RepoRoot
}

Invoke-FunBenchCompat `
    -Subcommand "dx12-doctrine-check" `
    -BoundParameters $bound `
    -FlagAliases @{
        RepoRoot = "fun-root"
        FunRoot = "fun-root"
        CategoryPath = "category-path"
        SummaryPath = "summary-path"
        ReportPath = "report-path"
        JsonOut = "json-out"
        SelfTest = "self-test"
        FailOnWarning = "fail-on-warning"
    } `
    -ActualParameters @(
        "RepoRoot",
        "FunRoot",
        "CategoryPath",
        "SummaryPath",
        "Format",
        "Diagnostics",
        "Output",
        "ReportPath",
        "JsonOut",
        "SelfTest",
        "FailOnWarning",
        "Explain"
    )
