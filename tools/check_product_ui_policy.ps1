param(
    [switch]$SelfTest,
    [string]$Root,
    [string]$EmitArtifact
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Split-Path -Parent $PSScriptRoot
}

$Root = [System.IO.Path]::GetFullPath($Root)

$productRoots = @(
    "game_client\src",
    "fun_render\src",
    "fun-renderer\src",
    "fun_host\src"
)

$forbiddenRules = @(
    @{
        Rule = "product-bevy-ui-crate"
        Pattern = "\bbevy_ui\b|\bbevy\s*::\s*ui\b"
        Detail = "Product lanes must not depend on Bevy UI."
    },
    @{
        Rule = "product-bevy-fps-overlay"
        Pattern = "\bbevy\s*::\s*dev_tools\s*::\s*fps_overlay\b|\bFpsOverlayPlugin\b"
        Detail = "Runtime FPS/debug overlays must be CEF/Svelte or renderer diagnostics, not Bevy UI."
    },
    @{
        Rule = "product-bevy-ui-component"
        Pattern = "\bImageNode\b|\bUiImage\b|\bNodeBundle\b|\bTextBundle\b|\bButtonBundle\b"
        Detail = "Product UI components must not be rendered through Bevy UI."
    },
    @{
        Rule = "cef-cpu-upload-product-path"
        Pattern = "\btracked_write_texture\s*\(|\bcopy_ready_frame_to_bevy_image\b|\bDx12CefBevyImageState\b"
        Detail = "CEF product pixels must not flow through CPU texture upload or Bevy image copy paths."
    },
    @{
        Rule = "cef-cpu-onpaint-selected"
        Pattern = "with_paint_transport_decision\s*\(\s*CefUiPaintTransport::CpuPaint"
        Detail = "Product CEF fallback must fail closed instead of selecting CPU OnPaint."
    }
)

