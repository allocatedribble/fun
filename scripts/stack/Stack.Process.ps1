function Stop-PreviousStackProcesses {
    param([string]$PidFile)

    if (-not (Test-Path $PidFile)) {
        return
    }
    try {
        $existing = Get-Content -Path $PidFile | ConvertFrom-Json
        foreach ($entry in $existing) {
            $process = Get-Process -Id $entry.pid -ErrorAction SilentlyContinue
            if ($null -ne $process) {
                Write-Host "Stopping previous $($entry.name) pid=$($entry.pid)..."
                Stop-Process -Id $entry.pid -Force
            }
        }
    }
    catch {
        Write-Warning "Could not clean up previous stack metadata: $_"
    }
}

function Start-FunProcess {
    param(
        [pscustomobject]$Paths,
        [string]$Name,
        [switch]$Visible
    )

    $binaryPath = Get-StackBinaryPath -Paths $Paths -Name $Name
    if (-not (Test-Path $binaryPath)) {
        throw "Expected binary was not built: $binaryPath"
    }

    $stdout = Join-Path $Paths.log_root "$Name.out.log"
    $stderr = Join-Path $Paths.log_root "$Name.err.log"

    $startArgs = @{
        FilePath = $binaryPath
        WorkingDirectory = $Paths.repo_root
        PassThru = $true
        RedirectStandardOutput = $stdout
        RedirectStandardError = $stderr
    }

    if (-not $Visible) {
        $startArgs.WindowStyle = "Hidden"
    }

    $process = Start-Process @startArgs

    [pscustomobject]@{
        name = $Name
        pid = $process.Id
        path = $binaryPath
        stdout = $stdout
        stderr = $stderr
        visible = [bool]$Visible
    }
}

function Start-StackProcesses {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Paths
    )

    $started = @()
    if ($Request.Server) {
        $started += Start-FunProcess -Paths $Paths -Name "game_server"
    }

    if (-not $Request.NoClient) {
        Start-Sleep -Seconds $Request.StartupDelaySeconds
        $started += Start-FunProcess -Paths $Paths -Name "game_client" -Visible
    }

    $started | ConvertTo-Json -Depth 3 | Set-Content -Path $Paths.pid_file
    return $started
}

function Confirm-StackProcesses {
    param([object[]]$Processes)

    Start-Sleep -Milliseconds 750
    foreach ($entry in $Processes) {
        $process = Get-Process -Id $entry.pid -ErrorAction SilentlyContinue
        if ($null -eq $process) {
            Write-Warning "$($entry.name) exited immediately. Check $($entry.stderr) and $($entry.stdout)."
        }
        else {
            Write-Host "Started $($entry.name) pid=$($entry.pid)"
        }
    }
}
