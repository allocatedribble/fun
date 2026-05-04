#!/usr/bin/env python3
"""Generate the DX12 descriptor/PSO pipeline cardinality report."""

from __future__ import annotations

import argparse
import json
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any


PIPELINE_METRICS: tuple[str, ...] = (
    "render_churn_render_pipeline_creations",
    "render_churn_compute_pipeline_creations",
    "render_churn_pipeline_layout_creations",
    "render_churn_bind_group_layout_creations",
    "render_churn_material_pipeline_key_count",
    "render_churn_post_process_pipeline_key_count",
    "render_churn_cloud_pipeline_key_count",
    "render_churn_solari_pipeline_key_count",
    "render_churn_meshlet_pipeline_key_count",
    "render_churn_ui_pipeline_key_count",
    "render_churn_debug_overlay_pipeline_key_count",
    "render_shader_render_pipeline_create_count",
    "render_shader_compute_pipeline_create_count",
    "render_shader_pipeline_create_count",
    "render_shader_pipeline_create_ns",
)

CREATION_OPERATIONS = {
    "bind_group_layout",
    "bind_group_layout_cache_miss",
    "pipeline_layout",
    "render_pipeline_queued",
    "compute_pipeline_queued",
    "render_pipeline_created",
    "compute_pipeline_created",
    "render_pipeline_ready",
    "compute_pipeline_ready",
    "render_pipeline_error",
    "compute_pipeline_error",
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def metric(summary: dict[str, Any], name: str, field: str = "p95") -> float | None:
    value = summary.get("metrics", {}).get(name, {}).get(field)
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def collect_summaries(matrix_path: Path | None, summary_paths: list[Path]) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    if matrix_path and matrix_path.exists():
        matrix = load_json(matrix_path)
        for lane in matrix.get("lanes", []):
            path_text = lane.get("summary_json")
            if not path_text:
                continue
            path = Path(path_text)
            if not path.exists():
                continue
            summary = load_json(path)
            summary["_lane"] = lane.get("name")
            summary["_matrix_category"] = lane.get("category")
            summaries.append(summary)
    for path in summary_paths:
        if path.exists():
            summary = load_json(path)
            summary["_lane"] = path.parent.name
            summaries.append(summary)
    deduped: dict[str, dict[str, Any]] = {}
    for summary in summaries:
        key = str(summary.get("_lane") or summary.get("source_log") or id(summary))
        deduped[key] = summary
    return list(deduped.values())


def summarize_lane(summary: dict[str, Any]) -> dict[str, Any]:
    row = {
        "lane": summary.get("_lane"),
        "backend": summary.get("config", {}).get("render_backend"),
        "present_mode": summary.get("config", {}).get("present_mode"),
        "cef_transport": (
            summary.get("cef_ui_transport_selection", {}).get("selected")
            or summary.get("config", {}).get("cef_paint_transport")
        ),
    }
    for name in PIPELINE_METRICS:
        row[name] = metric(summary, name)
    return row


def top_creation_events(summaries: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], bool]:
    counter: Counter[tuple[str, str, str]] = Counter()
    has_creation_table = False
    for summary in summaries:
        events = summary.get("render_churn_creation_events")
        if events:
            has_creation_table = True
        else:
            events = [
                event
                for event in summary.get("render_churn_events", [])
                if event.get("operation") in CREATION_OPERATIONS
            ]
        for event in events or []:
            key = (
                str(event.get("operation", "unknown")),
                str(event.get("category", "unknown")),
                str(event.get("label", "unknown")),
            )
            counter[key] += int(float(event.get("calls", 0)))
    rows = [
        {
            "rank": rank + 1,
            "operation": operation,
            "category": category,
            "label": label,
            "calls": calls,
        }
        for rank, ((operation, category, label), calls) in enumerate(counter.most_common(10))
    ]
    return rows, has_creation_table


