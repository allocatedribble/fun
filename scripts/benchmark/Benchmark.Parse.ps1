$ErrorActionPreference = "Stop"

function ConvertTo-MetricName {
    param([string]$Name)

    return ([regex]::Replace($Name.ToLowerInvariant(), "[^a-z0-9]+", "_")).Trim("_")
}

function Remove-AnsiEscape {
    param([string]$Text)

    if ([string]::IsNullOrEmpty($Text)) {
        return $Text
    }
    return [regex]::Replace($Text, "\x1B\[[0-9;]*m", "")
}

function Add-KeyValueMetrics {
    param(
        [System.Collections.IDictionary]$Sample,
        [string]$Payload,
        [string]$Prefix,
        [switch]$Milliseconds
    )

    $Payload = Remove-AnsiEscape -Text $Payload
    foreach ($match in [regex]::Matches($Payload, "([A-Za-z0-9_\/]+)=([^\s,]+)")) {
        $key = ConvertTo-MetricName $match.Groups[1].Value
        $value = $match.Groups[2].Value
        $metricName = "$Prefix$key"
        if ($Milliseconds -and -not $metricName.EndsWith("_ms")) {
            $metricName = $metricName + "_ms"
        }
        Add-Metric -Sample $Sample -Name $metricName -Text $value -Milliseconds:$Milliseconds
    }
}

function ConvertTo-KeyValueObject {
    param(
        [string]$Payload,
        [string]$HashKey = "hash"
    )

    $result = [ordered]@{}
    $Payload = Remove-AnsiEscape -Text $Payload
    foreach ($match in [regex]::Matches($Payload, "([A-Za-z0-9_\/]+)=([^\s,]+)")) {
        $key = ConvertTo-MetricName $match.Groups[1].Value
        if ($key -eq "hash") {
            $key = $HashKey
        }
        $result[$key] = $match.Groups[2].Value
    }
    return $result
}

function Parse-RenderCapabilitiesLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $capabilities = [regex]::Match($line, "\[bevy render\] capabilities: (?<payload>.*)$")
        if ($capabilities.Success) {
            $result = ConvertTo-KeyValueObject -Payload $capabilities.Groups["payload"].Value -HashKey "backend_capability_hash"
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        backend_capability_hash = $null
    }
}

function Parse-Dx12BackendDiagnosticsLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $diagnostics = [regex]::Match($line, "\[bevy render\] dx12 backend: (?<payload>.*)$")
        if ($diagnostics.Success) {
            $result = ConvertTo-KeyValueObject -Payload $diagnostics.Groups["payload"].Value
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        schema_version = $null
    }
}

function Parse-RenderFeatureGatesLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $featureGates = [regex]::Match($line, "\[fun render\] RT gates: (?<payload>.*)$")
        if ($featureGates.Success) {
            $result = ConvertTo-KeyValueObject -Payload $featureGates.Groups["payload"].Value -HashKey "rt_feature_hash"
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        rt_feature_hash = $null
    }
}

