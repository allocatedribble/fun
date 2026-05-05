function New-LocalStackSessionToken {
    $bytes = [byte[]]::new(32)
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $rng.GetBytes($bytes)
        return [Convert]::ToBase64String($bytes)
    }
    finally {
        $rng.Dispose()
    }
}

function Set-StackDevelopmentTokens {
    param([pscustomobject]$Request)

    $events = @()
    if (-not $env:FUN_GAME_SERVER_TLS_MODE) {
        $env:FUN_GAME_SERVER_TLS_MODE = "development"
    }
    if ($env:FUN_GAME_SERVER_TLS_MODE -ine "development") {
        return $events
    }

    $serverDevToken = $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN
    $clientSessionToken = $env:FUN_GAME_CLIENT_SESSION_TOKEN

    if ([string]::IsNullOrWhiteSpace($serverDevToken) -and [string]::IsNullOrWhiteSpace($clientSessionToken)) {
        $localStackToken = New-LocalStackSessionToken
        $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = $localStackToken
        $env:FUN_GAME_CLIENT_SESSION_TOKEN = $localStackToken
        $events += [pscustomobject]@{
            id = "dev_session_token_generated"
            severity = "info"
            message = "Using ephemeral local development session token for stack client/server admission."
        }
    }
    elseif ([string]::IsNullOrWhiteSpace($serverDevToken)) {
        $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = $clientSessionToken
        $events += [pscustomobject]@{
            id = "server_token_aligned_from_client"
            severity = "info"
            message = "Using FUN_GAME_CLIENT_SESSION_TOKEN for local development server admission."
        }
    }
    elseif ([string]::IsNullOrWhiteSpace($clientSessionToken)) {
        $env:FUN_GAME_CLIENT_SESSION_TOKEN = $serverDevToken
        $events += [pscustomobject]@{
            id = "client_token_aligned_from_server"
            severity = "info"
            message = "Using FUN_GAME_SERVER_DEV_SESSION_TOKEN for local development client admission."
        }
    }
    elseif ($serverDevToken -cne $clientSessionToken) {
        $events += [pscustomobject]@{
            id = "dev_session_token_mismatch"
            severity = "warn"
            message = "FUN_GAME_SERVER_DEV_SESSION_TOKEN and FUN_GAME_CLIENT_SESSION_TOKEN differ; the local client may be rejected by the development server."
        }
    }

    return $events
}
