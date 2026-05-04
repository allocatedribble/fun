#!/usr/bin/env python3
"""Build the DX12 PIX barrier/state summary artifact.

The tool is intentionally conservative: client benchmark JSON can provide scene
and CEF context, but barrier counts only become measured when an explicit PIX
CSV is supplied. Without PIX input the generated report is a blocked evidence
artifact, not a guessed barrier diagnosis.
"""

from __future__ import annotations

import argparse
import csv
import json
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


REQUIRED_SCENES: tuple[str, ...] = (
    "ui_hidden_representative",
    "ui_accelerated_representative",
    "solari_cloud_heavy",
    "meshlet_world_stream",
)


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def as_number(value: Any, default: float = 0.0) -> float:
    if value is None:
        return default
    try:
        return float(str(value).strip())
    except ValueError:
        return default


def normalize_key(value: str) -> str:
    return value.strip().lower().replace(" ", "_").replace("-", "_")


def read_pix_rows(paths: list[Path]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in paths:
        with path.open("r", encoding="utf-8-sig", newline="") as handle:
            reader = csv.DictReader(handle)
            for raw in reader:
                normalized = {normalize_key(k): v for k, v in raw.items() if k is not None}
                normalized["_source"] = str(path)
                rows.append(normalized)
    return rows


def first_value(row: dict[str, Any], *names: str, default: str = "") -> str:
    for name in names:
        value = row.get(name)
        if value is not None and str(value).strip():
            return str(value).strip()
    return default


def row_barrier_count(row: dict[str, Any]) -> int:
    return int(
        as_number(
            first_value(
                row,
                "barrier_count",
                "resource_barrier_count",
                "transition_count",
                "count",
                default="1",
            ),
            1.0,
        )
    )


def collect_summaries(matrix: dict[str, Any] | None, summary_paths: list[Path]) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    if matrix:
        for lane in matrix.get("lanes", []):
            summary_json = lane.get("summary_json")
            if not summary_json:
                continue
            path = Path(summary_json)
            if path.exists():
                summary = load_json(path)
                summary["_lane"] = lane.get("name")
                summary["_summary_json"] = str(path)
                summaries.append(summary)
    for path in summary_paths:
        if path.exists():
            summary = load_json(path)
            summary["_lane"] = path.parent.name
            summary["_summary_json"] = str(path)
            summaries.append(summary)
    return summaries


def benchmark_context(summaries: list[dict[str, Any]]) -> dict[str, Any]:
    lanes: list[dict[str, Any]] = []
    for summary in summaries:
        config = summary.get("config", {})
        health = summary.get("cef_ui_transport_health", {})
        selection = summary.get("cef_ui_transport_selection", {})
        lanes.append(
            {
                "lane": summary.get("_lane"),
                "summary_json": summary.get("_summary_json"),
                "render_backend": config.get("render_backend"),
                "present_mode": config.get("present_mode"),
                "cef_mode": config.get("cef_ui_mode"),
                "cef_transport": selection.get("selected") or config.get("cef_paint_transport"),
                "cef_health": health.get("status"),
            }
        )
    return {"lane_count": len(lanes), "lanes": lanes[:16]}


def analyze_pix(rows: list[dict[str, Any]]) -> dict[str, Any]:
    pass_counts: Counter[str] = Counter()
    resource_counts: Counter[str] = Counter()
    transition_counts: Counter[str] = Counter()
    common_bounces: Counter[str] = Counter()
    cef_transitions: Counter[str] = Counter()
    post_process_transitions: Counter[str] = Counter()
    tiny_pass_overheads: Counter[str] = Counter()
    queue_idle_ns: defaultdict[str, float] = defaultdict(float)

    for row in rows:
        count = row_barrier_count(row)
        pass_name = first_value(row, "pass", "event", "marker", "scope", "queue", default="unknown")
        resource = first_value(row, "resource", "resource_name", "texture", "object", default="unknown")
        before = first_value(row, "before", "before_state", "from", "state_before", default="unknown")
        after = first_value(row, "after", "after_state", "to", "state_after", default="unknown")
        pair = f"{before}->{after}"
        pass_counts[pass_name] += count
        resource_counts[resource] += count
        transition_counts[f"{pass_name}|{resource}|{pair}"] += count
        queue_idle_ns[pass_name] += as_number(
            first_value(row, "queue_idle_ns", "idle_ns", "gpu_idle_ns", default="0")
        )
        if before.upper() == "COMMON" or after.upper() == "COMMON":
            common_bounces[f"{resource}|{pair}"] += count
        lowered = f"{pass_name} {resource}".lower()
        if "cef" in lowered or "ui image" in lowered:
            cef_transitions[f"{pass_name}|{resource}|{pair}"] += count
        if "post" in lowered or "bloom" in lowered or "tonemap" in lowered:
            post_process_transitions[f"{pass_name}|{resource}|{pair}"] += count
        duration_ns = as_number(first_value(row, "duration_ns", "gpu_duration_ns", default="0"))
        if duration_ns > 0 and count >= 4 and duration_ns < 100_000:
            tiny_pass_overheads[pass_name] += count

    def top(counter: Counter[str], limit: int = 10) -> list[dict[str, Any]]:
        return [
            {"rank": rank + 1, "key": key, "count": int(value)}
            for rank, (key, value) in enumerate(counter.most_common(limit))
        ]

    return {
        "status": "measured" if rows else "blocked",
        "barriers_per_pass": top(pass_counts),
        "top_resources_by_transition_count": top(resource_counts),
        "top_transitions": top(transition_counts),
        "queue_idle_spans": [
            {"rank": rank + 1, "pass": key, "queue_idle_ns": int(value)}
            for rank, (key, value) in enumerate(
                sorted(queue_idle_ns.items(), key=lambda item: item[1], reverse=True)[:10]
            )
        ],
        "cef_copy_state_transitions": top(cef_transitions),
        "post_process_ping_pong_transitions": top(post_process_transitions),
        "common_bounces": top(common_bounces),
        "tiny_pass_large_barrier_overhead": top(tiny_pass_overheads),
        "row_count": len(rows),
    }


def build_report(
    *,
    pix_paths: list[Path],
    matrix_path: Path | None,
    summary_paths: list[Path],
) -> dict[str, Any]:
    matrix = load_json(matrix_path) if matrix_path and matrix_path.exists() else None
    summaries = collect_summaries(matrix, summary_paths)
    rows = read_pix_rows(pix_paths)
    analysis = analyze_pix(rows)
    missing = []
    if not pix_paths:
        missing.append("pix_barrier_csv")
    if analysis["status"] != "measured":
        missing.append("pix_rows")
    if not summaries:
        missing.append("benchmark_summary_json")
    scene_status = [
        {
            "scene": scene,
            "status": "measured" if rows else "missing",
            "evidence": "PIX CSV row" if rows else "no PIX capture imported",
        }
        for scene in REQUIRED_SCENES
    ]
    return {
        "schema": "fun.dx12_pix_barrier_summary.v1",
        "status": analysis["status"],
        "verdict": "blocked_missing_pix_evidence" if analysis["status"] != "measured" else "measured_ready_for_cleanup_selection",
        "pix_csv": [str(path) for path in pix_paths],
        "matrix_json": str(matrix_path) if matrix_path else None,
        "missing_evidence": missing,
        "required_scenes": scene_status,
        "benchmark_context": benchmark_context(summaries),
        **analysis,
    }


def write_markdown(report: dict[str, Any], path: Path) -> None:
    lines = [
        "# DX12 PIX Barrier Summary",
        "",
        f"- status: `{report['status']}`",
        f"- verdict: `{report['verdict']}`",
        f"- PIX CSV: {', '.join(report['pix_csv']) if report['pix_csv'] else 'none'}",
        f"- missing evidence: {', '.join(report['missing_evidence']) if report['missing_evidence'] else 'none'}",
        "",
        "## Capture Scenes",
        "",
        "| scene | status | evidence |",
        "|---|---|---|",
    ]
    for scene in report["required_scenes"]:
        lines.append(f"| {scene['scene']} | {scene['status']} | {scene['evidence']} |")
    lines.extend(["", "## Barriers Per Pass", ""])
    append_table(lines, report["barriers_per_pass"], ("rank", "key", "count"))
    lines.extend(["", "## Top Resources By Transition Count", ""])
    append_table(lines, report["top_resources_by_transition_count"], ("rank", "key", "count"))
    lines.extend(["", "## Queue Idle Spans", ""])
    append_table(lines, report["queue_idle_spans"], ("rank", "pass", "queue_idle_ns"))
    lines.extend(["", "## CEF Copy State Transitions", ""])
    append_table(lines, report["cef_copy_state_transitions"], ("rank", "key", "count"))
    lines.extend(["", "## Post-Process Ping-Pong Transitions", ""])
    append_table(lines, report["post_process_ping_pong_transitions"], ("rank", "key", "count"))
    lines.extend(["", "## COMMON Bounces", ""])
    append_table(lines, report["common_bounces"], ("rank", "key", "count"))
    lines.extend(["", "## Tiny Pass Barrier Overhead", ""])
    append_table(lines, report["tiny_pass_large_barrier_overhead"], ("rank", "key", "count"))
    lines.extend(
        [
            "",
            "## Cleanup Decision",
            "",
        ]
    )
    if report["status"] == "measured":
        lines.append("- Barrier cleanup may select exact resources from this summary.")
    else:
        lines.append("- No barrier cleanup is selected; importing PIX rows is the next required action.")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def append_table(lines: list[str], rows: list[dict[str, Any]], columns: tuple[str, ...]) -> None:
    if not rows:
        lines.append("_No measured rows._")
        return
    lines.append("| " + " | ".join(columns) + " |")
    lines.append("|" + "|".join("---" for _ in columns) + "|")
    for row in rows:
        lines.append("| " + " | ".join(str(row.get(column, "")) for column in columns) + " |")


def run(args: argparse.Namespace) -> int:
    report = build_report(
        pix_paths=[Path(path) for path in args.pix_csv],
        matrix_path=Path(args.matrix_json) if args.matrix_json else None,
        summary_paths=[Path(path) for path in args.summary_json],
    )
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "barrier_summary.json"
    markdown_path = output_dir / "barrier_summary.md"
    json_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    write_markdown(report, markdown_path)
    return 0 if report["status"] == "measured" else 2


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        pix = root / "pix.csv"
        pix.write_text(
            "scene,pass,resource,before,after,barrier_count,queue_idle_ns,duration_ns\n"
            "ui,fun.cef.copy_ring_source_to_bevy_image,CEF UI image,PIXEL_SHADER_RESOURCE,COPY_DEST,1,0,50000\n"
            "ui,fun.cef.copy_ring_source_to_bevy_image,CEF UI image,COPY_DEST,PIXEL_SHADER_RESOURCE,1,0,50000\n"
            "post,post_bloom,post_ping,COMMON,RENDER_TARGET,5,1000,50000\n",
            encoding="utf-8",
        )
        code = run(
            argparse.Namespace(
                pix_csv=[str(pix)],
                matrix_json="",
                summary_json=[],
                output_dir=str(root / "out"),
            )
        )
        data = json.loads((root / "out" / "barrier_summary.json").read_text(encoding="utf-8"))
        if code != 0 or data["status"] != "measured":
            raise AssertionError("synthetic PIX rows were not measured")
        if not data["cef_copy_state_transitions"]:
            raise AssertionError("CEF transitions were not extracted")
        if not data["common_bounces"]:
            raise AssertionError("COMMON bounce was not extracted")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pix", "--pix-csv", dest="pix_csv", action="append", default=[], help="PIX summary CSV to import")
    parser.add_argument("--matrix", "--matrix-json", dest="matrix_json", default="", help="Optional parity matrix JSON")
    parser.add_argument("--summary", "--summary-json", dest="summary_json", action="append", default=[], help="Optional benchmark summary JSON")
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