function Parse-RenderPresentationLog {
    param([string[]]$Lines)

    $result = [ordered]@{
        status = "not_found"
        startup_status = "not_found"
        surface_status = "not_found"
        backend = $null
        adapter = $null
        driver = $null
        present_mode = $null
        desired_maximum_frame_latency = $null
        vrr_detected = $null
        hdr_active = $null
        swapchain_format = $null
        window_mode = $null
        resolution_width = $null
        resolution_height = $null
        surface_requested_present_mode = $null
        surface_selected_present_mode = $null
        surface_available_present_modes = $null
    }

    foreach ($line in $Lines) {
        $presentation = [regex]::Match($line, "\[fun render\] presentation: (?<payload>.*)$")
        if ($presentation.Success) {
            $parsed = ConvertTo-KeyValueObject -Payload $presentation.Groups["payload"].Value -HashKey "presentation_hash"
            foreach ($property in $parsed.GetEnumerator()) {
                $result[$property.Key] = $property.Value
            }
            $result["startup_status"] = "found"
            $result["status"] = "found"
            continue
        }

        $surface = [regex]::Match($line, "\[bevy render\] surface present mode requested (?<requested>[^;]+); selected (?<selected>[^;]+); available (?<available>.+)$")
        if ($surface.Success) {
            $result["surface_requested_present_mode"] = $surface.Groups["requested"].Value.Trim()
            $result["surface_selected_present_mode"] = $surface.Groups["selected"].Value.Trim()
            $result["surface_available_present_modes"] = $surface.Groups["available"].Value.Trim()
            $result["surface_status"] = "found"
            $result["status"] = "found"
            continue
        }

        $surfaceConfig = [regex]::Match($line, "\[bevy render\] surface config: format (?<format>[^;]+); width (?<width>\d+); height (?<height>\d+); present_mode (?<present>[^;]+); desired_maximum_frame_latency (?<latency>\d+)")
        if ($surfaceConfig.Success) {
            $result["swapchain_format"] = $surfaceConfig.Groups["format"].Value.Trim()
            $result["resolution_width"] = $surfaceConfig.Groups["width"].Value.Trim()
            $result["resolution_height"] = $surfaceConfig.Groups["height"].Value.Trim()
            $result["surface_selected_present_mode"] = $surfaceConfig.Groups["present"].Value.Trim()
            $result["desired_maximum_frame_latency"] = $surfaceConfig.Groups["latency"].Value.Trim()
            $result["surface_status"] = "found"
            $result["status"] = "found"
        }
    }

    return $result
}

function Parse-RenderUploadCallsitesLog {
    param([string[]]$Lines)

    $callsites = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render upload top: rank=(?<rank>\d+) operation=(?<operation>\S+) label=(?<label>\S+) calls=(?<calls>\d+) bytes=(?<bytes>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$label"
        if (-not $callsites.Contains($key)) {
            $callsites[$key] = [ordered]@{
                operation = $operation
                label = $label
                calls = 0
                bytes = 0
                samples = 0
            }
        }
        $entry = $callsites[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.bytes = [uint64]$entry.bytes + [uint64]$match.Groups["bytes"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $callsites.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.bytes }; Descending = $true }, @{ Expression = { [uint64]$_.calls }; Descending = $true }, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    label = $_.label
                    calls = $_.calls
                    bytes = $_.bytes
                    samples = $_.samples
                }
            }
    )
}

function Parse-RenderChurnEventsLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render churn top: rank=(?<rank>\d+) operation=(?<operation>\S+) category=(?<category>\S+) label=(?<label>\S+) calls=(?<calls>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $category = $match.Groups["category"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$category`n$label"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                operation = $operation
                category = $category
                label = $label
                calls = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.calls }; Descending = $true }, operation, category, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    category = $_.category
                    label = $_.label
                    calls = $_.calls
                    samples = $_.samples
                }
            }
    )
}

function Parse-RenderChurnCreationEventsLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render churn creation top: rank=(?<rank>\d+) operation=(?<operation>\S+) category=(?<category>\S+) label=(?<label>\S+) calls=(?<calls>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $category = $match.Groups["category"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$category`n$label"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                operation = $operation
                category = $category
                label = $label
                calls = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.calls }; Descending = $true }, operation, category, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    category = $_.category
                    label = $_.label
                    calls = $_.calls
                    samples = $_.samples
                }
            }
    )
}

