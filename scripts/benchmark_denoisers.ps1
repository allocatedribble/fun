param(
    [string[]]$Modes = @("off", "cheap-temporal", "balanced-fast", "balanced", "quality", "rr"),
    [string]$RenderBackend = "dx12",
    [string]$PresentMode = "immediate",
    [string]$SolariInternalScale = "1.0",
    [string]$SolariArch = "budgeted",
    [int]$SolariTargetFps = 144,
    [int]$SolariFrameBudgetNs = 6944444,
    [int]$SolariGpuBudgetNs = 3000000,
    [string]$SolariVisualTarget = "competitive",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$DisableMeshlets,
    [switch]$TraceDiagnostics,
    [switch]$DryRun,
    [string]$OutputRoot = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "FunBench.Compat.ps1")

Invoke-FunBenchCompat `
    -Subcommand "denoisers" `
    -BoundParameters $PSBoundParameters `
    -FlagAliases @{ Modes = "mode" } `
    -ActualParameters @(
        "Modes",
        "RenderBackend",
        "PresentMode",
        "SolariInternalScale",
        "SolariArch",
        "SolariTargetFps",
        "SolariFrameBudgetNs",
        "SolariGpuBudgetNs",
        "SolariVisualTarget",
        "WarmupSeconds",
        "SampleSeconds",
        "Release",
        "StaticBevy",
        "DisableMeshlets",
        "TraceDiagnostics",
        "DryRun",
        "OutputRoot"
    )
