#!/usr/bin/env python3
"""Summarize transient resource reuse evidence from benchmark JSON."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


NATIVE_INTEROP_EXCLUSIONS: tuple[tuple[str, str], ...] = (
    ("cef_ring_textures", "CEF shared/D3D11On12 ring resources own callback-local fences and native handles"),
    ("dlss_input_output", "DLSS input/output resources require explicit SDK state, lifetime, and non-aliasing validation"),
    ("solari_rr_guide_resources", "Ray Reconstruction guide resources are future native-DLSS inputs and must not alias yet"),
    ("readback_capture_resources", "readback and capture resources are synchronization surfaces, not scratch aliases"),
    ("raw_dx12_command_list_resources", "resources touched by raw DX12 command lists stay outside transient aliasing until audited"),
)
METRICS: tuple[str, ...] = (
    "transient_texture_requests",
    "transient_texture_creates",
    "transient_texture_reuses",
    "transient_texture_aliases",
    "transient_buffer_requests",
    "transient_buffer_creates",
    "transient_buffer_reuses",
    "transient_buffer_aliases",
    "transient_texture_descriptor_miss_creates",
    "transient_texture_lifetime_conflict_creates",
    "transient_buffer_descriptor_miss_creates",
    "transient_buffer_lifetime_conflict_creates",
    "transient_texture_near_miss_usage",
    "transient_texture_near_miss_format",
    "transient_texture_near_miss_size",
    "transient_texture_every_frame_create_descriptors",
    "transient_buffer_every_frame_create_descriptors",
)


def number(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    try:
        return float(str(value))
    except ValueError:
        return None


def metric_value(summary: dict[str, Any], metric: str, field: str = "p95") -> float | None:
    metrics = summary.get("metrics")
    if not isinstance(metrics, dict):
        return None
    entry = metrics.get(metric)
    if not isinstance(entry, dict):
        return None
    return number(entry.get(field))


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8-sig"))
    payload["source_path"] = str(path)
    return payload


def summary_paths_from_matrix(matrix: dict[str, Any]) -> list[Path]:
    paths: list[Path] = []
    matrix_path = Path(str(matrix.get("source_path", "")))
    base = matrix_path.parent if matrix_path.name else Path.cwd()
    for lane in matrix.get("lanes", []):
        if not isinstance(lane, dict):
            continue
        raw = str(lane.get("summary_json") or "")
        if not raw:
            continue
        path = Path(raw)
        if not path.is_absolute():
            path = base / path
        if path.exists():
            paths.append(path)
    return paths


def aggregate_descriptor_rows(summaries: list[dict[str, Any]], field: str) -> list[dict[str, Any]]:
    buckets: dict[tuple[str, ...], dict[str, Any]] = {}
    for summary in summaries:
        lane = str(summary.get("config", {}).get("benchmark_matrix_lane") or summary.get("source_path", "unknown"))
        for row in summary.get(field, []) or []:
            if not isinstance(row, dict):
                continue
            key = (
                str(row.get("resource", "unknown")),
                str(row.get("label", "unknown")),
                str(row.get("reason", "unknown")),
                str(row.get("near_miss", "unknown")),
                str(row.get("create_pattern", "unknown")),
            )
            bucket = buckets.setdefault(
                key,
                {
                    "resource": key[0],
                    "label": key[1],
                    "reason": key[2],
                    "near_miss": key[3],
                    "create_pattern": key[4],
                    "samples": 0,
                    "lanes": Counter(),
                },
            )
            bucket["samples"] += int(number(row.get("count")) or 1)
            bucket["lanes"][lane] += 1
    rows = []
    for bucket in buckets.values():
        rows.append(
            {
                "resource": bucket["resource"],
                "label": bucket["label"],
                "reason": bucket["reason"],
                "near_miss": bucket["near_miss"],
                "create_pattern": bucket["create_pattern"],
                "samples": bucket["samples"],
                "lane_count": len(bucket["lanes"]),
                "top_lanes": [name for name, _count in bucket["lanes"].most_common(5)],
            }
        )
    return sorted(rows, key=lambda row: (-int(row["samples"]), str(row["resource"]), str(row["label"])))


def aggregate_metrics(summaries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    values: dict[str, list[float]] = defaultdict(list)
    for summary in summaries:
        for metric in METRICS:
            value = metric_value(summary, metric, "p95")
            if value is not None:
                values[metric].append(value)
    rows = []
    for metric in METRICS:
        metric_values = values.get(metric, [])
        rows.append(
            {
                "metric": metric,
                "sample_count": len(metric_values),
                "max_p95": max(metric_values) if metric_values else None,
                "avg_p95": sum(metric_values) / len(metric_values) if metric_values else None,
            }
        )
    return rows


def choose_reuse_action(descriptor_rows: list[dict[str, Any]], metrics: list[dict[str, Any]]) -> dict[str, Any]:
    actionable = [
        row
        for row in descriptor_rows
        if row["near_miss"] in {"usage", "format", "view_formats"}
        or row["create_pattern"] == "every_frame"
    ]
    if actionable:
        top = actionable[0]
        return {
            "status": "candidate_found",
            "next_action": "canonicalize the top descriptor family with canonical_transient_texture_desc before broad allocator changes",
            "candidate": top,
        }
    every_frame = next(
        (row for row in metrics if row["metric"].endswith("every_frame_create_descriptors") and (row["max_p95"] or 0) > 0),
        None,
    )
    if every_frame:
        return {
            "status": "metric_candidate_without_rows",
            "next_action": "rerun with transient descriptor top rows enabled, then canonicalize the named family",
            "candidate": every_frame,
        }
    if descriptor_rows:
        return {
            "status": "observed_no_safe_fix",
            "next_action": "keep collecting descriptor rows; current rows do not isolate a usage/format/every-frame reuse miss",
            "candidate": descriptor_rows[0],
        }
    return {
        "status": "missing_descriptor_rows",
        "next_action": "rerun a transient-focused lane with bevy_render::transient=debug before changing descriptors",
        "candidate": None,
    }


def build_report(paths: list[Path], matrix_path: Path | None) -> dict[str, Any]:
    summaries = [load_json(path) for path in paths]
    descriptor_rows = aggregate_descriptor_rows(summaries, "transient_descriptor_creates")
    label_rows = aggregate_descriptor_rows(summaries, "transient_descriptor_label_variants")
    metrics = aggregate_metrics(summaries)
    action = choose_reuse_action(descriptor_rows, metrics)
    return {
        "schema": "fun.dx12_transient_reuse_report.v1",
        "matrix_json": str(matrix_path or ""),
        "summary_count": len(summaries),
        "top_descriptor_creates": descriptor_rows[:20],
        "top_label_variants": label_rows[:20],
        "metric_summary": metrics,
        "reuse_action": action,
        "native_interop_alias_exclusions": [
            {"resource_family": family, "status": "excluded", "reason": reason}
            for family, reason in NATIVE_INTEROP_EXCLUSIONS
        ],
    }


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    action = report["reuse_action"]
    lines = [
        "# DX12 Transient Reuse Report",
        "",
        f"- Matrix: `{report['matrix_json'] or 'none'}`",
        f"- Summary count: {report['summary_count']}",
        f"- Reuse action: `{action['status']}`",
        f"- Next action: {action['next_action']}",
        "",
        "## Top Descriptor Creates",
        "",
        "| samples | lanes | resource | near miss | pattern | label |",
        "|---:|---:|---|---|---|---|",
    ]
    for row in report["top_descriptor_creates"]:
        lines.append(
            f"| {row['samples']} | {row['lane_count']} | {row['resource']} | {row['near_miss']} | {row['create_pattern']} | `{row['label']}` |"
        )
    if not report["top_descriptor_creates"]:
        lines.append("| 0 | 0 | n/a | n/a | n/a | n/a |")
    lines.extend(["", "## Metric Summary", "", "| metric | samples | max p95 | avg p95 |", "|---|---:|---:|---:|"])
    for row in report["metric_summary"]:
        lines.append(
            f"| {row['metric']} | {row['sample_count']} | {format_number(row['max_p95'])} | {format_number(row['avg_p95'])} |"
        )
    lines.extend(["", "## Native Interop Aliasing Exclusions", "", "| resource family | status | reason |", "|---|---|---|"])
    for item in report["native_interop_alias_exclusions"]:
        lines.append(f"| {item['resource_family']} | {item['status']} | {item['reason']} |")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def format_number(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def run(
    *,
    matrix_json: str,
    summary_json: list[str],
    markdown_report: str,
    json_report: str,
) -> dict[str, Any]:
    matrix_path = Path(matrix_json) if matrix_json else None
    paths: list[Path] = [Path(path) for path in summary_json]
    if matrix_path is not None:
        matrix = load_json(matrix_path)
        paths.extend(summary_paths_from_matrix(matrix))
    deduped = []
    seen = set()
    for path in paths:
        resolved = path.resolve()
        if resolved in seen or not resolved.exists():
            continue
        seen.add(resolved)
        deduped.append(resolved)
    report = build_report(deduped, matrix_path)
    if json_report:
        json_path = Path(json_report)
        json_path.parent.mkdir(parents=True, exist_ok=True)
        json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if markdown_report:
        write_markdown(Path(markdown_report), report)
    return report


def self_test() -> None:
    root = Path.cwd() / "target" / "tool-selftests" / "dx12-transient"
    root.mkdir(parents=True, exist_ok=True)
    summary = root / "summary.json"
    summary.write_text(
        json.dumps(
            {
                "config": {"benchmark_matrix_lane": "synthetic_dx12"},
                "metrics": {
                    "transient_texture_creates": {"p95": 3},
                    "transient_texture_every_frame_create_descriptors": {"p95": 1},
                },
                "transient_descriptor_creates": [
                    {
                        "resource": "texture",
                        "label": "post.process.temp",
                        "reason": "descriptor_miss",
                        "near_miss": "usage",
                        "create_pattern": "every_frame",
                        "count": 4,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    report = run(
        matrix_json="",
        summary_json=[str(summary)],
        markdown_report=str(root / "report.md"),
        json_report=str(root / "report.json"),
    )
    if report["reuse_action"]["status"] != "candidate_found":
        raise AssertionError("synthetic usage near-miss should produce a candidate")
    if report["native_interop_alias_exclusions"][0]["status"] != "excluded":
        raise AssertionError("native interop exclusions should be explicit")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix-json", default="", help="Benchmark matrix JSON")
    parser.add_argument("--summary-json", action="append", default=[], help="Single benchmark summary JSON; may be repeated")
    parser.add_argument("--markdown-report", default="", help="Output Markdown report")
    parser.add_argument("--json-report", default="", help="Output JSON report")
    parser.add_argument("--self-test", action="store_true", help="Run synthetic self-test")
    args = parser.parse_args(argv)
    if args.self_test:
        self_test()
        print("dx12_transient_reuse_report self-test passed")
        return 0
    if not args.matrix_json and not args.summary_json:
        parser.error("--matrix-json or --summary-json is required unless --self-test is used")
    if not args.markdown_report and not args.json_report:
        parser.error("at least one output path is required")
    run(
        matrix_json=args.matrix_json,
        summary_json=args.summary_json,
        markdown_report=args.markdown_report,
        json_report=args.json_report,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