function Parse-RenderCommandEventsLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render command top: rank=(?<rank>\d+) operation=(?<operation>\S+) category=(?<category>\S+) label=(?<label>\S+) calls=(?<calls>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $category = $match.Groups["category"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$category`n$label"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                operation = $operation
                category = $category
                label = $label
                calls = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.calls }; Descending = $true }, operation, category, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    category = $_.category
                    label = $_.label
                    calls = $_.calls
                    samples = $_.samples
                }
            }
    )
}

function Parse-RenderReadbackEventsLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render readback top: rank=(?<rank>\d+) operation=(?<operation>\S+) category=(?<category>\S+) label=(?<label>\S+) calls=(?<calls>\d+) latency_frame_sum=(?<latency_frame_sum>\d+) latency_frame_max=(?<latency_frame_max>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $category = $match.Groups["category"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$category`n$label"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                operation = $operation
                category = $category
                label = $label
                calls = 0
                latency_frame_sum = 0
                latency_frame_max = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.latency_frame_sum = [uint64]$entry.latency_frame_sum + [uint64]$match.Groups["latency_frame_sum"].Value
        $entry.latency_frame_max = [Math]::Max([uint64]$entry.latency_frame_max, [uint64]$match.Groups["latency_frame_max"].Value)
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.calls }; Descending = $true }, @{ Expression = { [uint64]$_.latency_frame_sum }; Descending = $true }, operation, category, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    category = $_.category
                    label = $_.label
                    calls = $_.calls
                    latency_frame_sum = $_.latency_frame_sum
                    latency_frame_max = $_.latency_frame_max
                    samples = $_.samples
                }
            }
    )
}

function Parse-RenderShaderEventsLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render shader top: rank=(?<rank>\d+) operation=(?<operation>\S+) category=(?<category>\S+) label=(?<label>\S+) calls=(?<calls>\d+) elapsed_ns=(?<elapsed_ns>\d+) shader_defs=(?<shader_defs>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $category = $match.Groups["category"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$category`n$label"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                operation = $operation
                category = $category
                label = $label
                calls = 0
                elapsed_ns = 0
                shader_defs = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.elapsed_ns = [uint64]$entry.elapsed_ns + [uint64]$match.Groups["elapsed_ns"].Value
        $entry.shader_defs = [uint64]$entry.shader_defs + [uint64]$match.Groups["shader_defs"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.elapsed_ns }; Descending = $true }, @{ Expression = { [uint64]$_.calls }; Descending = $true }, operation, category, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    category = $_.category
                    label = $_.label
                    calls = $_.calls
                    elapsed_ns = $_.elapsed_ns
                    shader_defs = $_.shader_defs
                    samples = $_.samples
                }
            }
    )
}

function Parse-TransientDescriptorCreateLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        if (-not $line.Contains("transient descriptor create top")) {
            continue
        }
        $parsed = ConvertTo-KeyValueObject -Payload $line
        $resource = [string]$parsed.resource
        $label = [string]$parsed.label
        $reason = [string]$parsed.reason
        $nearMiss = [string]$parsed.near_miss
        $createPattern = [string]$parsed.create_pattern
        $format = [string]$parsed.format
        $width = [string]$parsed.width
        $height = [string]$parsed.height
        $size = [string]$parsed.size
        $usageBits = [string]$parsed.usage_bits
        $key = "$resource`n$label`n$reason`n$nearMiss`n$createPattern`n$format`n$width`n$height`n$size`n$usageBits"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                resource = $resource
                label = $label
                reason = $reason
                near_miss = $nearMiss
                create_pattern = $createPattern
                format = $format
                width = $width
                height = $height
                size = $size
                usage_bits = $usageBits
                create_count = 0
                estimated_bytes = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.create_count = [uint64]$entry.create_count + [uint64]$parsed.create_count
        $entry.estimated_bytes = [uint64]$entry.estimated_bytes + [uint64]$parsed.estimated_bytes
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.create_count }; Descending = $true }, resource, label, reason, near_miss |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    resource = $_.resource
                    label = $_.label
                    reason = $_.reason
                    near_miss = $_.near_miss
                    create_pattern = $_.create_pattern
                    format = $_.format
                    width = $_.width
                    height = $_.height
                    size = $_.size
                    usage_bits = $_.usage_bits
                    create_count = $_.create_count
                    estimated_bytes = $_.estimated_bytes
                    samples = $_.samples
                }
            }
    )
}

