param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$TraceDiagnostics,
    [switch]$FrameTimeDiagnostics,
    [ValidateSet("quick", "full", "present", "cef_transport", "stream_pressure")]
    [string]$MatrixSize = "quick",
    [string[]]$Lane = @(),
    [switch]$PlanOnly,
    [switch]$ContinueOnFailure,
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [int]$WindowWidth = 1280,
    [int]$WindowHeight = 720,
    [string]$OutputRoot = "",
    [switch]$DryRun,
    [ValidateSet("human", "json", "json-pretty")]
    [string]$Format = "human",
    [ValidateSet("off", "summary", "verbose")]
    [string]$Diagnostics = "off",
    [string]$Output = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "FunBench.Compat.ps1")

Invoke-FunBenchCompat `
    -Subcommand "dx12-parity" `
    -BoundParameters $PSBoundParameters `
    -FlagAliases @{ Lane = "lane" } `
    -ActualParameters @(
        "Release",
        "StaticBevy",
        "TraceDiagnostics",
        "FrameTimeDiagnostics",
        "MatrixSize",
        "Lane",
        "PlanOnly",
        "ContinueOnFailure",
        "WarmupSeconds",
        "SampleSeconds",
        "WindowWidth",
        "WindowHeight",
        "OutputRoot",
        "DryRun",
        "Format",
        "Diagnostics",
        "Output"
    )
