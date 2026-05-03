#!/usr/bin/env python3
"""Generate a DX12 parity dashboard from benchmark JSON files."""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
import tempfile
from pathlib import Path
from typing import Any


DELTA_METRICS: tuple[tuple[str, str, str], ...] = (
    ("fps", "higher", "FPS"),
    ("frame_ns", "lower", "frame time"),
    ("present_wait_ns", "lower", "present wait"),
    ("post_process_gpu_ns", "lower", "post process GPU"),
    ("standard_raster_gpu_ns", "lower", "standard raster GPU"),
    ("solari_gpu_ns", "lower", "Solari GPU"),
    ("meshlet_visibility_gpu_ns", "lower", "meshlet visibility GPU"),
    ("cloud_total_gpu_ns", "lower", "cloud total GPU"),
    ("ui_overlay_cpu_ns", "lower", "UI overlay CPU"),
    ("meshlet_extract_cpu_ns", "lower", "meshlet extract CPU"),
    ("meshlet_prepare_cpu_ns", "lower", "meshlet prepare CPU"),
    ("world_stream_apply_cpu_ns", "lower", "world stream CPU"),
    ("physics_fixed_update_cpu_ns", "lower", "physics CPU"),
    ("network_receive_cpu_ns", "lower", "network receive CPU"),
    ("cef_on_paint_fps", "higher", "CEF OnPaint FPS"),
    ("cef_on_accelerated_paint_fps", "higher", "CEF accelerated paint FPS"),
    ("cef_cpu_upload_bytes", "lower", "CEF CPU upload bytes"),
    ("cef_gpu_copy_bytes", "lower", "CEF GPU copy bytes"),
    ("cef_gpu_copy_ns", "lower", "CEF GPU copy"),
    ("cef_gpu_frame_ready_count", "higher", "CEF GPU ready frames"),
    ("cef_gpu_frame_not_ready_count", "lower", "CEF GPU not-ready frames"),
    ("cef_gpu_frame_reused_count", "lower", "CEF GPU reused frames"),
    ("cef_gpu_frame_blocking_wait_count", "lower", "CEF GPU blocking waits"),
    ("cef_transport_fallback_count", "lower", "CEF fallback count"),
    ("render_upload_write_texture_calls", "lower", "render texture upload calls"),
    ("render_upload_write_texture_bytes", "lower", "render texture upload bytes"),
    ("render_upload_write_buffer_calls", "lower", "render buffer upload calls"),
    ("render_upload_write_buffer_bytes", "lower", "render buffer upload bytes"),
    ("render_upload_write_buffer_with_calls", "lower", "render buffer-with upload calls"),
    ("render_upload_write_buffer_with_bytes", "lower", "render buffer-with upload bytes"),
    ("render_upload_callsite_count", "lower", "render upload callsite count"),
    ("transient_texture_creates", "lower", "transient texture creates"),
    ("transient_buffer_creates", "lower", "transient buffer creates"),
    ("render_scheduler_pressure", "lower", "render scheduler pressure"),
)

GPU_PASS_METRICS = {
    "post_process_gpu_ns",
    "standard_raster_gpu_ns",
    "solari_gpu_ns",
    "meshlet_visibility_gpu_ns",
    "cloud_total_gpu_ns",
}

CPU_WORK_METRICS = {
    "ui_overlay_cpu_ns",
    "meshlet_extract_cpu_ns",
    "meshlet_prepare_cpu_ns",
    "world_stream_apply_cpu_ns",
    "physics_fixed_update_cpu_ns",
    "network_receive_cpu_ns",
}


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8-sig") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return value


def get_path(value: Any, *path: str, default: Any = None) -> Any:
    current = value
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return default
        current = current[key]
    return current


def number(value: Any) -> float | None:
    if value is None:
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(parsed):
        return None
    return parsed


def metric_value(summary: dict[str, Any], metric: str, field: str) -> float | None:
    return number(get_path(summary, "metrics", metric, field))


def percent_delta(current: float | None, baseline: float | None) -> float | None:
    if current is None or baseline is None or baseline == 0:
        return None
    return ((current - baseline) / abs(baseline)) * 100.0


