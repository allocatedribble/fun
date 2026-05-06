param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$TraceDiagnostics,
    [string]$RenderBackend = "dx12",
    [string]$PresentMode = "immediate",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [switch]$DryRun,
    [string]$OutputRoot = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "FunBench.Compat.ps1")

Invoke-FunBenchCompat `
    -Subcommand "rt-matrix" `
    -BoundParameters $PSBoundParameters `
    -ActualParameters @(
        "Release",
        "StaticBevy",
        "TraceDiagnostics",
        "RenderBackend",
        "PresentMode",
        "WarmupSeconds",
        "SampleSeconds",
        "DryRun",
        "OutputRoot"
    )
