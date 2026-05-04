#!/usr/bin/env python3
"""Generate the DX12 command submission and readback decision report."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any


COMMAND_METRICS: tuple[str, ...] = (
    "render_command_command_encoder_creations",
    "render_command_render_passes",
    "render_command_compute_passes",
    "render_command_command_buffers_submitted",
    "render_command_queue_submits",
    "render_command_copy_commands",
    "render_command_native_interop_command_insertions",
    "render_command_event_count",
)
READBACK_METRICS: tuple[str, ...] = (
    "render_readback_readback_requested_count",
    "render_readback_readback_completed_count",
    "render_readback_readback_dropped_count",
    "render_readback_readback_blocking_wait_count",
    "render_readback_readback_latency_frame_sum",
    "render_readback_readback_latency_frame_max",
    "render_readback_map_async_count",
    "render_readback_poll_count",
    "render_readback_event_count",
)
ACTIONABLE_COMMAND_CATEGORIES: set[str] = {
    "cef_copy",
    "debug_overlay",
    "uploads",
}
ACTIONABLE_COMMAND_OPERATIONS: set[str] = {
    "copy_command",
    "copy_only_command_buffer",
    "native_interop_command_inserted",
}
SUBMIT_HIGH_WATERMARK = 64.0
COMMAND_BUFFER_HIGH_WATERMARK = 256.0


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


def metric_from_summary(summary: dict[str, Any], metric: str, field: str = "p95") -> float | None:
    entry = summary.get("metrics", {}).get(metric)
    if not isinstance(entry, dict):
        return None
    return number(entry.get(field))


def metric_from_lane(lane: dict[str, Any], metric: str, field: str = "p95") -> float | None:
    entry = lane.get("key_metrics", {}).get(metric)
    if not isinstance(entry, dict):
        return None
    return number(entry.get(field))


def path_from_matrix(matrix_path: Path, raw: str) -> Path:
    path = Path(raw)
    if path.is_absolute():
        return path
    return matrix_path.parent / path


def collect_summaries(matrix_path: Path | None, summary_paths: list[Path]) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    if matrix_path and matrix_path.exists():
        matrix = load_json(matrix_path)
        for lane in matrix.get("lanes", []) or []:
            if not isinstance(lane, dict):
                continue
            raw = str(lane.get("summary_json") or "")
            if not raw:
                continue
            path = path_from_matrix(matrix_path, raw)
            if not path.exists():
                continue
            summary = load_json(path)
            summary["_lane"] = lane.get("name")
            summary["_matrix_category"] = lane.get("category")
            summaries.append(summary)
    for path in summary_paths:
        if path.exists():
            summary = load_json(path)
            summary["_lane"] = summary.get("config", {}).get("benchmark_matrix_lane") or path.parent.name
            summaries.append(summary)
    deduped: dict[str, dict[str, Any]] = {}
    for summary in summaries:
        key = str(summary.get("_source_path") or id(summary))
        deduped[key] = summary
    return list(deduped.values())


def collect_lanes(matrix_path: Path | None, summaries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    lanes: list[dict[str, Any]] = []
    if matrix_path and matrix_path.exists():
        matrix = load_json(matrix_path)
        for lane in matrix.get("lanes", []) or []:
            if isinstance(lane, dict):
                lanes.append(lane)
    if lanes:
        return lanes
    for summary in summaries:
        config = summary.get("config", {})
        lane = {
            "name": summary.get("_lane") or config.get("benchmark_matrix_lane"),
            "status": "passed",
            "render_backend": config.get("render_backend"),
            "present_mode": config.get("present_mode"),
            "cef_paint_transport": config.get("cef_paint_transport"),
            "key_metrics": {
                metric: {"p95": metric_from_summary(summary, metric)}
                for metric in (*COMMAND_METRICS, *READBACK_METRICS)
            },
        }
        lanes.append(lane)
    return lanes


def lane_metric_row(lane: dict[str, Any], metrics: tuple[str, ...]) -> dict[str, Any]:
    row = {
        "lane": lane.get("name") or lane.get("benchmark_lane") or "unknown",
        "status": lane.get("status", "unknown"),
        "backend": lane.get("render_backend", "unknown"),
        "present_mode": lane.get("present_mode", "unknown"),
        "cef_transport": lane.get("cef_paint_transport", "unknown"),
    }
    for metric in metrics:
        row[metric] = metric_from_lane(lane, metric)
    return row


def aggregate_events(summaries: list[dict[str, Any]], field: str) -> list[dict[str, Any]]:
    calls: Counter[tuple[str, str, str]] = Counter()
    samples: Counter[tuple[str, str, str]] = Counter()
    latency_sum: Counter[tuple[str, str, str]] = Counter()
    latency_max: dict[tuple[str, str, str], int] = {}
    for summary in summaries:
        for event in summary.get(field, []) or []:
            if not isinstance(event, dict):
                continue
            key = (
                str(event.get("operation", "unknown")),
                str(event.get("category", "unknown")),
                str(event.get("label", "unknown")),
            )
            calls[key] += int(number(event.get("calls")) or 0)
            samples[key] += int(number(event.get("samples")) or 0)
            latency_sum[key] += int(number(event.get("latency_frame_sum")) or 0)
            latency_max[key] = max(latency_max.get(key, 0), int(number(event.get("latency_frame_max")) or 0))
    rows = []
    for rank, (key, call_count) in enumerate(calls.most_common(12), start=1):
        operation, category, label = key
        rows.append(
            {
                "rank": rank,
                "operation": operation,
                "category": category,
                "label": label,
                "calls": call_count,
                "samples": samples[key],
                "latency_frame_sum": latency_sum[key],
                "latency_frame_max": latency_max.get(key, 0),
            }
        )
    return rows


def max_metric(rows: list[dict[str, Any]], metric: str) -> float:
    values = [number(row.get(metric)) for row in rows]
    return max((value for value in values if value is not None), default=0.0)


def actionable_command_events(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for event in events:
        category = str(event.get("category", ""))
        operation = str(event.get("operation", ""))
        label = str(event.get("label", ""))
        if category in ACTIONABLE_COMMAND_CATEGORIES:
            rows.append(event)
        elif operation in ACTIONABLE_COMMAND_OPERATIONS:
            rows.append(event)
        elif "copy" in label.lower() and str(event.get("calls", 0)) != "0":
            rows.append(event)
    return rows


def command_decision(command_rows: list[dict[str, Any]], command_events: list[dict[str, Any]]) -> dict[str, Any]:
    max_submits = max_metric(command_rows, "render_command_queue_submits")
    max_buffers = max_metric(command_rows, "render_command_command_buffers_submitted")
    actionable = actionable_command_events(command_events)
    high_counts = max_submits > SUBMIT_HIGH_WATERMARK or max_buffers > COMMAND_BUFFER_HIGH_WATERMARK
    if not command_rows or (max_submits == 0.0 and max_buffers == 0.0 and not command_events):
        return {
            "status": "missing_command_evidence",
            "next_action": "rerun with RenderDiagnostics so command counters and top command events are present",
            "required_trace": "command_counter_summary",
            "confidence": "high",
            "candidate": None,
        }
    if high_counts and actionable:
        top = actionable[0]
        return {
            "status": "candidate_needs_pix_before_behavior_change",
            "next_action": "inspect the top actionable command event in PIX before batching, merging, or moving the pass",
            "required_trace": "PIX or GPUView queue idle span around the candidate event",
            "confidence": "medium",
            "candidate": top,
        }
    if high_counts:
        return {
            "status": "needs_pix_queue_idle_before_behavior_change",
            "next_action": "do not reduce submits yet; current counters are high but top rows do not isolate a tiny copy/debug/UI submit offender",
            "required_trace": "PIX or GPUView queue idle span plus labels for remaining unlabeled command encoders",
            "confidence": "medium",
            "candidate": command_events[0] if command_events else None,
        }
    return {
        "status": "measured_no_submit_fragmentation_fix",
        "next_action": "keep submission behavior unchanged unless a later lane shows higher submits with queue idle or latency regression",
        "required_trace": "none",
        "confidence": "medium",
        "candidate": command_events[0] if command_events else None,
    }


def readback_decision(readback_rows: list[dict[str, Any]], readback_events: list[dict[str, Any]]) -> dict[str, Any]:
    max_requested = max_metric(readback_rows, "render_readback_readback_requested_count")
    max_completed = max_metric(readback_rows, "render_readback_readback_completed_count")
    max_dropped = max_metric(readback_rows, "render_readback_readback_dropped_count")
    max_blocking = max_metric(readback_rows, "render_readback_readback_blocking_wait_count")
    max_latency = max_metric(readback_rows, "render_readback_readback_latency_frame_max")
    max_map_async = max_metric(readback_rows, "render_readback_map_async_count")
    event_blocking = sum(
        int(number(event.get("calls")) or 0)
        for event in readback_events
        if str(event.get("operation")) == "blocking_wait"
    )
    diagnostic_events = [
        event
        for event in readback_events
        if str(event.get("category")) in {"debug", "timestamp", "screenshot"}
    ]
    if not readback_rows and not readback_events:
        status = "missing_readback_evidence"
        next_action = "rerun with readback diagnostics enabled"
        confidence = "high"
    elif max_blocking > 0 or event_blocking > 0:
        status = "blocking_waits_found"
        next_action = "move the blocking readback to N-frame delayed consumption or disable it in performance lanes"
        confidence = "high"
    elif max_requested > 0 or max_map_async > 0:
        status = "nonblocking_proven"
        next_action = "keep readback behavior unchanged; diagnostic/capture lanes should continue to mark readback overhead explicitly"
        confidence = "medium"
    else:
        status = "no_readbacks_active"
        next_action = "normal lane has no readback work; capture/debug lanes still need explicit overhead markers when enabled"
        confidence = "medium"
    return {
        "status": status,
        "next_action": next_action,
        "required_trace": "none" if status == "nonblocking_proven" else "readback_counter_summary",
        "confidence": confidence,
        "max_requested_p95": max_requested,
        "max_completed_p95": max_completed,
        "max_dropped_p95": max_dropped,
        "max_blocking_wait_p95": max_blocking,
        "max_latency_frame_p95": max_latency,
        "max_map_async_p95": max_map_async,
        "diagnostic_overhead_present": bool(diagnostic_events or max_requested > 0 or max_map_async > 0),
        "top_diagnostic_event": diagnostic_events[0] if diagnostic_events else None,
    }


def build_report(matrix_path: Path | None, summary_paths: list[Path]) -> dict[str, Any]:
    summaries = collect_summaries(matrix_path, summary_paths)
    lanes = collect_lanes(matrix_path, summaries)
    command_rows = [lane_metric_row(lane, COMMAND_METRICS) for lane in lanes]
    readback_rows = [lane_metric_row(lane, READBACK_METRICS) for lane in lanes]
    command_events = aggregate_events(summaries, "render_command_events")
    readback_events = aggregate_events(summaries, "render_readback_events")
    command_action = command_decision(command_rows, command_events)
    readback_action = readback_decision(readback_rows, readback_events)
    return {
        "schema": "fun.dx12_command_readback_report.v1",
        "matrix_json": str(matrix_path or ""),
        "summary_count": len(summaries),
        "lane_count": len(lanes),
        "command_submission_decision": command_action,
        "readback_decision": readback_action,
        "command_lane_summary": command_rows,
        "readback_lane_summary": readback_rows,
        "top_command_events": command_events,
        "top_readback_events": readback_events,
    }


def fmt(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    command = report["command_submission_decision"]
    readback = report["readback_decision"]
    lines = [
        "# DX12 Command And Readback Report",
        "",
        f"- Matrix: `{report['matrix_json'] or 'none'}`",
        f"- Lanes: {report['lane_count']}",
        f"- Summaries: {report['summary_count']}",
        f"- Submit decision: `{command['status']}`",
        f"- Submit next action: {command['next_action']}",
        f"- Readback decision: `{readback['status']}`",
        f"- Readback next action: {readback['next_action']}",
        "",
        "## Command Metrics",
        "",
        "| lane | backend | encoders p95 | buffers p95 | submits p95 | render passes p95 | compute passes p95 | copy commands p95 | native interop p95 |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in report["command_lane_summary"]:
        lines.append(
            "| {lane} | {backend} | {enc} | {buffers} | {submits} | {render} | {compute} | {copy} | {interop} |".format(
                lane=row["lane"],
                backend=row["backend"],
                enc=fmt(row.get("render_command_command_encoder_creations")),
                buffers=fmt(row.get("render_command_command_buffers_submitted")),
                submits=fmt(row.get("render_command_queue_submits")),
                render=fmt(row.get("render_command_render_passes")),
                compute=fmt(row.get("render_command_compute_passes")),
                copy=fmt(row.get("render_command_copy_commands")),
                interop=fmt(row.get("render_command_native_interop_command_insertions")),
            )
        )
    if not report["command_lane_summary"]:
        lines.append("| n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |")
    lines.extend(
        [
            "",
            "## Top Command Events",
            "",
            "| rank | operation | category | label | calls | samples |",
            "|---:|---|---|---|---:|---:|",
        ]
    )
    for event in report["top_command_events"]:
        lines.append(
            f"| {event['rank']} | {event['operation']} | {event['category']} | `{event['label']}` | {event['calls']} | {event['samples']} |"
        )
    if not report["top_command_events"]:
        lines.append("| 0 | n/a | n/a | n/a | 0 | 0 |")
    lines.extend(
        [
            "",
            "## Readback Metrics",
            "",
            "| lane | backend | requested p95 | completed p95 | dropped p95 | blocking waits p95 | latency frames p95 | map_async p95 | polls p95 |",
            "|---|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for row in report["readback_lane_summary"]:
        lines.append(
            "| {lane} | {backend} | {requested} | {completed} | {dropped} | {blocking} | {latency} | {maps} | {polls} |".format(
                lane=row["lane"],
                backend=row["backend"],
                requested=fmt(row.get("render_readback_readback_requested_count")),
                completed=fmt(row.get("render_readback_readback_completed_count")),
                dropped=fmt(row.get("render_readback_readback_dropped_count")),
                blocking=fmt(row.get("render_readback_readback_blocking_wait_count")),
                latency=fmt(row.get("render_readback_readback_latency_frame_max")),
                maps=fmt(row.get("render_readback_map_async_count")),
                polls=fmt(row.get("render_readback_poll_count")),
            )
        )
    if not report["readback_lane_summary"]:
        lines.append("| n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |")
    lines.extend(
        [
            "",
            "## Top Readback Events",
            "",
            "| rank | operation | category | label | calls | latency sum | latency max | samples |",
            "|---:|---|---|---|---:|---:|---:|---:|",
        ]
    )
    for event in report["top_readback_events"]:
        lines.append(
            f"| {event['rank']} | {event['operation']} | {event['category']} | `{event['label']}` | {event['calls']} | {event['latency_frame_sum']} | {event['latency_frame_max']} | {event['samples']} |"
        )
    if not report["top_readback_events"]:
        lines.append("| 0 | n/a | n/a | n/a | 0 | 0 | 0 | 0 |")
    lines.extend(
        [
            "",
            "## Acceptance",
            "",
            f"- Submit count reduction selected: {'yes' if command['status'] == 'fix_selected' else 'no'}",
            f"- Required submit trace: `{command['required_trace']}`",
            f"- Normal blocking readback waits: `{fmt(readback['max_blocking_wait_p95'])}` p95",
            f"- Diagnostic readback overhead present: `{str(readback['diagnostic_overhead_present']).lower()}`",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_report(
    *,
    matrix_json: str,
    summary_json: list[str],
    markdown_report: str,
    json_report: str,
) -> dict[str, Any]:
    matrix_path = Path(matrix_json) if matrix_json else None
    summary_paths = [Path(path) for path in summary_json]
    report = build_report(matrix_path, summary_paths)
    if markdown_report:
        write_markdown(Path(markdown_report), report)
    if json_report:
        json_path = Path(json_report)
        json_path.parent.mkdir(parents=True, exist_ok=True)
        json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


def self_test() -> None:
    base = Path.cwd() / "target" / "tool-self-tests"
    base.mkdir(parents=True, exist_ok=True)
    root = base / "dx12_command_readback_report"
    root.mkdir(parents=True, exist_ok=True)
    summary = {
        "config": {
            "benchmark_matrix_lane": "dx12_smoke",
            "render_backend": "dx12",
            "present_mode": "immediate",
            "cef_paint_transport": "d3d11on12",
        },
        "metrics": {
            "render_command_queue_submits": {"p95": 128},
            "render_command_command_buffers_submitted": {"p95": 384},
            "render_command_command_encoder_creations": {"p95": 384},
            "render_readback_readback_requested_count": {"p95": 32},
            "render_readback_readback_completed_count": {"p95": 32},
            "render_readback_readback_blocking_wait_count": {"p95": 0},
            "render_readback_map_async_count": {"p95": 32},
        },
        "render_command_events": [
            {
                "operation": "command_buffer_submitted",
                "category": "other",
                "label": "render_queue.submit",
                "calls": 384,
                "samples": 3,
            },
            {
                "operation": "command_encoder_created",
                "category": "other",
                "label": "unlabeled",
                "calls": 384,
                "samples": 3,
            },
        ],
        "render_readback_events": [
            {
                "operation": "map_async",
                "category": "debug",
                "label": "render_diagnostic_value_map_on_submit",
                "calls": 32,
                "samples": 3,
                "latency_frame_sum": 0,
                "latency_frame_max": 0,
            }
        ],
    }
    summary_path = root / "summary.json"
    summary_path.write_text(json.dumps(summary), encoding="utf-8")
    matrix = {
        "lanes": [
            {
                "name": "dx12_smoke",
                "status": "passed",
                "render_backend": "dx12",
                "present_mode": "immediate",
                "cef_paint_transport": "d3d11on12",
                "summary_json": str(summary_path),
                "key_metrics": {
                    "render_command_queue_submits": {"p95": 128},
                    "render_command_command_buffers_submitted": {"p95": 384},
                    "render_command_command_encoder_creations": {"p95": 384},
                    "render_readback_readback_requested_count": {"p95": 32},
                    "render_readback_readback_completed_count": {"p95": 32},
                    "render_readback_readback_blocking_wait_count": {"p95": 0},
                    "render_readback_map_async_count": {"p95": 32},
                },
            }
        ]
    }
    matrix_path = root / "matrix.json"
    matrix_path.write_text(json.dumps(matrix), encoding="utf-8")
    report = run_report(
        matrix_json=str(matrix_path),
        summary_json=[],
        markdown_report=str(root / "report.md"),
        json_report=str(root / "report.json"),
    )
    assert report["command_submission_decision"]["status"] == "needs_pix_queue_idle_before_behavior_change"
    assert report["readback_decision"]["status"] == "nonblocking_proven"
    assert report["readback_decision"]["diagnostic_overhead_present"] is True
    assert (root / "report.md").exists()
    assert (root / "report.json").exists()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix-json", default="")
    parser.add_argument("--summary-json", action="append", default=[])
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
        markdown_report=args.markdown_report,
        json_report=args.json_report,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