function Parse-TransientDescriptorLabelVariantLog {
    param([string[]]$Lines)

    $events = [ordered]@{}
    foreach ($line in $Lines) {
        if (-not $line.Contains("transient descriptor label variants")) {
            continue
        }
        $parsed = ConvertTo-KeyValueObject -Payload $line
        $resource = [string]$parsed.resource
        $labels = [string]$parsed.labels
        $format = [string]$parsed.format
        $width = [string]$parsed.width
        $height = [string]$parsed.height
        $size = [string]$parsed.size
        $usageBits = [string]$parsed.usage_bits
        $key = "$resource`n$labels`n$format`n$width`n$height`n$size`n$usageBits"
        if (-not $events.Contains($key)) {
            $events[$key] = [ordered]@{
                resource = $resource
                labels = $labels
                format = $format
                width = $width
                height = $height
                size = $size
                usage_bits = $usageBits
                label_count = 0
                samples = 0
            }
        }
        $entry = $events[$key]
        $entry.label_count = [uint64][Math]::Max([uint64]$entry.label_count, [uint64]$parsed.label_count)
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $events.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.label_count }; Descending = $true }, resource, labels |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    resource = $_.resource
                    labels = $_.labels
                    format = $_.format
                    width = $_.width
                    height = $_.height
                    size = $_.size
                    usage_bits = $_.usage_bits
                    label_count = $_.label_count
                    samples = $_.samples
                }
            }
    )
}

function Parse-CefUiTransportSelectionLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $selection = [regex]::Match($line, "\[client perf\] cef_ui transport selected: (?<payload>.*)$")
        if ($selection.Success) {
            $result = ConvertTo-KeyValueObject -Payload $selection.Groups["payload"].Value -HashKey "transport_hash"
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        requested = $null
        selected = $null
        backend = $null
        bridge_ready = $null
        cpu_fallback_enabled = $null
        ring_depth = $null
        copy_mode = $null
        strict = $null
        debug_timings = $null
        fallback_reason = $null
    }
}

function Parse-CefUiTransportHealthLog {
    param([string[]]$Lines)

    $last = $null
    foreach ($line in $Lines) {
        $health = [regex]::Match($line, "\[client perf\] cef_ui transport health: (?<payload>.*)$")
        if ($health.Success) {
            $last = ConvertTo-KeyValueObject -Payload $health.Groups["payload"].Value -HashKey "transport_health_hash"
            $last["status"] = if ($last.Contains("status")) { $last["status"] } else { "found" }
            $last["record_status"] = "found"
        }
    }

    if ($null -ne $last) {
        return $last
    }

    return [ordered]@{
        record_status = "not_found"
        transport = $null
        status = $null
        accel_paint_fps = $null
        paint_fps = $null
        gpu_copy_ms = $null
        gpu_copy_ns_per_copy = $null
        cpu_upload_bytes_per_frame = $null
        reused_frames = $null
        not_ready_frames = $null
        blocking_waits = $null
        fallback_count = $null
        ring_depth = $null
    }
}

