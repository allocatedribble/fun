param()

$ErrorActionPreference = "Stop"

$stackRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
foreach ($module in @(
    "Stack.Paths.ps1",
    "Stack.Env.ps1",
    "Stack.Security.ps1",
    "Stack.Cef.ps1",
    "Stack.Diagnostics.ps1",
    "Stack.Render.ps1",
    "Stack.Build.ps1",
    "Stack.Process.ps1",
    "Stack.Schema.ps1"
)) {
    . (Join-Path $stackRoot $module)
}

function Assert-StackTest {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-StackEqual {
    param(
        [object]$Actual,
        [object]$Expected,
        [string]$Message
    )

    if ($Actual -ne $Expected) {
        throw "$Message actual='$Actual' expected='$Expected'"
    }
}

function New-TestRequest {
    param([hashtable]$Bound = @{})

    New-StackRunRequest -BoundParameters $Bound -ProfileRoot (Join-Path $stackRoot "profiles")
}

function Save-TestEnv {
    param([string[]]$Names)

    $snapshot = @{}
    foreach ($name in $Names) {
        $snapshot[$name] = [System.Environment]::GetEnvironmentVariable($name, "Process")
    }
    return $snapshot
}

function Restore-TestEnv {
    param([hashtable]$Snapshot)

    foreach ($name in $Snapshot.Keys) {
        [System.Environment]::SetEnvironmentVariable($name, $Snapshot[$name], "Process")
    }
}

$envNames = @(
    "FUN_START_MODE",
    "FUN_RENDERER_CAPABILITY_REPORT_PATH",
    "FUN_CEF_UI_TRANSPORT_STATUS_PATH",
    "FUN_CEF_UI_PAINT_TRANSPORT",
    "FUN_CEF_UI_ACCELERATED_PAINT",
    "FUN_CEF_UI_ALLOW_CPU_FALLBACK",
    "FUN_DISABLE_FPS_OVERLAY",
    "FUN_RENDER_UPLOAD_COUNTERS",
    "BEVY_RENDER_SHADER_DIAGNOSTICS",
    "FUN_GAME_SERVER_TLS_MODE",
    "FUN_GAME_SERVER_DEV_SESSION_TOKEN",
    "FUN_GAME_CLIENT_SESSION_TOKEN"
)
$snapshot = Save-TestEnv -Names $envNames
$results = @()

try {
    $profileRoot = Join-Path $stackRoot "profiles"
    foreach ($profileFile in Get-ChildItem -LiteralPath $profileRoot -File -Filter *.json) {
        Read-StackProfile -ProfileRoot $profileRoot -Name $profileFile.BaseName | Out-Null
    }
    $results += "profiles_validate"

    $defaultRequest = New-TestRequest
    Assert-StackEqual -Actual $defaultRequest.RenderBackend -Expected "dx12" -Message "default profile backend"
    Assert-StackEqual -Actual $defaultRequest.PresentMode -Expected "immediate" -Message "default profile present mode"
    $results += "default_profile_dx12_immediate"

    $paths = [pscustomobject]@{
        cef_transport_status_file = Join-Path $stackRoot "..\..\target\run-stack\cef-ui-transport.test.json"
        renderer_capability_report_file = Join-Path $stackRoot "..\..\target\run-stack\renderer-capabilities.test.json"
    }
    $env:FUN_START_MODE = "game"
    $env:FUN_RENDERER_CAPABILITY_REPORT_PATH = "stale"
    $env:FUN_CEF_UI_TRANSPORT_STATUS_PATH = "stale"
    $noClientRequest = New-TestRequest -Bound @{ NoClient = $true; CefUi = $true }
    Resolve-StackCefRequest -Request $noClientRequest -Paths $paths | Out-Null
    Set-StackRuntimeEnv -Request $noClientRequest
    Set-StackCefEnv -Request $noClientRequest -Paths $paths
    Set-StackRenderEnv -Request $noClientRequest -Paths $paths
    Assert-StackTest -Condition ([string]::IsNullOrWhiteSpace($env:FUN_START_MODE)) -Message "NoClient should remove FUN_START_MODE"
    Assert-StackTest -Condition ([string]::IsNullOrWhiteSpace($env:FUN_RENDERER_CAPABILITY_REPORT_PATH)) -Message "NoClient should remove renderer capability env"
    Assert-StackTest -Condition ([string]::IsNullOrWhiteSpace($env:FUN_CEF_UI_TRANSPORT_STATUS_PATH)) -Message "NoClient should remove CEF status env"
    Assert-StackTest -Condition ([string]::IsNullOrWhiteSpace($env:FUN_DISABLE_FPS_OVERLAY)) -Message "NoClient ignored CEF UI should not set overlay env"
    $results += "no_client_removes_client_env"

    $cefRequest = New-TestRequest -Bound @{ CefPaintTransport = "d3d11on12" }
    Resolve-StackCefRequest -Request $cefRequest -Paths $paths | Out-Null
    Assert-StackTest -Condition $cefRequest.CefUi -Message "d3d11on12 transport should enable CEF UI"
    Assert-StackTest -Condition $cefRequest.CefUiDx12AcceleratedPaint -Message "d3d11on12 transport should enable accelerated paint"
    $results += "cef_d3d11on12_implies_cef_ui"

    $diagRequest = New-TestRequest -Bound @{ RenderDiagnostics = $true }
    $diagState = Get-StackDiagnosticsState -Request $diagRequest
    Set-StackDiagnosticsEnv -Request $diagRequest -Diagnostics $diagState
    Assert-StackEqual -Actual $env:FUN_RENDER_UPLOAD_COUNTERS -Expected "1" -Message "render upload counter env"
    Assert-StackEqual -Actual $env:BEVY_RENDER_SHADER_DIAGNOSTICS -Expected "1" -Message "shader diagnostic env"
    $results += "diagnostics_flags_set_counters"

    $releaseRequest = New-TestRequest -Bound @{ Release = $true; RenderDiagnostics = $true }
    $releaseDiagnostics = Get-StackDiagnosticsState -Request $releaseRequest
    $releasePlan = New-StackBuildPlan -Request $releaseRequest -Diagnostics $releaseDiagnostics
    Assert-StackTest -Condition ([bool]($releasePlan.warnings | Where-Object { $_.id -eq "release_debug_diagnostics_ignored" } | Select-Object -First 1)) -Message "release diagnostics warning should be deterministic"
    $results += "release_warns_for_debug_diagnostics"

    Clear-StackEnvValue -Name "FUN_GAME_SERVER_TLS_MODE"
    Clear-StackEnvValue -Name "FUN_GAME_SERVER_DEV_SESSION_TOKEN"
    Clear-StackEnvValue -Name "FUN_GAME_CLIENT_SESSION_TOKEN"
    $tokenEvents = Set-StackDevelopmentTokens -Request $defaultRequest
    Assert-StackTest -Condition (-not [string]::IsNullOrWhiteSpace($env:FUN_GAME_SERVER_DEV_SESSION_TOKEN)) -Message "server token should be generated"
    Assert-StackEqual -Actual $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN -Expected $env:FUN_GAME_CLIENT_SESSION_TOKEN -Message "generated tokens should match"
    Assert-StackTest -Condition ([bool]($tokenEvents | Where-Object { $_.id -eq "dev_session_token_generated" } | Select-Object -First 1)) -Message "token generation event missing"
    $results += "dev_token_generated_when_both_empty"

    $env:FUN_GAME_SERVER_TLS_MODE = "development"
    $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = "server-token"
    $env:FUN_GAME_CLIENT_SESSION_TOKEN = "client-token"
    $mismatchEvents = Set-StackDevelopmentTokens -Request $defaultRequest
    Assert-StackEqual -Actual $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN -Expected "server-token" -Message "mismatched server token should not mutate"
    Assert-StackEqual -Actual $env:FUN_GAME_CLIENT_SESSION_TOKEN -Expected "client-token" -Message "mismatched client token should not mutate"
    Assert-StackTest -Condition ([bool]($mismatchEvents | Where-Object { $_.id -eq "dev_session_token_mismatch" } | Select-Object -First 1)) -Message "mismatched token warning missing"
    $results += "mismatched_tokens_warn_without_mutation"
}
finally {
    Restore-TestEnv -Snapshot $snapshot
}

[pscustomobject]@{
    schema_version = "stack_self_test_v1"
    status = "pass"
    tests = $results
} | ConvertTo-Json -Depth 4
