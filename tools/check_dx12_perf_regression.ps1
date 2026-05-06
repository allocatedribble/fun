param(
    [string]$Baseline = "",
    [string]$Current = "",
    [string]$EnvelopePath = "",
    [string]$Lane = "",
    [string]$ReportPath = "",
    [string]$JsonOut = "",
    [ValidateSet("human", "json", "json-pretty")]
    [string]$Format = "human",
    [ValidateSet("off", "summary", "verbose")]
    [string]$Diagnostics = "off",
    [string]$Output = "",
    [switch]$SelfTest,
    [switch]$FailOnWarning,
    [switch]$Explain
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\scripts\FunBench.Compat.ps1")

Invoke-FunBenchCompat `
    -Subcommand "dx12-perf-regression-check" `
    -BoundParameters $PSBoundParameters `
    -FlagAliases @{
        EnvelopePath = "envelope-path"
        ReportPath = "report-path"
        JsonOut = "json-out"
        SelfTest = "self-test"
        FailOnWarning = "fail-on-warning"
    } `
    -ActualParameters @(
        "Baseline",
        "Current",
        "EnvelopePath",
        "Lane",
        "ReportPath",
        "JsonOut",
        "Format",
        "Diagnostics",
        "Output",
        "SelfTest",
        "FailOnWarning",
        "Explain"
    )