function Parse-ClientPerfLog {
    param([string[]]$Lines)

    $samples = New-Object "System.Collections.Generic.List[object]"
    $current = $null
    $pendingCefUiMetrics = $null
    $pendingCefUiHealthMetrics = $null

    foreach ($line in $Lines) {
        $main = [regex]::Match($line, "\[client perf\] fps=(?<fps>\S+) frame_ms=(?<frame_ms>\S+) solari_gpu_ms=(?<solari>\S+) meshlet_visibility_gpu_ms=(?<meshlet>\S+) dlss_rr_gpu_ms=(?<dlss>\S+)")
        if ($main.Success) {
            $current = [ordered]@{}
            Add-Metric -Sample $current -Name "fps" -Text $main.Groups["fps"].Value
            Add-Metric -Sample $current -Name "frame_ms" -Text $main.Groups["frame_ms"].Value -Milliseconds
            Add-Metric -Sample $current -Name "solari_gpu_ms" -Text $main.Groups["solari"].Value -Milliseconds
            Add-Metric -Sample $current -Name "meshlet_visibility_gpu_ms" -Text $main.Groups["meshlet"].Value -Milliseconds
            Add-Metric -Sample $current -Name "dlss_rr_gpu_ms" -Text $main.Groups["dlss"].Value -Milliseconds
            if ($null -ne $pendingCefUiMetrics) {
                foreach ($key in $pendingCefUiMetrics.Keys) {
                    $current[$key] = $pendingCefUiMetrics[$key]
                }
                $pendingCefUiMetrics = $null
            }
            if ($null -ne $pendingCefUiHealthMetrics) {
                foreach ($key in $pendingCefUiHealthMetrics.Keys) {
                    $current[$key] = $pendingCefUiHealthMetrics[$key]
                }
                $pendingCefUiHealthMetrics = $null
            }
            $samples.Add($current) | Out-Null
            continue
        }

        $cpu = [regex]::Match($line, "\[client perf\] process_cpu_pct=(?<process_cpu>\S+) process_mem_gib=(?<process_mem>\S+) system_cpu_pct=(?<system_cpu>\S+) system_mem_pct=(?<system_mem>\S+)")
        if ($cpu.Success) {
            Add-Metric -Sample $current -Name "process_cpu_pct" -Text $cpu.Groups["process_cpu"].Value
            Add-Metric -Sample $current -Name "process_mem_gib" -Text $cpu.Groups["process_mem"].Value
            Add-Metric -Sample $current -Name "system_cpu_pct" -Text $cpu.Groups["system_cpu"].Value
            Add-Metric -Sample $current -Name "system_mem_pct" -Text $cpu.Groups["system_mem"].Value
            continue
        }

        $passes = [regex]::Match($line, "\[client perf\] solari passes gpu_ms: (?<payload>.*)$")
        if ($passes.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $passes.Groups["payload"].Value -Prefix "solari_pass_" -Milliseconds
            continue
        }

        $budget = [regex]::Match($line, "\[client perf\] solari budget: (?<payload>.*)$")
        if ($budget.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $budget.Groups["payload"].Value -Prefix "solari_budget_"
            continue
        }

        $top = [regex]::Match($line, "\[client perf\] top render timings (?<payload>.*)$")
        if ($top.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $top.Groups["payload"].Value -Prefix "top_render_" -Milliseconds
            continue
        }

        $nonSolariGpu = [regex]::Match($line, "\[client perf\] non_solari gpu_ms: (?<payload>.*)$")
        if ($nonSolariGpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $nonSolariGpu.Groups["payload"].Value -Prefix "" -Milliseconds
            continue
        }

        $cloudGpu = [regex]::Match($line, "\[client perf\] clouds gpu_ms: (?<payload>.*)$")
        if ($cloudGpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudGpu.Groups["payload"].Value -Prefix "cloud_" -Milliseconds
            continue
        }

        $cloudCpu = [regex]::Match($line, "\[client perf\] clouds cpu_ns: (?<payload>.*)$")
        if ($cloudCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudCpu.Groups["payload"].Value -Prefix "cloud_"
            continue
        }

        $cloudState = [regex]::Match($line, "\[client perf\] clouds state: (?<payload>.*)$")
        if ($cloudState.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudState.Groups["payload"].Value -Prefix "cloud_"
            continue
        }

        $nonSolariCpu = [regex]::Match($line, "\[client perf\] non_solari cpu_ns: (?<payload>.*)$")
        if ($nonSolariCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $nonSolariCpu.Groups["payload"].Value -Prefix ""
            continue
        }

        $renderPaths = [regex]::Match($line, "\[client perf\] render paths: (?<payload>.*)$")
        if ($renderPaths.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderPaths.Groups["payload"].Value -Prefix ""
            continue
        }

        $meshletBuffers = [regex]::Match($line, "\[client perf\] meshlet buffers: (?<payload>.*)$")
        if ($meshletBuffers.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $meshletBuffers.Groups["payload"].Value -Prefix "meshlet_"
            continue
        }

        $renderUploads = [regex]::Match($line, "\[client perf\] render uploads: (?<payload>.*)$")
        if ($renderUploads.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderUploads.Groups["payload"].Value -Prefix "render_upload_"
            continue
        }

        $renderChurn = [regex]::Match($line, "\[client perf\] render churn: (?<payload>.*)$")
        if ($renderChurn.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderChurn.Groups["payload"].Value -Prefix "render_churn_"
            continue
        }

        $renderCommands = [regex]::Match($line, "\[client perf\] render commands: (?<payload>.*)$")
        if ($renderCommands.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderCommands.Groups["payload"].Value -Prefix "render_command_"
            continue
        }

        $renderReadbacks = [regex]::Match($line, "\[client perf\] render readbacks: (?<payload>.*)$")
        if ($renderReadbacks.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderReadbacks.Groups["payload"].Value -Prefix "render_readback_"
            continue
        }

        $renderShaders = [regex]::Match($line, "\[client perf\] render shaders: (?<payload>.*)$")
        if ($renderShaders.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderShaders.Groups["payload"].Value -Prefix "render_shader_"
            continue
        }

        $radianceCache = [regex]::Match($line, "\[client perf\] radiance cache: (?<payload>.*)$")
        if ($radianceCache.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $radianceCache.Groups["payload"].Value -Prefix "radiance_cache_"
            continue
        }

        if ($line.Contains("transient render resource arena frame")) {
            Add-KeyValueMetrics -Sample $current -Payload $line -Prefix "transient_"
            continue
        }

        if ($line.Contains("render graph budget pressure")) {
            Add-KeyValueMetrics -Sample $current -Payload $line -Prefix "render_scheduler_"
            continue
        }

        $scheduleCpu = [regex]::Match($line, "\[client perf\] schedule cpu_ns: (?<payload>.*)$")
        if ($scheduleCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $scheduleCpu.Groups["payload"].Value -Prefix "schedule_"
            continue
        }

        $scheduleDetail = [regex]::Match($line, "\[client perf\] schedule detail: (?<payload>.*)$")
        if ($scheduleDetail.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $scheduleDetail.Groups["payload"].Value -Prefix "schedule_"
            continue
        }

        $cefUi = [regex]::Match($line, "\[client perf\] cef_ui transport: (?<payload>.*)$")
        if ($cefUi.Success) {
            if ($null -eq $current) {
                $pendingCefUiMetrics = [ordered]@{}
                Add-KeyValueMetrics -Sample $pendingCefUiMetrics -Payload $cefUi.Groups["payload"].Value -Prefix ""
            }
            else {
                Add-KeyValueMetrics -Sample $current -Payload $cefUi.Groups["payload"].Value -Prefix ""
            }
            continue
        }

        $cefUiHealth = [regex]::Match($line, "\[client perf\] cef_ui transport health: (?<payload>.*)$")
        if ($cefUiHealth.Success) {
            if ($null -eq $current) {
                $pendingCefUiHealthMetrics = [ordered]@{}
                Add-KeyValueMetrics -Sample $pendingCefUiHealthMetrics -Payload $cefUiHealth.Groups["payload"].Value -Prefix "cef_health_"
            }
            else {
                Add-KeyValueMetrics -Sample $current -Payload $cefUiHealth.Groups["payload"].Value -Prefix "cef_health_"
            }
            continue
        }
    }

    return $samples
}