def top_shader_pipeline_events(summaries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    counter: Counter[tuple[str, str, str]] = Counter()
    elapsed: Counter[tuple[str, str, str]] = Counter()
    for summary in summaries:
        for event in summary.get("render_shader_events", []) or []:
            if event.get("operation") != "pipeline_created":
                continue
            key = (
                str(event.get("operation", "pipeline_created")),
                str(event.get("category", "unknown")),
                str(event.get("label", "unknown")),
            )
            counter[key] += int(float(event.get("calls", 0)))
            elapsed[key] += int(float(event.get("elapsed_ns", 0)))
    rows = []
    for rank, (key, calls) in enumerate(counter.most_common(10)):
        operation, category, label = key
        rows.append(
            {
                "rank": rank + 1,
                "operation": operation,
                "category": category,
                "label": label,
                "calls": calls,
                "elapsed_ns": elapsed[key],
            }
        )
    return rows


def category_cardinality(summaries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    fields = {
        "material": "render_churn_material_pipeline_key_count",
        "post_process": "render_churn_post_process_pipeline_key_count",
        "cloud": "render_churn_cloud_pipeline_key_count",
        "solari": "render_churn_solari_pipeline_key_count",
        "meshlet": "render_churn_meshlet_pipeline_key_count",
        "ui": "render_churn_ui_pipeline_key_count",
        "debug_overlay": "render_churn_debug_overlay_pipeline_key_count",
    }
    rows = []
    for category, field in fields.items():
        values = [metric(summary, field) or 0.0 for summary in summaries]
        rows.append(
            {
                "category": category,
                "max_p95": max(values) if values else 0.0,
                "field": field,
            }
        )
    return sorted(rows, key=lambda row: row["max_p95"], reverse=True)


def build_report(summaries: list[dict[str, Any]]) -> dict[str, Any]:
    lanes = [summarize_lane(summary) for summary in summaries]
    creation_events, has_creation_table = top_creation_events(summaries)
    shader_creation_events = top_shader_pipeline_events(summaries)
    categories = category_cardinality(summaries)
    max_render_creates = max((row.get("render_churn_render_pipeline_creations") or 0.0 for row in lanes), default=0.0)
    max_compute_creates = max((row.get("render_churn_compute_pipeline_creations") or 0.0 for row in lanes), default=0.0)
    max_shader_creates = max((row.get("render_shader_pipeline_create_count") or 0.0 for row in lanes), default=0.0)
    runtime_churn = max(max_render_creates, max_compute_creates, max_shader_creates) > 0
    if runtime_churn and (creation_events or shader_creation_events):
        verdict = "runtime PSO churn is the current bottleneck candidate; exact creation families are listed"
        next_action = "run observed pipeline warmup (FUN_RENDER_PIPELINE_WARMUP=observed), then compare creation counters before selecting layout canonicalization"
    elif runtime_churn:
        verdict = "runtime PSO churn is observed, but this artifact lacks creation-focused top events"
        next_action = "rerun benchmark_client after creation-event logging so warmup/layout work can target exact labels"
    else:
        verdict = "runtime PSO churn is not the current bottleneck in these summaries"
        next_action = "keep warmup off unless a scene transition produces creation counters"
    layout_decision = (
        "blocked_until_creation_events_and_PIX_descriptor_rows_identify_a_layout_family"
    )
    if creation_events:
        top_layout = next(
            (event for event in creation_events if "layout" in event["operation"]),
            None,
        )
        if top_layout:
            layout_decision = (
                f"candidate={top_layout['category']} label={top_layout['label']} "
                "pending binding-structure comparison"
            )
    return {
        "schema": "fun.dx12_pipeline_cardinality_report.v1",
        "summary_count": len(summaries),
        "verdict": verdict,
        "next_action": next_action,
        "has_creation_focused_events": has_creation_table,
        "layout_canonicalization_decision": layout_decision,
        "max_runtime_creations": {
            "render_pipeline_p95": max_render_creates,
            "compute_pipeline_p95": max_compute_creates,
            "shader_pipeline_create_p95": max_shader_creates,
        },
        "category_cardinality": categories,
        "lanes": lanes,
        "top_creation_events": creation_events,
        "top_shader_pipeline_events": shader_creation_events,
    }


def write_markdown(report: dict[str, Any], path: Path) -> None:
    lines = [
        "# DX12 Pipeline Cardinality Report",
        "",
        f"- summaries: {report['summary_count']}",
        f"- verdict: {report['verdict']}",
        f"- next action: {report['next_action']}",
        f"- creation-focused events present: `{str(report['has_creation_focused_events']).lower()}`",
        f"- layout decision: `{report['layout_canonicalization_decision']}`",
        "",
        "## Runtime Creation Maxima",
        "",
        "| metric | p95 max |",
        "|---|---:|",
    ]
    for key, value in report["max_runtime_creations"].items():
        lines.append(f"| {key} | {value:g} |")
    lines.extend(["", "## Pipeline Key Cardinality", ""])
    append_table(lines, report["category_cardinality"], ("category", "max_p95", "field"))
    lines.extend(["", "## Creation Events", ""])
    append_table(lines, report["top_creation_events"], ("rank", "operation", "category", "label", "calls"))
    lines.extend(["", "## Shader Pipeline Creation Events", ""])
    append_table(
        lines,
        report["top_shader_pipeline_events"],
        ("rank", "operation", "category", "label", "calls", "elapsed_ns"),
    )
    lines.extend(["", "## Lanes", ""])
    lane_columns = (
        "lane",
        "backend",
        "present_mode",
        "cef_transport",
        "render_churn_render_pipeline_creations",
        "render_churn_compute_pipeline_creations",
        "render_shader_pipeline_create_count",
    )
    append_table(lines, report["lanes"], lane_columns)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def append_table(lines: list[str], rows: list[dict[str, Any]], columns: tuple[str, ...]) -> None:
    if not rows:
        lines.append("_No rows._")
        return
    lines.append("| " + " | ".join(columns) + " |")
    lines.append("|" + "|".join("---" for _ in columns) + "|")
    for row in rows:
        lines.append("| " + " | ".join(str(row.get(column, "")) for column in columns) + " |")


def run(args: argparse.Namespace) -> int:
    summaries = collect_summaries(
        Path(args.matrix_json) if args.matrix_json else None,
        [Path(path) for path in args.summary_json],
    )
    report = build_report(summaries)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "pipeline_cardinality_report.json").write_text(
        json.dumps(report, indent=2),
        encoding="utf-8",
    )
    write_markdown(report, output_dir / "pipeline_cardinality_report.md")
    return 0


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        summary = root / "summary.json"
        summary.write_text(
            json.dumps(
                {
                    "config": {"render_backend": "dx12", "present_mode": "immediate"},
                    "metrics": {
                        "render_churn_render_pipeline_creations": {"p95": 2},
                        "render_churn_compute_pipeline_creations": {"p95": 1},
                        "render_shader_pipeline_create_count": {"p95": 3},
                        "render_churn_solari_pipeline_key_count": {"p95": 7},
                    },
                    "render_churn_creation_events": [
                        {
                            "operation": "render_pipeline_created",
                            "category": "solari",
                            "label": "bevy_solari::realtime::diffuse",
                            "calls": 2,
                        }
                    ],
                    "render_shader_events": [
                        {
                            "operation": "pipeline_created",
                            "category": "solari",
                            "label": "bevy_solari::realtime::diffuse",
                            "calls": 2,
                            "elapsed_ns": 10,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        code = run(
            argparse.Namespace(
                matrix_json="",
                summary_json=[str(summary)],
                output_dir=str(root / "out"),
            )
        )
        data = json.loads((root / "out" / "pipeline_cardinality_report.json").read_text())
        if code != 0:
            raise AssertionError("report returned failure")
        if not data["top_creation_events"]:
            raise AssertionError("creation event missing")
        if "runtime PSO churn" not in data["verdict"]:
            raise AssertionError("runtime PSO churn verdict missing")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", "--matrix-json", dest="matrix_json", default="", help="Optional parity matrix JSON")
    parser.add_argument("--summary", "--summary-json", dest="summary_json", action="append", default=[], help="Benchmark summary JSON")
    parser.add_argument("--output-dir", default="target/dx12-pix")
    parser.add_argument("--self-test", action="store_true")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