def classify_delta(metric: str, direction: str, mean_pct: float | None, p95_pct: float | None) -> tuple[str, str]:
    observed = p95_pct if p95_pct is not None else mean_pct
    if observed is None:
        return "unknown", "missing metric"
    regression = -observed if direction == "higher" else observed
    if regression > 5.0:
        return "red", "regression over 5 percent"
    if regression > 1.0:
        return "yellow", "regression over 1 percent"
    if metric == "frame_ns" and regression > 0.0:
        return "yellow", "minor p95 regression"
    return "green", "within threshold"


def build_delta_rows(vulkan: dict[str, Any], dx12: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for metric, direction, label in DELTA_METRICS:
        vk_mean = metric_value(vulkan, metric, "mean")
        dx_mean = metric_value(dx12, metric, "mean")
        vk_p95 = metric_value(vulkan, metric, "p95")
        dx_p95 = metric_value(dx12, metric, "p95")
        mean_delta = None if vk_mean is None or dx_mean is None else dx_mean - vk_mean
        p95_delta = None if vk_p95 is None or dx_p95 is None else dx_p95 - vk_p95
        mean_pct = percent_delta(dx_mean, vk_mean)
        p95_pct = percent_delta(dx_p95, vk_p95)
        status, reason = classify_delta(metric, direction, mean_pct, p95_pct)
        rows.append(
            {
                "metric": metric,
                "label": label,
                "direction": direction,
                "vulkan_mean": vk_mean,
                "dx12_mean": dx_mean,
                "delta": mean_delta,
                "delta_pct": mean_pct,
                "vulkan_p95": vk_p95,
                "dx12_p95": dx_p95,
                "p95_delta": p95_delta,
                "p95_delta_pct": p95_pct,
                "status": status,
                "reason": reason,
            }
        )
    return rows


def is_accelerated_cef_lane(summary: dict[str, Any]) -> bool:
    config = summary.get("config", {})
    selection = summary.get("cef_ui_transport_selection", {})
    transport = str(
        selection.get("selected") or config.get("cef_paint_transport", "")
    ).lower()
    return bool(config.get("cef_accelerated_feature_requested")) or transport in {
        "auto",
        "d3d11on12",
        "d3d11_shared_texture_dx12_copy",
    }


def summarize_gpu(summary: dict[str, Any]) -> str:
    gpu = get_path(summary, "hardware", "gpu", default=[])
    if isinstance(gpu, list) and gpu:
        first = gpu[0]
        if isinstance(first, dict):
            name = first.get("Name") or first.get("name") or "unknown"
            driver = first.get("DriverVersion") or first.get("driver_version") or "unknown"
            return f"{name} / driver {driver}"
    return "unknown"


def backend_summary(summary: dict[str, Any]) -> dict[str, str]:
    config = summary.get("config", {})
    presentation = summary.get("render_presentation", {})
    cef_selection = summary.get("cef_ui_transport_selection", {})
    width = config.get("window_width") or presentation.get("resolution_width") or "unknown"
    height = config.get("window_height") or presentation.get("resolution_height") or "unknown"
    return {
        "backend": str(config.get("render_backend") or presentation.get("backend") or "unknown"),
        "adapter": summarize_gpu(summary),
        "driver": str(presentation.get("driver") or "see adapter"),
        "present_mode": str(config.get("present_mode") or presentation.get("present_mode") or "unknown"),
        "surface_present_mode": str(presentation.get("surface_selected_present_mode") or "unknown"),
        "max_frame_latency": str(
            config.get("requested_maximum_frame_latency")
            or presentation.get("desired_maximum_frame_latency")
            or "unknown"
        ),
        "resolution": f"{width}x{height}",
        "cef_transport": str(
            cef_selection.get("selected") or config.get("cef_paint_transport") or "unknown"
        ),
        "cef_mode": str(config.get("cef_ui_mode") or "unknown"),
    }


def parse_external_csv(path: Path | None) -> dict[str, float]:
    if path is None:
        return {}
    if not path.exists():
        raise FileNotFoundError(path)
    totals: dict[str, float] = {}
    counts: dict[str, int] = {}
    with path.open("r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            for raw_key, raw_value in row.items():
                key = normalize_key(raw_key)
                value = number(raw_value)
                if value is None:
                    continue
                totals[key] = totals.get(key, 0.0) + value
                counts[key] = counts.get(key, 0) + 1
    return {key: totals[key] / counts[key] for key in totals if counts.get(key, 0) > 0}


def normalize_key(key: str | None) -> str:
    if not key:
        return ""
    out = []
    prev_sep = False
    for char in key.lower():
        if char.isalnum():
            out.append(char)
            prev_sep = False
        elif not prev_sep:
            out.append("_")
            prev_sep = True
    return "".join(out).strip("_")


def row_by_metric(rows: list[dict[str, Any]], metric: str) -> dict[str, Any] | None:
    for row in rows:
        if row["metric"] == metric:
            return row
    return None


def likely_bottleneck(
    rows: list[dict[str, Any]],
    vulkan: dict[str, Any],
    dx12: dict[str, Any],
    pix: dict[str, float],
    presentmon: dict[str, float],
) -> tuple[str, str, list[str]]:
    reasons: list[str] = []
    dx_fps = metric_value(dx12, "fps", "mean")
    vk_fps = metric_value(vulkan, "fps", "mean")
    dx_frame_p95 = metric_value(dx12, "frame_ns", "p95")
    vk_frame_p95 = metric_value(vulkan, "frame_ns", "p95")
    dx_loses_avg = dx_fps is not None and vk_fps is not None and dx_fps < vk_fps
    p95_regression = (
        dx_frame_p95 is not None
        and vk_frame_p95 is not None
        and dx_frame_p95 > vk_frame_p95 * 1.01
    )

    cef_cpu_upload = metric_value(dx12, "cef_cpu_upload_bytes", "mean") or 0.0
    if is_accelerated_cef_lane(dx12) and cef_cpu_upload > 0.0:
        reasons.append("accelerated CEF lane still reports CPU upload bytes")
        return "CEF sync", "PIX plus CEF transport counters", reasons

    present = row_by_metric(rows, "present_wait_ns")
    if present and present["p95_delta"] is not None and present["p95_delta"] > 500_000:
        reasons.append("DX12 present_wait_ns p95 is materially higher")
        return "present-bound", "PresentMon", reasons
    if presentmon:
        wait_keys = [key for key in presentmon if "present" in key and "wait" in key]
        if wait_keys:
            reasons.append("PresentMon CSV includes present wait fields; inspect pacing there")
            return "present-bound", "PresentMon", reasons

    cef_gpu = row_by_metric(rows, "cef_gpu_copy_ns")
    if cef_cpu_upload > 0.0 or (cef_gpu and cef_gpu["p95_delta"] is not None and cef_gpu["p95_delta"] > 200_000):
        reasons.append("CEF upload/copy metric regressed")
        return "upload-bound", "PIX", reasons

    texture_upload = row_by_metric(rows, "render_upload_write_texture_bytes")
    buffer_upload = row_by_metric(rows, "render_upload_write_buffer_bytes")
    if texture_upload and texture_upload["p95_delta"] is not None and texture_upload["p95_delta"] > 1_000_000:
        reasons.append("DX12 texture upload bytes are materially higher")
        return "upload-bound", "PIX", reasons
    if buffer_upload and buffer_upload["p95_delta"] is not None and buffer_upload["p95_delta"] > 1_000_000:
        reasons.append("DX12 buffer upload bytes are materially higher")
        return "upload-bound", "PIX", reasons

    if pix.get("barrier_count", 0.0) > 0 or pix.get("resource_barrier_count", 0.0) > 0:
        reasons.append("PIX summary includes barrier counters")
        return "barrier/state-bound", "PIX", reasons
    if pix.get("pipeline_creation_count", 0.0) > 0 or pix.get("pso_creation_count", 0.0) > 0:
        reasons.append("PIX summary includes pipeline creation counters")
        return "pipeline churn", "PIX", reasons
    if pix.get("descriptor_heap_switch_count", 0.0) > 0:
        reasons.append("PIX summary includes descriptor heap switch counters")
        return "pipeline churn", "PIX", reasons

    cpu_regressions = [
        row
        for row in rows
        if row["metric"] in CPU_WORK_METRICS and row["p95_delta"] is not None and row["p95_delta"] > 200_000
    ]
    if cpu_regressions:
        reasons.append(f"CPU-side metric regressed: {cpu_regressions[0]['metric']}")
        return "upload-bound", "PIX", reasons

    gpu_regressions = [
        row
        for row in rows
        if row["metric"] in GPU_PASS_METRICS and row["p95_delta"] is not None and row["p95_delta"] > 200_000
    ]
    if gpu_regressions:
        reasons.append(f"GPU pass metric regressed: {gpu_regressions[0]['metric']}")
        return "shader/pass GPU bound", "PIX or RenderDoc", reasons

    if not dx_loses_avg and p95_regression:
        reasons.append("DX12 wins average FPS but loses frame p95")
        return "p95 regression", "PresentMon plus PIX", reasons
    if dx_loses_avg:
        reasons.append("DX12 loses average FPS without a dominant parsed metric")
        return "unknown, requires PIX", "PIX", reasons

    reasons.append("no obvious DX12 loss in parsed metrics")
    return "no clear bottleneck", "PresentMon smoke trace", reasons


def lane_failure_messages(dx12: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    cef_cpu_upload = metric_value(dx12, "cef_cpu_upload_bytes", "mean") or 0.0
    if is_accelerated_cef_lane(dx12) and cef_cpu_upload > 0.0:
        failures.append(
            "CEF accelerated lane failed: cef_cpu_upload_bytes.mean is nonzero "
            f"({cef_cpu_upload:g})"
        )
    return failures


def format_number(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def format_pct(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    return f"{parsed:+.2f}%"


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = [
        "metric",
        "label",
        "status",
        "reason",
        "vulkan_mean",
        "dx12_mean",
        "delta",
        "delta_pct",
        "vulkan_p95",
        "dx12_p95",
        "p95_delta",
        "p95_delta_pct",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field) for field in fields})


def write_markdown(
    path: Path,
    vulkan: dict[str, Any],
    dx12: dict[str, Any],
    rows: list[dict[str, Any]],
    bottleneck: tuple[str, str, list[str]],
    failures: list[str],
    pix: dict[str, float],
    presentmon: dict[str, float],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    vk_summary = backend_summary(vulkan)
    dx_summary = backend_summary(dx12)
    category, trace, reasons = bottleneck
    p95_row = row_by_metric(rows, "frame_ns")
    fps_row = row_by_metric(rows, "fps")
    p95_regression = (
        fps_row is not None
        and p95_row is not None
        and (fps_row["delta"] or 0) > 0
        and (p95_row["p95_delta"] or 0) > 0
    )

    lines = [
        "# DX12 Parity Dashboard",
        "",
        "## Backend Summary",
        "",
        "| field | Vulkan | DX12 |",
        "|---|---|---|",
    ]
    for key in (
        "backend",
        "adapter",
        "driver",
        "present_mode",
        "surface_present_mode",
        "max_frame_latency",
        "resolution",
        "cef_transport",
        "cef_mode",
    ):
        lines.append(f"| {key} | {vk_summary[key]} | {dx_summary[key]} |")

    lines.extend(
        [
            "",
            "## DX12 vs Vulkan Deltas",
            "",
            "| status | metric | mean delta | mean delta % | p95 delta | p95 delta % | reason |",
            "|---|---|---:|---:|---:|---:|---|",
        ]
    )
    for row in rows:
        if row["vulkan_mean"] is None and row["dx12_mean"] is None:
            continue
        lines.append(
            "| {status} | {metric} | {delta} | {delta_pct} | {p95_delta} | {p95_delta_pct} | {reason} |".format(
                status=row["status"],
                metric=row["metric"],
                delta=format_number(row["delta"]),
                delta_pct=format_pct(row["delta_pct"]),
                p95_delta=format_number(row["p95_delta"]),
                p95_delta_pct=format_pct(row["p95_delta_pct"]),
                reason=row["reason"],
            )
        )

    lines.extend(
        [
            "",
            "## Bottleneck Guess",
            "",
            f"- Category: {category}",
            f"- Required next trace: {trace}",
        ]
    )
    if p95_regression:
        lines.append("- P95 regression: DX12 wins average FPS but loses frame p95.")
    if failures:
        lines.append(f"- Lane failure: {'; '.join(failures)}")
    for reason in reasons:
        lines.append(f"- Evidence: {reason}")

    lines.extend(["", "## External Trace Summary", ""])
    if pix:
        lines.append(f"- PIX fields parsed: {', '.join(sorted(pix)[:12])}")
    else:
        lines.append("- PIX fields parsed: none")
    if presentmon:
        lines.append(f"- PresentMon fields parsed: {', '.join(sorted(presentmon)[:12])}")
    else:
        lines.append("- PresentMon fields parsed: none")

    lines.extend(
        [
            "",
            "## Verdict",
            "",
            f"- Lane status: {'fail' if failures else 'pass'}",
            "- Red/yellow/green thresholds: red >5 percent regression, yellow >1 percent regression, green otherwise.",
        ]
    )

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_report(args: argparse.Namespace) -> int:
    vulkan = load_json(Path(args.vulkan_json))
    dx12 = load_json(Path(args.dx12_json))
    rows = build_delta_rows(vulkan, dx12)
    pix = parse_external_csv(Path(args.pix_csv)) if args.pix_csv else {}
    presentmon = parse_external_csv(Path(args.presentmon_csv)) if args.presentmon_csv else {}
    failures = lane_failure_messages(dx12)
    bottleneck = likely_bottleneck(rows, vulkan, dx12, pix, presentmon)
    write_csv(Path(args.csv_summary), rows)
    write_markdown(Path(args.markdown_report), vulkan, dx12, rows, bottleneck, failures, pix, presentmon)
    return 2 if failures else 0


def write_sample_summary(path: Path, backend: str, fps: float, frame_p95: float, cef_cpu_bytes: float) -> None:
    payload = {
        "hardware": {"gpu": [{"Name": "Synthetic GPU", "DriverVersion": "0.0"}]},
        "config": {
            "render_backend": backend,
            "present_mode": "immediate",
            "requested_maximum_frame_latency": 2,
            "window_width": 1280,
            "window_height": 720,
            "cef_ui_mode": "animated",
            "cef_paint_transport": "d3d11on12" if backend == "dx12" else "cpu",
            "cef_accelerated_feature_requested": backend == "dx12",
        },
        "metrics": {
            "fps": {"mean": fps, "p95": fps},
            "frame_ns": {"mean": 1_000_000_000.0 / fps, "p95": frame_p95},
            "present_wait_ns": {"mean": 100_000.0, "p95": 100_000.0},
            "cef_cpu_upload_bytes": {"mean": cef_cpu_bytes, "p95": cef_cpu_bytes},
        },
    }
    path.write_text(json.dumps(payload), encoding="utf-8")


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        vulkan = root / "vulkan.json"
        dx12 = root / "dx12.json"
        markdown = root / "report.md"
        summary_csv = root / "summary.csv"
        write_sample_summary(vulkan, "vulkan", fps=100.0, frame_p95=10_000_000.0, cef_cpu_bytes=0.0)
        write_sample_summary(dx12, "dx12", fps=105.0, frame_p95=15_000_000.0, cef_cpu_bytes=4096.0)
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(dx12),
                markdown_report=str(markdown),
                csv_summary=str(summary_csv),
                pix_csv="",
                presentmon_csv="",
            )
        )
        text = markdown.read_text(encoding="utf-8")
        if code != 2:
            raise AssertionError("accelerated CEF CPU upload failure did not produce exit code 2")
        if "P95 regression" not in text:
            raise AssertionError("p95 regression was not reported")
        if "CEF accelerated lane failed" not in text:
            raise AssertionError("CEF accelerated lane failure was not reported")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vulkan", "--vulkan-json", dest="vulkan_json", help="Vulkan baseline benchmark summary.json")
    parser.add_argument("--dx12", "--dx12-json", dest="dx12_json", help="DX12 benchmark summary.json")
    parser.add_argument("--pix", "--pix-csv", dest="pix_csv", default="", help="Optional PIX summary CSV")
    parser.add_argument("--presentmon", "--presentmon-csv", dest="presentmon_csv", default="", help="Optional PresentMon CSV")
    parser.add_argument("--markdown", "--markdown-report", dest="markdown_report", default="dx12_parity_report.md")
    parser.add_argument("--csv", "--csv-summary", dest="csv_summary", default="dx12_parity_summary.csv")
    parser.add_argument("--self-test", action="store_true", help="Run the built-in parser/report smoke test")
    return parser


def main(argv: list[str]) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    if not args.vulkan_json or not args.dx12_json:
        parser.error("--vulkan-json and --dx12-json are required unless --self-test is used")
    return run_report(args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
