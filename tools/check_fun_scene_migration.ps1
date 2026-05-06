param(
    [switch]$SelfTest,
    [string]$Root
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Split-Path -Parent $PSScriptRoot
}

$Root = [System.IO.Path]::GetFullPath($Root)

$directImportPatterns = @(
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?\{[^}]*\bbsn\b",
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?\{[^}]*\bbsn_list\b",
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?bsn\b",
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?bsn_list\b"
)

$directMacroPatterns = @(
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?bsn\s*!",
    "\bbevy\s*::\s*scene\s*::\s*(?:prelude\s*::\s*)?bsn_list\s*!",
    "\bbevy_scene\s*::\s*bsn\s*!",
    "\bbevy_scene\s*::\s*bsn_list\s*!"
)

$gameScenePlainMacroPatterns = @(
    "\bbsn\s*!",
    "\bbsn_list\s*!"
)

function Convert-ToRelativePath {
    param([string]$Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($fullPath.StartsWith($Root, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $fullPath.Substring($Root.Length).TrimStart("\", "/")
    }

    return $fullPath
}

function Test-BridgePath {
    param([string]$RelativePath)

    $normalized = $RelativePath -replace "/", "\"
    return $normalized -eq "fun-scene-macros\src\fun_macro.rs" -or
        $normalized -eq "fun-scene-macros\src\fun_list_macro.rs" -or
        $normalized -eq "fun-scene-macros\src\lib.rs"
}

function Test-GameScenePath {
    param([string]$RelativePath)

    $normalized = $RelativePath -replace "/", "\"
    return $normalized.StartsWith("game_scene\src\", [System.StringComparison]::OrdinalIgnoreCase)
}

function Find-FunSceneMigrationViolations {
    param(
        [string]$RelativePath,
        [string[]]$Lines
    )

    $violations = New-Object System.Collections.Generic.List[object]
    $allowBridgeMacro = Test-BridgePath -RelativePath $RelativePath
    $isGameScene = Test-GameScenePath -RelativePath $RelativePath

    for ($index = 0; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        $trimmed = $line.TrimStart()

        if ($trimmed.StartsWith("//")) {
            continue
        }

        foreach ($pattern in $directImportPatterns) {
            if ($line -match $pattern) {
                $violations.Add([pscustomobject]@{
                    Path = $RelativePath
                    Line = $index + 1
                    Rule = "direct-bevy-scene-bsn-import"
                    Text = $line.Trim()
                })
            }
        }

        if (-not $allowBridgeMacro) {
            foreach ($pattern in $directMacroPatterns) {
                if ($line -match $pattern) {
                    $violations.Add([pscustomobject]@{
                        Path = $RelativePath
                        Line = $index + 1
                        Rule = "direct-bevy-scene-bsn-macro"
                        Text = $line.Trim()
                    })
                }
            }
        }

        if ($isGameScene) {
            foreach ($pattern in $gameScenePlainMacroPatterns) {
                if ($line -match $pattern) {
                    $violations.Add([pscustomobject]@{
                        Path = $RelativePath
                        Line = $index + 1
                        Rule = "game-scene-plain-bsn-macro"
                        Text = $line.Trim()
                    })
                }
            }
        }
    }

    return $violations
}

function Invoke-SelfTest {
    $cases = @(
        @{
            Name = "reject grouped bevy scene import"
            Path = "game_scene\src\bad.rs"
            Lines = @("use bevy::scene::{bsn, bsn_list};")
            ShouldFail = $true
        },
        @{
            Name = "reject bevy scene prelude import"
            Path = "game_scene\src\bad.rs"
            Lines = @("use bevy::scene::prelude::{CommandsSceneExt, bsn};")
            ShouldFail = $true
        },
        @{
            Name = "reject direct bevy scene macro path"
            Path = "game_scene\src\bad.rs"
            Lines = @("let scene = bevy::scene::bsn! { Transform::default() };")
            ShouldFail = $true
        },
        @{
            Name = "reject plain bsn in game_scene"
            Path = "game_scene\src\bad.rs"
            Lines = @("let scene = bsn! { Transform::default() };")
            ShouldFail = $true
        },
        @{
            Name = "allow fun_scene prelude and fun macro"
            Path = "game_scene\src\good.rs"
            Lines = @(
                "use fun_scene::prelude::*;",
                "let scene = fun! { Transform::default() };"
            )
            ShouldFail = $false
        },
        @{
            Name = "allow macro bridge wrapper"
            Path = "fun-scene-macros\src\fun_macro.rs"
            Lines = @('pub(crate) const TARGET: &str = "::fun_scene::bevy_scene::bsn!";')
            ShouldFail = $false
        }
    )

    foreach ($case in $cases) {
        $violations = @(Find-FunSceneMigrationViolations -RelativePath $case["Path"] -Lines $case["Lines"])
        $failed = $violations.Count -gt 0
        if ($failed -ne $case["ShouldFail"]) {
            Write-Error "self-test failed: $($case["Name"])"
        }
    }

    Write-Host "fun-scene migration self-test passed"
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

$files = Get-ChildItem -Path $Root -Recurse -File -Filter "*.rs" |
    Where-Object {
        $relative = Convert-ToRelativePath -Path $_.FullName
        $normalized = $relative -replace "/", "\"
        -not $normalized.StartsWith("target\", [System.StringComparison]::OrdinalIgnoreCase)
    }

$allViolations = New-Object System.Collections.Generic.List[object]

foreach ($file in $files) {
    $relative = Convert-ToRelativePath -Path $file.FullName
    $lines = Get-Content -LiteralPath $file.FullName
    $violations = Find-FunSceneMigrationViolations -RelativePath $relative -Lines $lines
    foreach ($violation in $violations) {
        $allViolations.Add($violation)
    }
}

if ($allViolations.Count -gt 0) {
    Write-Host "fun-scene migration check failed:"
    foreach ($violation in $allViolations) {
        Write-Host ("{0}:{1} [{2}] {3}" -f $violation.Path, $violation.Line, $violation.Rule, $violation.Text)
    }
    exit 1
}

Write-Host "fun-scene migration check passed"
