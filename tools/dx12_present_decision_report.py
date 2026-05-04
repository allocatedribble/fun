#!/usr/bin/env python3
"""Generate a present-mode/frame-latency decision report from a parity matrix."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


EXPECTED_SCENARIOS: tuple[str, ...] = (
    "ui_hidden",
    "ui_accelerated",
    "representative_gameplay",
    "solari_cloud_heavy",
)
REQUIRED_PRESENT_MODES: tuple[str, ...] = ("immediate", "auto_no_vsync", "fifo")
OPTIONAL_PRESENT_MODES: tuple[str, ...] = ("auto_vsync",)
REQUIRED_FRAME_LATENCIES: tuple[int, ...] = (1, 2, 3, 4)
REQUIRED_BACKENDS: tuple[str, ...] = ("dx12", "vulkan")
CURRENT_DEFAULT = {"present_mode": "immediate", "max_frame_latency": 0}
THROUGHPUT_TOLERANCE = 0.95


def number(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        parsed = float(value)
    else:
        try:
            parsed = float(str(value))
        except ValueError:
            return None
    if math.isnan(parsed) or math.isinf(parsed):
        return None
    return parsed


def metric_value(lane: dict[str, Any], metric: str, field: str) -> float | None:
    metrics = lane.get("key_metrics")
    if not isinstance(metrics, dict):
        return None
    entry = metrics.get(metric)
    if not isinstance(entry, dict):
        return None
    return number(entry.get(field))


def scenario_name(lane: dict[str, Any]) -> str:
    raw = str(lane.get("name", ""))
    match = re.search(r"_fl[0-9]+_(?P<scenario>.+)$", raw)
    if match:
        return match.group("scenario")
    mode = str(lane.get("cef_ui_mode", ""))
    transport = str(lane.get("cef_paint_transport", ""))
    if mode == "hidden":
        return "ui_hidden"
    if mode == "animated" and transport == "d3d11on12":
        return "ui_accelerated"
    return str(lane.get("benchmark_lane") or "unknown")


def present_lanes(matrix: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        lane
        for lane in matrix.get("lanes", [])
        if isinstance(lane, dict) and str(lane.get("category")) == "present_matrix"
    ]


def lane_key(lane: dict[str, Any]) -> tuple[str, str, int, str]:
    return (
        str(lane.get("render_backend", "")),
        str(lane.get("present_mode", "")),
        int(number(lane.get("max_frame_latency")) or 0),
        scenario_name(lane),
    )


def coverage(matrix: dict[str, Any]) -> dict[str, Any]:
    lanes = present_lanes(matrix)
    by_key = {lane_key(lane): lane for lane in lanes}
    required = []
    missing_defined = []
    missing_passed = []
    for backend in REQUIRED_BACKENDS:
        for mode in REQUIRED_PRESENT_MODES:
            for latency in REQUIRED_FRAME_LATENCIES:
                for scenario in EXPECTED_SCENARIOS:
                    key = (backend, mode, latency, scenario)
                    required.append(key)
                    lane = by_key.get(key)
                    if lane is None:
                        missing_defined.append(key)
                    elif lane.get("status") != "passed":
                        missing_passed.append(key)
    optional_defined = 0
    for backend in REQUIRED_BACKENDS:
        for mode in OPTIONAL_PRESENT_MODES:
            for latency in REQUIRED_FRAME_LATENCIES:
                for scenario in EXPECTED_SCENARIOS:
                    if (backend, mode, latency, scenario) in by_key:
                        optional_defined += 1
    passed_count = len(required) - len(missing_defined) - len(missing_passed)
    return {
        "required_lane_count": len(required),
        "required_defined_count": len(required) - len(missing_defined),
        "required_passed_count": passed_count,
        "optional_defined_count": optional_defined,
        "complete": not missing_defined and not missing_passed,
        "missing_defined": [format_key(key) for key in missing_defined[:64]],
        "missing_passed": [format_key(key) for key in missing_passed[:64]],
        "missing_defined_count": len(missing_defined),
        "missing_passed_count": len(missing_passed),
    }


def format_key(key: tuple[str, str, int, str]) -> str:
    backend, mode, latency, scenario = key
    return f"{backend}/{mode}/fl{latency}/{scenario}"


def group_dx12_settings(matrix: dict[str, Any]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, int], dict[str, Any]] = {}
    for lane in present_lanes(matrix):
        if lane.get("status") != "passed" or str(lane.get("render_backend")) != "dx12":
            continue
        mode = str(lane.get("present_mode", ""))
        latency = int(number(lane.get("max_frame_latency")) or 0)
        scenario = scenario_name(lane)
        key = (mode, latency)
        group = groups.setdefault(
            key,
            {
                "present_mode": mode,
                "max_frame_latency": latency,
                "scenarios": set(),
                "fps_mean_values": [],
                "frame_p95_values": [],
                "present_wait_p95_values": [],
                "lanes": [],
            },
        )
        group["scenarios"].add(scenario)
        group["lanes"].append(str(lane.get("name", "")))
        fps = metric_value(lane, "fps", "mean")
        frame_p95 = metric_value(lane, "frame_ns", "p95")
        present_wait = metric_value(lane, "present_wait_ns", "p95")
        if fps is not None:
            group["fps_mean_values"].append(fps)
        if frame_p95 is not None:
            group["frame_p95_values"].append(frame_p95)
        if present_wait is not None:
            group["present_wait_p95_values"].append(present_wait)
    result = []
    for group in groups.values():
        fps_values = group["fps_mean_values"]
        frame_values = group["frame_p95_values"]
        wait_values = group["present_wait_p95_values"]
        scenario_list = sorted(group["scenarios"])
        result.append(
            {
                "present_mode": group["present_mode"],
                "max_frame_latency": group["max_frame_latency"],
                "scenario_count": len(scenario_list),
                "scenarios": scenario_list,
                "complete_scenario_set": all(scenario in group["scenarios"] for scenario in EXPECTED_SCENARIOS),
                "fps_mean_avg": sum(fps_values) / len(fps_values) if fps_values else None,
                "frame_ns_p95_worst": max(frame_values) if frame_values else None,
                "present_wait_ns_p95_worst": max(wait_values) if wait_values else None,
                "present_wait_sample_count": len(wait_values),
                "lanes": sorted(group["lanes"]),
            }
        )
    return sorted(result, key=lambda item: (str(item["present_mode"]), int(item["max_frame_latency"])))


def best_by(groups: list[dict[str, Any]], field: str, higher: bool) -> dict[str, Any] | None:
    candidates = [group for group in groups if number(group.get(field)) is not None]
    complete = [group for group in candidates if group.get("complete_scenario_set")]
    if complete:
        candidates = complete
    if not candidates:
        return None
    return sorted(candidates, key=lambda group: number(group.get(field)) or 0.0, reverse=higher)[0]


def setting_identity(group: dict[str, Any] | None) -> tuple[str, int] | None:
    if group is None:
        return None
    return str(group.get("present_mode")), int(number(group.get("max_frame_latency")) or 0)


def hardware_snapshot(matrix: dict[str, Any]) -> dict[str, Any]:
    env = matrix.get("environment") if isinstance(matrix.get("environment"), dict) else {}
    gpu = env.get("gpu") if isinstance(env.get("gpu"), list) else []
    names = [str(item.get("Name") or item.get("name") or "") for item in gpu if isinstance(item, dict)]
    joined = " ".join(names).lower()
    if "nvidia" in joined or "geforce" in joined or "rtx" in joined or "gtx" in joined:
        adapter_class = "nvidia_dgpu"
    elif "amd" in joined or "radeon" in joined:
        adapter_class = "amd_dgpu"
    elif "intel" in joined:
        adapter_class = "intel_gpu"
    else:
        adapter_class = "unknown"
    refresh = number(env.get("monitor_refresh_hz"))
    vrr = str(env.get("vrr_state", "unknown")).lower()
    return {
        "adapter_class": adapter_class,
        "adapter_names": names,
        "monitor_refresh_hz": refresh,
        "vrr_state": vrr,
        "capture_tools": env.get("capture_tools", {}),
    }


def build_report(matrix: dict[str, Any]) -> dict[str, Any]:
    cov = coverage(matrix)
    groups = group_dx12_settings(matrix)
    best_throughput = best_by(groups, "fps_mean_avg", higher=True)
    best_p95 = best_by(groups, "frame_ns_p95_worst", higher=False)
    best_latency = best_by(groups, "present_wait_ns_p95_worst", higher=False)
    hardware = hardware_snapshot(matrix)
    missing_evidence = []
    if not cov["complete"]:
        missing_evidence.append("full present matrix with required scenarios")
    if best_latency is None:
        missing_evidence.append("present_wait_ns or PresentMon/GPUView latency evidence")
    if str(hardware.get("vrr_state", "unknown")).lower() in {"unknown", "not_collected"}:
        missing_evidence.append("VRR state")
    if hardware.get("monitor_refresh_hz") is None:
        missing_evidence.append("monitor refresh rate")

    default_status = "unchanged"
    default_reason = "current evidence is incomplete"
    default_setting = CURRENT_DEFAULT.copy()
    benchmark_override = {
        "throughput": summarize_group(best_throughput),
        "p95": summarize_group(best_p95),
        "latency": summarize_group(best_latency),
    }
    throughput_identity = setting_identity(best_throughput)
    p95_identity = setting_identity(best_p95)
    latency_identity = setting_identity(best_latency)
    throughput_value = number(best_throughput.get("fps_mean_avg")) if best_throughput else None
    p95_throughput = number(best_p95.get("fps_mean_avg")) if best_p95 else None
    p95_is_close_to_throughput = (
        throughput_value is not None
        and p95_throughput is not None
        and p95_throughput >= throughput_value * THROUGHPUT_TOLERANCE
    )
    if cov["complete"] and best_latency is not None:
        if p95_identity == latency_identity and p95_is_close_to_throughput:
            default_status = "change_recommended"
            default_reason = "complete matrix agrees on p95 and latency without sacrificing more than 5 percent mean FPS"
            default_setting = {
                "present_mode": best_p95["present_mode"],
                "max_frame_latency": best_p95["max_frame_latency"],
            }
        else:
            default_reason = "complete matrix has a throughput/p95/latency tradeoff; keep default until product priority chooses the tradeoff"
    elif cov["complete"]:
        default_reason = "complete matrix lacks latency evidence; do not change default without present_wait_ns, PresentMon, or GPUView"

    hardware_defaults = {
        "status": "not_justified",
        "reason": "single-hardware evidence is not enough for hardware-class defaults",
        "required_before_enable": [
            "adapter detection",
            "measured evidence for the hardware class",
            "explicit override",
            "startup log naming the selected class and fallback",
        ],
    }

    return {
        "schema": "fun.dx12_present_decision.v1",
        "matrix_json": str(matrix.get("source_path", "")),
        "created_at": matrix.get("created_at"),
        "coverage": cov,
        "hardware": hardware,
        "dx12_setting_groups": [summarize_group(group) for group in groups],
        "best_throughput_setting": summarize_group(best_throughput),
        "best_p95_setting": summarize_group(best_p95),
        "best_latency_setting": summarize_group(best_latency),
        "recommended_default": {
            "status": default_status,
            "setting": default_setting,
            "reason": default_reason,
            "current_default": CURRENT_DEFAULT,
        },
        "recommended_benchmark_override": benchmark_override,
        "hardware_class_defaults": hardware_defaults,
        "missing_evidence": missing_evidence,
    }


def summarize_group(group: dict[str, Any] | None) -> dict[str, Any] | None:
    if group is None:
        return None
    return {
        "present_mode": group.get("present_mode"),
        "max_frame_latency": group.get("max_frame_latency"),
        "scenario_count": group.get("scenario_count"),
        "complete_scenario_set": group.get("complete_scenario_set"),
        "fps_mean_avg": group.get("fps_mean_avg"),
        "frame_ns_p95_worst": group.get("frame_ns_p95_worst"),
        "present_wait_ns_p95_worst": group.get("present_wait_ns_p95_worst"),
        "present_wait_sample_count": group.get("present_wait_sample_count"),
        "scenarios": group.get("scenarios", []),
    }


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# DX12 Present Pacing Decision",
        "",
        f"- Created from matrix: `{report['matrix_json'] or 'unknown'}`",
        f"- Default decision: `{report['recommended_default']['status']}`",
        f"- Reason: {report['recommended_default']['reason']}",
        "",
        "## Coverage",
        "",
        "| field | value |",
        "|---|---:|",
    ]
    cov = report["coverage"]
    for field in (
        "required_lane_count",
        "required_defined_count",
        "required_passed_count",
        "optional_defined_count",
        "missing_defined_count",
        "missing_passed_count",
    ):
        lines.append(f"| {field} | {cov[field]} |")
    lines.extend(["", "## Best Settings", "", "| target | present | max latency | fps mean avg | worst p95 ns | worst present wait ns | scenarios |", "|---|---|---:|---:|---:|---:|---:|"])
    for label, key in (
        ("throughput", "best_throughput_setting"),
        ("p95", "best_p95_setting"),
        ("latency", "best_latency_setting"),
    ):
        group = report.get(key)
        if group is None:
            lines.append(f"| {label} | n/a | n/a | n/a | n/a | n/a | n/a |")
            continue
        lines.append(
            "| {label} | {present} | {latency} | {fps} | {p95} | {wait} | {scenarios} |".format(
                label=label,
                present=group["present_mode"],
                latency=group["max_frame_latency"],
                fps=format_number(group["fps_mean_avg"]),
                p95=format_number(group["frame_ns_p95_worst"]),
                wait=format_number(group["present_wait_ns_p95_worst"]),
                scenarios=group["scenario_count"],
            )
        )
    lines.extend(
        [
            "",
            "## Benchmark Override",
            "",
            "Use the target-specific rows above for benchmark sweeps. Do not change the product default unless the default decision is `change_recommended`.",
            "",
            "## Hardware Defaults",
            "",
            f"- status: `{report['hardware_class_defaults']['status']}`",
            f"- reason: {report['hardware_class_defaults']['reason']}",
            f"- detected adapter class: `{report['hardware']['adapter_class']}`",
            f"- monitor refresh Hz: `{format_number(report['hardware']['monitor_refresh_hz'])}`",
            f"- VRR state: `{report['hardware']['vrr_state']}`",
        ]
    )
    missing = report.get("missing_evidence", [])
    if missing:
        lines.extend(["", "## Missing Evidence", ""])
        lines.extend(f"- {item}" for item in missing)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def format_number(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def load_matrix(path: Path) -> dict[str, Any]:
    matrix = json.loads(path.read_text(encoding="utf-8-sig"))
    matrix["source_path"] = str(path)
    return matrix


def run(
    *,
    matrix_json: str,
    markdown_report: str,
    json_report: str,
) -> dict[str, Any]:
    report = build_report(load_matrix(Path(matrix_json)))
    if json_report:
        json_path = Path(json_report)
        json_path.parent.mkdir(parents=True, exist_ok=True)
        json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if markdown_report:
        write_markdown(Path(markdown_report), report)
    return report


def make_synthetic_matrix(root: Path) -> Path:
    lanes = []
    for backend in REQUIRED_BACKENDS:
        for mode in (*REQUIRED_PRESENT_MODES, *OPTIONAL_PRESENT_MODES):
            for latency in REQUIRED_FRAME_LATENCIES:
                for scenario in EXPECTED_SCENARIOS:
                    score = 100.0
                    frame_p95 = 8_000_000.0
                    present_wait = 2_000_000.0
                    if mode == "auto_no_vsync" and latency == 2:
                        score = 180.0
                        frame_p95 = 4_000_000.0
                        present_wait = 500_000.0
                    lanes.append(
                        {
                            "name": f"present_{backend}_{mode}_fl{latency}_{scenario}",
                            "category": "present_matrix",
                            "status": "passed",
                            "render_backend": backend,
                            "present_mode": mode,
                            "max_frame_latency": latency,
                            "cef_ui_mode": "animated" if scenario == "ui_accelerated" else "disabled",
                            "cef_paint_transport": "d3d11on12" if scenario == "ui_accelerated" else "default",
                            "key_metrics": {
                                "fps": {"mean": score},
                                "frame_ns": {"p95": frame_p95},
                                "present_wait_ns": {"p95": present_wait},
                            },
                        }
                    )
    path = root / "matrix.json"
    path.write_text(
        json.dumps(
            {
                "created_at": "2026-05-04T00:00:00Z",
                "matrix_size": "present",
                "environment": {
                    "monitor_refresh_hz": "144",
                    "vrr_state": "enabled",
                    "gpu": [{"Name": "NVIDIA GeForce RTX synthetic"}],
                },
                "lanes": lanes,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return path


def self_test() -> None:
    root = Path.cwd() / "target" / "tool-selftests" / "dx12-present"
    root.mkdir(parents=True, exist_ok=True)
    matrix = make_synthetic_matrix(root)
    report = run(
        matrix_json=str(matrix),
        markdown_report=str(root / "present.md"),
        json_report=str(root / "present.json"),
    )
    if report["coverage"]["complete"] is not True:
        raise AssertionError("synthetic coverage should be complete")
    if report["recommended_default"]["status"] != "change_recommended":
        raise AssertionError("synthetic aligned best setting should recommend a default change")
    setting = report["recommended_default"]["setting"]
    if setting["present_mode"] != "auto_no_vsync" or setting["max_frame_latency"] != 2:
        raise AssertionError("unexpected synthetic default recommendation")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix-json", default="", help="Matrix JSON from benchmark_dx12_parity.ps1")
    parser.add_argument("--markdown-report", default="", help="Output Markdown report")
    parser.add_argument("--json-report", default="", help="Output JSON report")
    parser.add_argument("--self-test", action="store_true", help="Run synthetic self-test")
    args = parser.parse_args(argv)
    if args.self_test:
        self_test()
        print("dx12_present_decision_report self-test passed")
        return 0
    if not args.matrix_json:
        parser.error("--matrix-json is required unless --self-test is used")
    if not args.markdown_report and not args.json_report:
        parser.error("at least one output path is required")
    run(
        matrix_json=args.matrix_json,
        markdown_report=args.markdown_report,
        json_report=args.json_report,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