function Convert-ToRelativePath {
    param([string]$Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($fullPath.StartsWith($Root, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $fullPath.Substring($Root.Length).TrimStart("\", "/")
    }

    return $fullPath
}

function Test-PolicyLineAllowed {
    param(
        [string]$RelativePath,
        [string]$Line
    )

    $normalized = $RelativePath -replace "/", "\"

    if ($normalized -eq "fun-renderer\src\lib.rs" -and
        $Line -match "bevy_ui_(runtime_product_allowed|test_only_allowed)|product_ui_policy_prohibits_runtime_bevy_ui") {
        return $true
    }

    if ($normalized -eq "fun-renderer\src\ui\cef.rs" -and
        $Line -match "CpuOnPaint|cpu_on_paint") {
        return $true
    }

    return $false
}

function Find-ProductUiPolicyViolations {
    param(
        [string]$RelativePath,
        [string[]]$Lines
    )

    $violations = New-Object System.Collections.Generic.List[object]

    for ($index = 0; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        $trimmed = $line.TrimStart()

        if ($trimmed.StartsWith("//")) {
            continue
        }

        if (Test-PolicyLineAllowed -RelativePath $RelativePath -Line $line) {
            continue
        }

        foreach ($rule in $forbiddenRules) {
            if ($line -match $rule.Pattern) {
                $violations.Add([pscustomobject]@{
                    Path = $RelativePath
                    Line = $index + 1
                    Rule = $rule.Rule
                    Detail = $rule.Detail
                    Text = $line.Trim()
                })
            }
        }
    }

    return $violations
}

function Invoke-SelfTest {
    $cases = @(
        @{
            Name = "reject Bevy ImageNode"
            Path = "game_client\src\bad.rs"
            Lines = @("commands.spawn(ImageNode::default());")
            ShouldFail = $true
        },
        @{
            Name = "reject Bevy FPS overlay"
            Path = "fun_render\src\bad.rs"
            Lines = @("app.add_plugins(FpsOverlayPlugin::default());")
            ShouldFail = $true
        },
        @{
            Name = "reject CPU OnPaint selection"
            Path = "game_client\src\bad.rs"
            Lines = @("config.with_paint_transport_decision(CefUiPaintTransport::CpuPaint, reason);")
            ShouldFail = $true
        },
        @{
            Name = "reject CEF Bevy image copy bridge"
            Path = "game_client\src\bad.rs"
            Lines = @("interop.copy_ready_frame_to_bevy_image(token, image, state);")
            ShouldFail = $true
        },
        @{
            Name = "allow renderer CEF CPU rejection enum"
            Path = "fun-renderer\src\ui\cef.rs"
            Lines = @("CpuOnPaintRuntimeFallback,")
            ShouldFail = $false
        },
        @{
            Name = "allow fun-renderer policy constants"
            Path = "fun-renderer\src\lib.rs"
            Lines = @("pub bevy_ui_runtime_product_allowed: bool,")
            ShouldFail = $false
        }
    )

    foreach ($case in $cases) {
        $violations = @(Find-ProductUiPolicyViolations -RelativePath $case.Path -Lines $case.Lines)
        $failed = $violations.Count -gt 0
        if ($failed -ne $case.ShouldFail) {
            throw "Self-test '$($case.Name)' expected ShouldFail=$($case.ShouldFail) but saw $($violations.Count) violations."
        }
    }

    Write-Host "product UI policy checker self-test passed"
}

function Get-ScanFiles {
    $files = New-Object System.Collections.Generic.List[System.IO.FileInfo]

    foreach ($relativeRoot in $productRoots) {
        $absoluteRoot = Join-Path $Root $relativeRoot
        if (-not (Test-Path -LiteralPath $absoluteRoot)) {
            continue
        }
        Get-ChildItem -LiteralPath $absoluteRoot -Recurse -File |
            Where-Object {
                $_.Extension -eq ".rs" -or $_.Name -eq "Cargo.toml"
            } |
            ForEach-Object {
                $files.Add($_)
            }
    }

    return $files
}

function Invoke-ProductUiPolicyCheck {
    $violations = New-Object System.Collections.Generic.List[object]
    $files = @(Get-ScanFiles)

    foreach ($file in $files) {
        $relativePath = Convert-ToRelativePath -Path $file.FullName
        $lines = @(Get-Content -LiteralPath $file.FullName)
        $fileViolations = @(Find-ProductUiPolicyViolations -RelativePath $relativePath -Lines $lines)
        foreach ($violation in $fileViolations) {
            $violations.Add($violation)
        }
    }

    $status = if ($violations.Count -eq 0) { "pass" } else { "fail" }
    $checks = foreach ($rule in $forbiddenRules) {
        [pscustomobject]@{
            rule = $rule.Rule
            detail = $rule.Detail
        }
    }
    $artifact = [pscustomobject]@{
        schema = "fun.product_ui_policy.v1"
        status = $status
        checked_at_utc = ([System.DateTimeOffset]::UtcNow).ToString("o")
        root = $Root
        scanned_files = $files.Count
        violation_count = $violations.Count
        checks = @($checks)
        violations = @($violations.ToArray())
    }

    if (-not [string]::IsNullOrWhiteSpace($EmitArtifact)) {
        $artifactPath = if ([System.IO.Path]::IsPathRooted($EmitArtifact)) {
            $EmitArtifact
        } else {
            Join-Path $Root $EmitArtifact
        }
        $artifactPath = [System.IO.Path]::GetFullPath($artifactPath)
        $parent = Split-Path -Parent $artifactPath
        if (-not [string]::IsNullOrWhiteSpace($parent)) {
            New-Item -ItemType Directory -Force -Path $parent | Out-Null
        }
        $artifact | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $artifactPath -Encoding UTF8
    }

    if ($violations.Count -gt 0) {
        $violations |
            Sort-Object Path, Line, Rule |
            Format-Table -AutoSize Path, Line, Rule, Text
        throw "Product UI policy check failed with $($violations.Count) violation(s)."
    }

    Write-Host "product UI policy check passed: $($files.Count) file(s) scanned"
}

if ($SelfTest) {
    Invoke-SelfTest
    return
}

Invoke-ProductUiPolicyCheck
