$ErrorActionPreference = "Stop"

function Normalize-WorkspacePath {
    param([string]$Path)

    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    if ($Path.StartsWith("\?\")) {
        return $Path.Substring(3)
    }
    return $Path
}

function ConvertTo-FunBenchFlag {
    param([string]$Name)

    $withBreaks = [regex]::Replace($Name, "([a-z0-9])([A-Z])", '$1-$2')
    return $withBreaks.Replace("_", "-").ToLowerInvariant()
}

function ConvertTo-FunBenchValue {
    param(
        [string]$Name,
        [object]$Value
    )

    $text = [string]$Value
    if ($Name -in @("PresentMode", "MatrixSize")) {
        return $text.Replace("_", "-")
    }
    return $text
}

function Invoke-FunBenchCompat {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Subcommand,
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$BoundParameters,
        [Parameter(Mandatory = $true)]
        [string[]]$ActualParameters,
        [hashtable]$FlagAliases = @{}
    )

    $scriptRoot = $PSScriptRoot
    $funRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
    $projectRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $funRoot "..")).Path)
    $manifest = Join-Path $projectRoot "fun-cli\Cargo.toml"
    if (-not (Test-Path -LiteralPath $manifest)) {
        throw "fun-cli manifest was not found: $manifest"
    }

    $actual = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($name in $ActualParameters) {
        [void]$actual.Add($name)
    }

    $args = @(
        "run",
        "--manifest-path",
        $manifest,
        "-p",
        "fun-bench",
        "--",
        $Subcommand,
        "--fun-root",
        $funRoot
    )

    foreach ($entry in @($BoundParameters.GetEnumerator() | Sort-Object Name)) {
        $name = [string]$entry.Key
        $value = $entry.Value
        if ($null -eq $value) {
            continue
        }

        $args += @("--legacy-arg", "$name=$value")
        if (-not $actual.Contains($name)) {
            continue
        }

        $flag = if ($FlagAliases.ContainsKey($name)) {
            [string]$FlagAliases[$name]
        }
        else {
            ConvertTo-FunBenchFlag -Name $name
        }

        if ($value -is [System.Management.Automation.SwitchParameter] -or $value -is [bool]) {
            if ([bool]$value) {
                $args += "--$flag"
            }
            continue
        }

        if ($value -is [array]) {
            foreach ($item in $value) {
                if ($null -ne $item -and -not [string]::IsNullOrWhiteSpace([string]$item)) {
                    $args += @("--$flag", (ConvertTo-FunBenchValue -Name $name -Value $item))
                }
            }
            continue
        }

        if (-not [string]::IsNullOrWhiteSpace([string]$value)) {
            $args += @("--$flag", (ConvertTo-FunBenchValue -Name $name -Value $value))
        }
    }

    & cargo @args
    exit $LASTEXITCODE
}
