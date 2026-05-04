#!/usr/bin/env python3
"""Generate a meshlet/world-stream pressure decision report."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any


PRESSURE_METRICS: tuple[str, ...] = (
    "frame_ns",
    "fps",
    "meshlet_buffer_reallocations",
    "meshlet_instance_buffer_upload_bytes",
    "meshlet_material_buffer_upload_bytes",
    "meshlet_view_visibility_buffer_upload_bytes",
    "meshlet_asset_buffer_upload_bytes",
    "meshlet_asset_buffer_grow_copies",
    "meshlet_buffer_capacity_high_water_bytes",
    "meshlet_asset_buffer_capacity_bytes",
    "world_stream_apply_cpu_ns",
    "world_stream_render_prep_budget_ns",
    "world_stream_render_prep_max_chunks_per_frame",
    "world_stream_render_prep_limit_reason_code",
    "world_stream_render_prep_queue_depth",
    "world_stream_render_prep_deferred_chunks",
    "world_stream_render_prep_applied_chunks",
    "world_stream_render_prep_dynamic_mesh_assets",
    "render_upload_write_buffer_bytes",
    "render_upload_write_buffer_with_bytes",
)
UPLOAD_OFFENDER_TOKENS: tuple[str, ...] = ("meshlet", "world_stream", "world-stream", "stream")


def number(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    try:
        return float(str(value))
    except (TypeError, ValueError):
        return None


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8-sig"))
    payload["_source_path"] = str(path)
    return payload


def summary_sample_seconds(summary: dict[str, Any]) -> float:
    samples = summary.get("samples", {})
    seconds = number(samples.get("sample_seconds") if isinstance(samples, dict) else None)
    return seconds if seconds is not None and seconds > 0.0 else 1.0


def matrix_summary_paths(matrix_path: Path) -> list[Path]:
    matrix = load_json(matrix_path)
    paths: list[Path] = []
    for lane in matrix.get("lanes", []) or []:
        if not isinstance(lane, dict):
            continue
        raw = str(lane.get("summary_json") or "")
        if not raw:
            continue
        path = Path(raw)
        if not path.is_absolute():
            path = matrix_path.parent / path
        if path.exists():
            paths.append(path)
    return paths


def collect_summaries(matrix_path: Path | None, summary_paths: list[Path]) -> list[dict[str, Any]]:
    paths: list[Path] = []
    if matrix_path and matrix_path.exists():
        paths.extend(matrix_summary_paths(matrix_path))
    paths.extend(path for path in summary_paths if path.exists())
    summaries: dict[str, dict[str, Any]] = {}
    for path in paths:
        summary = load_json(path)
        config = summary.get("config", {})
        summary["_lane"] = config.get("benchmark_matrix_lane") or path.parent.name
        summaries[str(path)] = summary
    return list(summaries.values())


def metric(summary: dict[str, Any], name: str, field: str = "p95") -> float | None:
    entry = summary.get("metrics", {}).get(name)
    if not isinstance(entry, dict):
        return None
    return number(entry.get(field))


def lane_row(summary: dict[str, Any]) -> dict[str, Any]:
    config = summary.get("config", {})
    row = {
        "lane": summary.get("_lane") or config.get("benchmark_matrix_lane") or "unknown",
        "backend": config.get("render_backend", "unknown"),
        "present_mode": config.get("present_mode", "unknown"),
        "benchmark_lane": config.get("benchmark_lane", "unknown"),
        "stream_render_prep_budget_ms": config.get("stream_render_prep_budget_ms"),
        "stream_render_prep_max_chunks_per_frame": config.get(
            "stream_render_prep_max_chunks_per_frame"
        ),
        "source_path": summary.get("_source_path", ""),
    }
    for name in PRESSURE_METRICS:
        row[f"{name}_mean"] = metric(summary, name, "mean")
        row[f"{name}_p95"] = metric(summary, name, "p95")
        row[f"{name}_p99"] = metric(summary, name, "p99")
    return row


def max_field(rows: list[dict[str, Any]], field: str) -> float:
    return max((value for row in rows if (value := number(row.get(field))) is not None), default=0.0)


def capacity_decision(rows: list[dict[str, Any]]) -> dict[str, Any]:
    max_reallocations = max_field(rows, "meshlet_buffer_reallocations_p95")
    max_asset_grow = max_field(rows, "meshlet_asset_buffer_grow_copies_p95")
    max_capacity = max_field(rows, "meshlet_buffer_capacity_high_water_bytes_p95")
    if not rows:
        return {
            "status": "missing_evidence",
            "next_action": "run stream_pressure lanes before claiming capacity behavior",
        }
    if max_reallocations <= 0.0 and max_asset_grow <= 0.0:
        return {
            "status": "reallocations_disappeared",
            "next_action": "keep power-of-two capacity behavior; watch upload callsites for remaining pressure",
            "max_reallocations_p95": max_reallocations,
            "max_asset_grow_copies_p95": max_asset_grow,
            "max_capacity_high_water_bytes_p95": max_capacity,
        }
    if max_reallocations <= 4.0 and max_asset_grow <= 4.0:
        return {
            "status": "reallocations_bounded",
            "next_action": "compare before/after p95; bounded reallocations are acceptable only if streaming p95 improves or remains stable",
            "max_reallocations_p95": max_reallocations,
            "max_asset_grow_copies_p95": max_asset_grow,
            "max_capacity_high_water_bytes_p95": max_capacity,
        }
    return {
        "status": "reallocation_pressure",
        "next_action": "inspect meshlet buffer capacity buckets and remaining upload offenders before tuning render-prep budget",
        "max_reallocations_p95": max_reallocations,
        "max_asset_grow_copies_p95": max_asset_grow,
        "max_capacity_high_water_bytes_p95": max_capacity,
    }


def render_prep_decision(rows: list[dict[str, Any]]) -> dict[str, Any]:
    if not rows:
        return {
            "status": "missing_evidence",
            "next_action": "run stream_pressure lanes before selecting a render-prep budget or chunk cap",
        }
    max_limit_code = max_field(rows, "world_stream_render_prep_limit_reason_code_p95")
    max_deferred = max_field(rows, "world_stream_render_prep_deferred_chunks_p95")
    max_queue_depth = max_field(rows, "world_stream_render_prep_queue_depth_p95")
    max_applied = max_field(rows, "world_stream_render_prep_applied_chunks_p95")
    max_budget_ns = max_field(rows, "world_stream_render_prep_budget_ns_p95")
    if max_limit_code == 1.0:
        status = "time_budget_limited"
        next_action = "raise FUN_STREAM_RENDER_PREP_BUDGET_MS or lower chunk work; dashboard should show a time-to-ready versus p95 tradeoff"
    elif max_limit_code == 2.0:
        status = "max_chunks_limited"
        next_action = "raise FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME when time-to-ready matters more than p95 stability"
    elif max_deferred > 0.0 and max_applied > 0.0:
        status = "pressure_limited_unknown_reason"
        next_action = "rerun with new limit-reason metrics so the dashboard can separate time budget from chunk cap"
    elif max_queue_depth > 0.0 and max_applied <= 0.0:
        status = "not_draining_or_route_incomplete"
        next_action = "verify the streaming_spike route is receiving complete chunks and render prep is scheduled"
    else:
        status = "not_limited"
        next_action = "keep the current budget unless before/after p95 says otherwise"
    return {
        "status": status,
        "next_action": next_action,
        "max_limit_reason_code_p95": max_limit_code,
        "max_deferred_chunks_p95": max_deferred,
        "max_queue_depth_p95": max_queue_depth,
        "max_applied_chunks_p95": max_applied,
        "max_budget_ns_p95": max_budget_ns,
    }


def frame_delta_decision(rows: list[dict[str, Any]], baseline_rows: list[dict[str, Any]]) -> dict[str, Any]:
    if not baseline_rows:
        return {
            "status": "baseline_missing",
            "next_action": "attach a prior stream_pressure matrix or matched summary JSON before claiming p95 improvement",
            "comparisons": [],
        }
    baseline_by_lane = {str(row["lane"]): row for row in baseline_rows}
    comparisons = []
    for row in rows:
        lane = str(row["lane"])
        baseline = baseline_by_lane.get(lane)
        if baseline is None:
            continue
        current_p95 = number(row.get("frame_ns_p95"))
        baseline_p95 = number(baseline.get("frame_ns_p95"))
        current_p99 = number(row.get("frame_ns_p99"))
        baseline_p99 = number(baseline.get("frame_ns_p99"))
        if current_p95 is None or baseline_p95 is None:
            continue
        comparisons.append(
            {
                "lane": lane,
                "frame_ns_p95_delta": current_p95 - baseline_p95,
                "frame_ns_p95_delta_percent": percent_delta(current_p95, baseline_p95),
                "frame_ns_p99_delta": None
                if current_p99 is None or baseline_p99 is None
                else current_p99 - baseline_p99,
                "frame_ns_p99_delta_percent": None
                if current_p99 is None or baseline_p99 is None
                else percent_delta(current_p99, baseline_p99),
            }
        )
    if not comparisons:
        return {
            "status": "matched_baseline_missing",
            "next_action": "rerun baseline/candidate with stable lane names before claiming p95 improvement",
            "comparisons": [],
        }
    worst_p95_delta = max(item["frame_ns_p95_delta"] for item in comparisons)
    if worst_p95_delta < 0.0:
        status = "p95_improved"
        next_action = "capacity and render-prep changes improved matched p95; keep validating with heavier scenes"
    elif worst_p95_delta <= 0.0:
        status = "p95_stable"
        next_action = "p95 did not regress; use upload top offenders to choose the next cleanup"
    else:
        status = "p95_regressed"
        next_action = "do not promote tuning defaults; inspect limiter and upload offender rows"
    return {"status": status, "next_action": next_action, "comparisons": comparisons}


def percent_delta(current: float, baseline: float) -> float | None:
    if baseline == 0.0:
        return None
    return ((current - baseline) / baseline) * 100.0


def top_upload_offenders(summaries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    calls: Counter[tuple[str, str]] = Counter()
    bytes_written: Counter[tuple[str, str]] = Counter()
    sample_seconds: Counter[tuple[str, str]] = Counter()
    frame_count: Counter[tuple[str, str]] = Counter()
    for summary in summaries:
        seconds = summary_sample_seconds(summary)
        fps = metric(summary, "fps", "mean")
        frames = (fps or 0.0) * seconds
        for row in summary.get("render_upload_callsites", []) or []:
            if not isinstance(row, dict):
                continue
            label = str(row.get("label", "unknown"))
            operation = str(row.get("operation", "unknown"))
            key = (operation, label)
            calls[key] += int(number(row.get("calls")) or 0)
            bytes_written[key] += int(number(row.get("bytes")) or 0)
            sample_seconds[key] += seconds
            frame_count[key] += frames
    ranked = sorted(bytes_written, key=lambda key: (-bytes_written[key], -calls[key], key[1]))
    rows = []
    for rank, (operation, label) in enumerate(ranked[:12], start=1):
        key = (operation, label)
        seconds = sample_seconds[key]
        frames = frame_count[key]
        call_count = calls[key]
        byte_count = bytes_written[key]
        rows.append(
            {
                "rank": rank,
                "operation": operation,
                "label": label,
                "stream_related": any(token in label.lower() for token in UPLOAD_OFFENDER_TOKENS),
                "calls": call_count,
                "bytes": byte_count,
                "calls_per_frame": None if frames <= 0.0 else call_count / frames,
                "bytes_per_frame": None if frames <= 0.0 else byte_count / frames,
                "calls_per_second": None if seconds <= 0.0 else call_count / seconds,
                "bytes_per_second": None if seconds <= 0.0 else byte_count / seconds,
            }
        )
    return rows


def build_report(
    matrix_path: Path | None,
    summary_paths: list[Path],
    baseline_matrix_path: Path | None,
    baseline_summary_paths: list[Path],
) -> dict[str, Any]:
    summaries = collect_summaries(matrix_path, summary_paths)
    baseline_summaries = collect_summaries(baseline_matrix_path, baseline_summary_paths)
    rows = [lane_row(summary) for summary in summaries]
    baseline_rows = [lane_row(summary) for summary in baseline_summaries]
    return {
        "schema": "fun.dx12_meshlet_stream_pressure_report.v1",
        "matrix_json": str(matrix_path or ""),
        "baseline_matrix_json": str(baseline_matrix_path or ""),
        "summary_count": len(summaries),
        "baseline_summary_count": len(baseline_summaries),
        "capacity_decision": capacity_decision(rows),
        "render_prep_decision": render_prep_decision(rows),
        "frame_delta_decision": frame_delta_decision(rows, baseline_rows),
        "lane_summary": rows,
        "top_upload_offenders": top_upload_offenders(summaries),
    }


def fmt(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def fmt_config(value: Any, fallback: Any = None) -> str:
    parsed = number(value)
    if parsed is None:
        parsed = number(fallback)
    return fmt(parsed)


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    capacity = report["capacity_decision"]
    prep = report["render_prep_decision"]
    frame = report["frame_delta_decision"]
    lines = [
        "# DX12 Meshlet And Stream Pressure Report",
        "",
        f"- Matrix: `{report['matrix_json'] or 'none'}`",
        f"- Baseline matrix: `{report['baseline_matrix_json'] or 'none'}`",
        f"- Summaries: {report['summary_count']}",
        f"- Capacity decision: `{capacity['status']}`",
        f"- Capacity next action: {capacity['next_action']}",
        f"- Render-prep decision: `{prep['status']}`",
        f"- Render-prep next action: {prep['next_action']}",
        f"- Frame p95 decision: `{frame['status']}`",
        f"- Frame p95 next action: {frame['next_action']}",
        "",
        "## Lane Metrics",
        "",
        "| lane | backend | budget ms | max chunks | frame p95 ns | frame p99 ns | realloc p95 | upload bytes p95 | apply CPU p95 ns | prep limit | queue p95 | deferred p95 | applied p95 |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in report["lane_summary"]:
        upload_bytes = sum(
            number(row.get(metric)) or 0.0
            for metric in (
                "meshlet_instance_buffer_upload_bytes_p95",
                "meshlet_material_buffer_upload_bytes_p95",
                "meshlet_view_visibility_buffer_upload_bytes_p95",
                "meshlet_asset_buffer_upload_bytes_p95",
            )
        )
        lines.append(
            "| {lane} | {backend} | {budget} | {max_chunks} | {frame_p95} | {frame_p99} | {realloc} | {upload} | {apply_cpu} | {limit} | {queue} | {deferred} | {applied} |".format(
                lane=row["lane"],
                backend=row["backend"],
                budget=fmt_config(
                    row.get("stream_render_prep_budget_ms"),
                    (number(row.get("world_stream_render_prep_budget_ns_p95")) or 0.0) / 1_000_000.0,
                ),
                max_chunks=fmt_config(
                    row.get("stream_render_prep_max_chunks_per_frame"),
                    row.get("world_stream_render_prep_max_chunks_per_frame_p95"),
                ),
                frame_p95=fmt(row.get("frame_ns_p95")),
                frame_p99=fmt(row.get("frame_ns_p99")),
                realloc=fmt(row.get("meshlet_buffer_reallocations_p95")),
                upload=fmt(upload_bytes),
                apply_cpu=fmt(row.get("world_stream_apply_cpu_ns_p95")),
                limit=fmt(row.get("world_stream_render_prep_limit_reason_code_p95")),
                queue=fmt(row.get("world_stream_render_prep_queue_depth_p95")),
                deferred=fmt(row.get("world_stream_render_prep_deferred_chunks_p95")),
                applied=fmt(row.get("world_stream_render_prep_applied_chunks_p95")),
            )
        )
    if not report["lane_summary"]:
        lines.append("| n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |")
    lines.extend(
        [
            "",
            "## Top Upload Offenders",
            "",
            "| rank | operation | label | stream-related | calls/frame | bytes/frame | calls/sec | bytes/sec | total calls | total bytes |",
            "|---:|---|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for row in report["top_upload_offenders"]:
        lines.append(
            "| {rank} | {operation} | `{label}` | {stream_related} | {calls_per_frame} | {bytes_per_frame} | {calls_per_second} | {bytes_per_second} | {calls} | {bytes} |".format(
                rank=row["rank"],
                operation=row["operation"],
                label=row["label"],
                stream_related="yes" if row["stream_related"] else "no",
                calls_per_frame=fmt(row.get("calls_per_frame")),
                bytes_per_frame=fmt(row.get("bytes_per_frame")),
                calls_per_second=fmt(row.get("calls_per_second")),
                bytes_per_second=fmt(row.get("bytes_per_second")),
                calls=row["calls"],
                bytes=row["bytes"],
            )
        )
    if not report["top_upload_offenders"]:
        lines.append("| 0 | n/a | n/a | no | 0 | 0 | 0 | 0 | 0 | 0 |")
    lines.extend(
        [
            "",
            "## Baseline Comparisons",
            "",
            "| lane | frame p95 delta ns | frame p95 delta percent | frame p99 delta ns | frame p99 delta percent |",
            "|---|---:|---:|---:|---:|",
        ]
    )
    for row in frame["comparisons"]:
        lines.append(
            "| {lane} | {p95_delta} | {p95_pct} | {p99_delta} | {p99_pct} |".format(
                lane=row["lane"],
                p95_delta=fmt(row.get("frame_ns_p95_delta")),
                p95_pct=fmt(row.get("frame_ns_p95_delta_percent")),
                p99_delta=fmt(row.get("frame_ns_p99_delta")),
                p99_pct=fmt(row.get("frame_ns_p99_delta_percent")),
            )
        )
    if not frame["comparisons"]:
        lines.append("| n/a | n/a | n/a | n/a | n/a |")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_report(
    *,
    matrix_json: str,
    summary_json: list[str],
    baseline_matrix_json: str,
    baseline_summary_json: list[str],
    markdown_report: str,
    json_report: str,
) -> dict[str, Any]:
    report = build_report(
        Path(matrix_json) if matrix_json else None,
        [Path(path) for path in summary_json],
        Path(baseline_matrix_json) if baseline_matrix_json else None,
        [Path(path) for path in baseline_summary_json],
    )
    if markdown_report:
        write_markdown(Path(markdown_report), report)
    if json_report:
        path = Path(json_report)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


def self_test() -> None:
    root = Path.cwd() / "target" / "tool-self-tests" / "dx12_meshlet_stream_pressure_report"
    root.mkdir(parents=True, exist_ok=True)
    summary = {
        "config": {
            "benchmark_matrix_lane": "stream_pressure_dx12_control_budget2",
            "benchmark_lane": "streaming_spike",
            "render_backend": "dx12",
            "present_mode": "immediate",
            "stream_render_prep_budget_ms": 2,
            "stream_render_prep_max_chunks_per_frame": 0,
        },
        "metrics": {
            "frame_ns": {"p95": 10_000_000, "p99": 12_000_000},
            "meshlet_buffer_reallocations": {"p95": 0},
            "meshlet_asset_buffer_grow_copies": {"p95": 0},
            "meshlet_buffer_capacity_high_water_bytes": {"p95": 4096},
            "meshlet_instance_buffer_upload_bytes": {"p95": 1024},
            "meshlet_material_buffer_upload_bytes": {"p95": 256},
            "meshlet_view_visibility_buffer_upload_bytes": {"p95": 128},
            "world_stream_apply_cpu_ns": {"p95": 100_000},
            "world_stream_render_prep_budget_ns": {"p95": 2_000_000},
            "world_stream_render_prep_max_chunks_per_frame": {"p95": 0},
            "world_stream_render_prep_limit_reason_code": {"p95": 1},
            "world_stream_render_prep_queue_depth": {"p95": 2},
            "world_stream_render_prep_deferred_chunks": {"p95": 1},
            "world_stream_render_prep_applied_chunks": {"p95": 1},
        },
        "render_upload_callsites": [
            {
                "operation": "write_buffer",
                "label": "meshlet.instance_uniforms",
                "calls": 2,
                "bytes": 1024,
            }
        ],
    }
    summary_path = root / "summary.json"
    summary_path.write_text(json.dumps(summary), encoding="utf-8")
    report = run_report(
        matrix_json="",
        summary_json=[str(summary_path)],
        baseline_matrix_json="",
        baseline_summary_json=[],
        markdown_report=str(root / "report.md"),
        json_report=str(root / "report.json"),
    )
    assert report["capacity_decision"]["status"] == "reallocations_disappeared"
    assert report["render_prep_decision"]["status"] == "time_budget_limited"
    assert report["frame_delta_decision"]["status"] == "baseline_missing"
    assert report["top_upload_offenders"][0]["label"] == "meshlet.instance_uniforms"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix-json", default="")
    parser.add_argument("--summary-json", action="append", default=[])
    parser.add_argument("--baseline-matrix-json", default="")
    parser.add_argument("--baseline-summary-json", action="append", default=[])
    parser.add_argument("--markdown-report", default="")
    parser.add_argument("--json-report", default="")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not args.matrix_json and not args.summary_json:
        parser.error("--matrix-json or --summary-json is required")
    run_report(
        matrix_json=args.matrix_json,
        summary_json=args.summary_json,
        baseline_matrix_json=args.baseline_matrix_json,
        baseline_summary_json=args.baseline_summary_json,
        markdown_report=args.markdown_report,
        json_report=args.json_report,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
