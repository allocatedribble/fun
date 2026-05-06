function Normalize-StackPath {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $Path
    }
    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    if ($Path.StartsWith("\?\")) {
        return $Path.Substring(3)
    }
    return $Path
}

function Resolve-StackPath {
    param(
        [string]$BasePath,
        [string]$Path
    )

    $candidate = if ([System.IO.Path]::IsPathRooted($Path)) {
        $Path
    }
    else {
        Join-Path $BasePath $Path
    }
    return Normalize-StackPath ((Resolve-Path -LiteralPath $candidate).Path)
}

function New-StackPaths {
    param(
        [string]$ScriptRoot,
        [bool]$Release
    )

    $repoRoot = Resolve-StackPath -BasePath $ScriptRoot -Path ".."
    $profile = if ($Release) { "release" } else { "debug" }
    $targetRoot = Join-Path $repoRoot "target"
    $profileTargetDir = Join-Path $targetRoot $profile
    $runRoot = Join-Path $targetRoot "run-stack"
    $logRoot = Join-Path $runRoot "logs"
    $rustSysroot = (& rustc --print sysroot).Trim()
    $rustTargetLibDir = (& rustc --print target-libdir).Trim()

    [pscustomobject]@{
        repo_root = $repoRoot
        script_root = (Normalize-StackPath $ScriptRoot)
        stack_root = Join-Path $ScriptRoot "stack"
        profile = $profile
        target_root = $targetRoot
        profile_target_dir = $profileTargetDir
        run_root = $runRoot
        log_root = $logRoot
        pid_file = Join-Path $runRoot "processes.json"
        session_file = Join-Path $runRoot "session.json"
        cef_transport_status_file = Join-Path $runRoot "cef-ui-transport.json"
        renderer_capability_report_file = Join-Path $runRoot "renderer-capabilities.json"
        rust_sysroot = $rustSysroot
        rust_toolchain_bin = Join-Path $rustSysroot "bin"
        rust_target_lib_dir = $rustTargetLibDir
    }
}

function Initialize-StackDirectories {
    param([pscustomobject]$Paths)

    New-Item -ItemType Directory -Force -Path $Paths.log_root | Out-Null
}

function Get-StackBinaryPath {
    param(
        [pscustomobject]$Paths,
        [string]$Name
    )

    $extension = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
    return Join-Path $Paths.profile_target_dir "$Name$extension"
}
