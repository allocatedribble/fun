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

function New-PesterStackRequest {
    param([hashtable]$Bound = @{})

    New-StackRunRequest -BoundParameters $Bound -ProfileRoot (Join-Path $stackRoot "profiles")
}

function Save-PesterStackEnv {
    param([string[]]$Names)

    $snapshot = @{}
    foreach ($name in $Names) {
        $snapshot[$name] = [System.Environment]::GetEnvironmentVariable($name, "Process")
    }
    return $snapshot
}

function Restore-PesterStackEnv {
    param([hashtable]$Snapshot)

    foreach ($name in $Snapshot.Keys) {
        [System.Environment]::SetEnvironmentVariable($name, $Snapshot[$name], "Process")
    }
}

$stackEnvNames = @(
    "FUN_START_MODE",
    "FUN_CEF_UI_TRANSPORT_STATUS_PATH",
    "FUN_CEF_UI_PAINT_TRANSPORT",
    "FUN_CEF_UI_ACCELERATED_PAINT",
    "FUN_DISABLE_FPS_OVERLAY",
    "FUN_RENDER_UPLOAD_COUNTERS",
    "BEVY_RENDER_SHADER_DIAGNOSTICS",
    "FUN_GAME_SERVER_TLS_MODE",
    "FUN_GAME_SERVER_DEV_SESSION_TOKEN",
    "FUN_GAME_CLIENT_SESSION_TOKEN"
)

Describe "Stack run profiles" {
    It "default profile resolves to DX12 immediate" {
        $request = New-PesterStackRequest
        $request.RenderBackend | Should Be "dx12"
        $request.PresentMode | Should Be "immediate"
    }

    It "validates every checked-in profile" {
        foreach ($profileFile in Get-ChildItem -LiteralPath (Join-Path $stackRoot "profiles") -File -Filter *.json) {
            { Read-StackProfile -ProfileRoot (Join-Path $stackRoot "profiles") -Name $profileFile.BaseName } | Should Not Throw
        }
    }
}

Describe "Stack environment policy" {
    It "NoClient removes client-only env" {
        $snapshot = Save-PesterStackEnv -Names $stackEnvNames
        try {
            $env:FUN_START_MODE = "game"
            $env:FUN_CEF_UI_TRANSPORT_STATUS_PATH = "stale"
            $paths = [pscustomobject]@{ cef_transport_status_file = Join-Path $stackRoot "..\..\target\run-stack\cef-ui-transport.test.json" }
            $request = New-PesterStackRequest -Bound @{ NoClient = $true; CefUi = $true }
            Resolve-StackCefRequest -Request $request -Paths $paths | Out-Null
            Set-StackRuntimeEnv -Request $request
            Set-StackCefEnv -Request $request -Paths $paths
            Set-StackRenderEnv -Request $request

            $env:FUN_START_MODE | Should BeNullOrEmpty
            $env:FUN_CEF_UI_TRANSPORT_STATUS_PATH | Should BeNullOrEmpty
            $env:FUN_DISABLE_FPS_OVERLAY | Should BeNullOrEmpty
        }
        finally {
            Restore-PesterStackEnv -Snapshot $snapshot
        }
    }

    It "CEF d3d11on12 implies CEF UI" {
        $paths = [pscustomobject]@{ cef_transport_status_file = Join-Path $stackRoot "..\..\target\run-stack\cef-ui-transport.test.json" }
        $request = New-PesterStackRequest -Bound @{ CefPaintTransport = "d3d11on12" }
        Resolve-StackCefRequest -Request $request -Paths $paths | Out-Null
        $request.CefUi | Should Be $true
        $request.CefUiDx12AcceleratedPaint | Should Be $true
    }

    It "diagnostics flags set expected counters" {
        $snapshot = Save-PesterStackEnv -Names $stackEnvNames
        try {
            $request = New-PesterStackRequest -Bound @{ RenderDiagnostics = $true }
            $state = Get-StackDiagnosticsState -Request $request
            Set-StackDiagnosticsEnv -Request $request -Diagnostics $state
            $env:FUN_RENDER_UPLOAD_COUNTERS | Should Be "1"
            $env:BEVY_RENDER_SHADER_DIAGNOSTICS | Should Be "1"
        }
        finally {
            Restore-PesterStackEnv -Snapshot $snapshot
        }
    }
}

Describe "Stack build and security policy" {
    It "release build warns for debug-only diagnostics deterministically" {
        $request = New-PesterStackRequest -Bound @{ Release = $true; RenderDiagnostics = $true }
        $state = Get-StackDiagnosticsState -Request $request
        $plan = New-StackBuildPlan -Request $request -Diagnostics $state
        @($plan.warnings | Where-Object { $_.id -eq "release_debug_diagnostics_ignored" }).Count | Should Be 1
        @($plan.features).Count | Should Be 0
    }

    It "generates dev token only when both sides are empty" {
        $snapshot = Save-PesterStackEnv -Names $stackEnvNames
        try {
            Clear-StackEnvValue -Name "FUN_GAME_SERVER_TLS_MODE"
            Clear-StackEnvValue -Name "FUN_GAME_SERVER_DEV_SESSION_TOKEN"
            Clear-StackEnvValue -Name "FUN_GAME_CLIENT_SESSION_TOKEN"
            $events = Set-StackDevelopmentTokens -Request (New-PesterStackRequest)
            $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN | Should Not BeNullOrEmpty
            $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN | Should Be $env:FUN_GAME_CLIENT_SESSION_TOKEN
            @($events | Where-Object { $_.id -eq "dev_session_token_generated" }).Count | Should Be 1
        }
        finally {
            Restore-PesterStackEnv -Snapshot $snapshot
        }
    }

    It "mismatched tokens warn without silently mutating both sides" {
        $snapshot = Save-PesterStackEnv -Names $stackEnvNames
        try {
            $env:FUN_GAME_SERVER_TLS_MODE = "development"
            $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = "server-token"
            $env:FUN_GAME_CLIENT_SESSION_TOKEN = "client-token"
            $events = Set-StackDevelopmentTokens -Request (New-PesterStackRequest)
            $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN | Should Be "server-token"
            $env:FUN_GAME_CLIENT_SESSION_TOKEN | Should Be "client-token"
            @($events | Where-Object { $_.id -eq "dev_session_token_mismatch" }).Count | Should Be 1
        }
        finally {
            Restore-PesterStackEnv -Snapshot $snapshot
        }
    }
}
