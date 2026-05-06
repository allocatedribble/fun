param(
    [string]$Package = "game_client",
    [string]$Bench = "client_costs",
    [string]$Baseline = "",
    [string]$SaveBaseline = "",
    [double]$WarmupSeconds = 0.25,
    [double]$MeasurementSeconds = 0.75,
    [int]$SampleSize = 10,
    [switch]$OpenReport,
    [switch]$DryRun,
    [string]$OutputRoot = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "FunBench.Compat.ps1")

Invoke-FunBenchCompat `
    -Subcommand "criterion" `
    -BoundParameters $PSBoundParameters `
    -ActualParameters @(
        "Package",
        "Bench",
        "Baseline",
        "SaveBaseline",
        "WarmupSeconds",
        "MeasurementSeconds",
        "SampleSize",
        "OpenReport",
        "DryRun",
        "OutputRoot"
    )
