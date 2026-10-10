#!/usr/bin/env python3
"""Calibrate viability-gate against a labelled rejection log.

This is the public, reproducible form of the harness that produced the numbers in
docs/calibration.md. It reads:

  --log FILE            a labelled log: one row per rejected submission, with a
                        stable id, the text the row is about, and the gate token
                        that actually decided it.
  --findings-dir DIR    the original finding texts, one file per row, named by
                        the row id (R-001.md, R-001.txt, ...).
  --gate-mapping FILE   the public rule -> gate-token mapping (docs/gate-mapping.yaml).

and reports three numbers over the rows it could match:

  strict   the deciding gate token is among the tokens the fired rules map to
  lenient  at least one fired rule maps to any gate token
  miss     no fired rule maps to any gate token

WHAT IT DOES NOT DO. It does not reproduce the published result. The published
numbers were scored against a private dataset under disclosure embargo with a
mapping that lived in a private harness; neither is published. Running this tool
on your own log reproduces the METHOD. See the PROVENANCE note at the top of
docs/gate-mapping.yaml and the caveats section of docs/calibration.md.

NO NETWORK. The only subprocess this runs is the local vg binary.

Usage:
    python tools/calibrate/calibrate.py --log my-log.csv --findings-dir my-findings/
    python tools/calibrate/calibrate.py --log my-log.csv --findings-dir my-findings/ --dry-run
    python tools/calibrate/calibrate.py --log my-log.csv --findings-dir my-findings/ --json

Exit code 0 = ran, 1 = the run was invalid (bad input, vg missing), 2 = usage.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover
    print("calibrate: PyYAML is required (pip install pyyaml)", file=sys.stderr)
    sys.exit(1)

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEFAULT_MAPPING = REPO_ROOT / "docs" / "gate-mapping.yaml"

# Columns a log may use for the row's identifying text, in priority order.
TEXT_COLUMNS = ("title", "report_title", "mechanism", "summary", "description", "text")
ID_COLUMNS = ("id", "row_id", "rid")
GATE_COLUMNS = ("earliest_gate", "gate", "deciding_gate")

STOPWORDS = frozenset(
    """a an and are as at be been by for from has have how in into is it its of on or that the
    their there these this to was were what when where which who will with without not no""".split()
)

TOKEN_RE = re.compile(r"[a-z0-9_]+")


def tokenize(text: str) -> set[str]:
    return {t for t in TOKEN_RE.findall(text.lower()) if len(t) > 2 and t not in STOPWORDS}


def load_log(path: Path) -> tuple[list[dict], str]:
    """Return (rows, text_column). Each row is {id, text, gate}."""
    raw = path.read_text(encoding="utf-8-sig")
    if path.suffix.lower() in (".yaml", ".yml"):
        doc = yaml.safe_load(raw)
        records = doc.get("rows", doc) if isinstance(doc, dict) else doc
    elif path.suffix.lower() == ".json":
        records = json.loads(raw)
    else:
        records = list(csv.DictReader(raw.splitlines()))

    if not isinstance(records, list) or not records:
        raise ValueError("log is empty or not a list of rows")

    columns = set(records[0].keys())

    def pick(candidates: tuple[str, ...]) -> str | None:
        for c in candidates:
            if c in columns:
                return c
        return None

    id_col = pick(ID_COLUMNS)
    gate_col = pick(GATE_COLUMNS)
    text_col = pick(TEXT_COLUMNS)
    if not id_col:
        raise ValueError("log has no id column (looked for " + ", ".join(ID_COLUMNS) + ")")
    if not gate_col:
        raise ValueError("log has no gate column (looked for " + ", ".join(GATE_COLUMNS) + ")")
    if not text_col:
        raise ValueError("log has no text column (looked for " + ", ".join(TEXT_COLUMNS) + ")")

    rows = []
    for rec in records:
        rid = str(rec.get(id_col) or "").strip()
        text = str(rec.get(text_col) or "").strip()
        gate = str(rec.get(gate_col) or "").strip()
        if rid:
            rows.append({"id": rid, "text": text, "gate": gate})
    return rows, text_col


def load_findings(directory: Path) -> dict[str, Path]:
    """Map row id -> finding file, by filename stem."""
    if not directory.is_dir():
        raise ValueError("findings directory does not exist: " + str(directory))
    found: dict[str, Path] = {}
    for path in sorted(directory.iterdir()):
        if path.is_file() and path.suffix.lower() in (".md", ".txt", ".markdown"):
            found[path.stem] = path
    return found


def load_mapping(path: Path) -> dict:
    doc = yaml.safe_load(path.read_text(encoding="utf-8"))
    if "rules" not in doc:
        raise ValueError(str(path) + " has no rules key")
    return doc


def match_row(row: dict, row_tokens: set[str], findings: dict[str, Path]) -> tuple[Path | None, float]:
    """Pair a log row with a finding file.

    A file named after the row id wins. Otherwise every finding is scored by how
    much of the row's vocabulary it contains, and the best score wins -- ties
    break on filename so a run is deterministic. The score is a sanity check, not
    a search engine: a wrong row-to-file pairing produces a meaningless score,
    which is why the threshold exists.
    """
    named = findings.get(row["id"])
    if named is not None:
        return named, containment(row_tokens, named)
    best: tuple[Path | None, float] = (None, 0.0)
    for path in sorted(findings.values()):
        score = containment(row_tokens, path)
        if score > best[1]:
            best = (path, score)
    return best


def containment(row_tokens: set[str], path: Path) -> float:
    finding_tokens = tokenize(path.read_text(encoding="utf-8"))
    if not row_tokens:
        return 0.0
    return len(row_tokens & finding_tokens) / len(row_tokens)


def run_vg(vg: str, finding: Path) -> list[str]:
    """Return the rule ids vg fired on a finding."""
    proc = subprocess.run(
        [vg, "check", str(finding), "--json"],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError("vg exited " + str(proc.returncode) + ": " + proc.stderr.strip()[:400])
    report = json.loads(proc.stdout)
    return [f["rule"] for f in report.get("findings", [])]


def score(rows: list[dict], findings: dict[str, Path], mapping: dict, vg: str,
          min_confidence: float, dry_run: bool) -> dict:
    rule_gates: dict[str, list[str]] = {k: list(v or []) for k, v in mapping["rules"].items()}

    matched: list[dict] = []
    unmatched: list[dict] = []
    for row in rows:
        row_tokens = tokenize(row["text"])
        if not row_tokens:
            unmatched.append({"id": row["id"], "reason": "row has no content tokens"})
            continue
        path, confidence = match_row(row, row_tokens, findings)
        if path is None or confidence < min_confidence:
            unmatched.append({"id": row["id"], "reason": "no finding scored above "
                              + format(min_confidence, ".2f")})
            continue
        entry = {"id": row["id"], "gate": row["gate"], "confidence": round(confidence, 4),
                 "finding": path.name}
        if not dry_run:
            entry["fired"] = run_vg(vg, path)
        matched.append(entry)

    if dry_run:
        return {
            "rows": len(rows),
            "matched": len(matched),
            "unmatched": unmatched,
            "min_confidence": min_confidence,
            "dry_run": True,
        }

    strict = lenient = miss = 0
    per_gate: dict[str, dict[str, int]] = {}
    for entry in matched:
        fired = entry.get("fired", [])
        mapped: set[str] = set()
        for rule in fired:
            mapped.update(rule_gates.get(rule, []))
        entry["mapped_tokens"] = sorted(mapped)
        decided = entry["gate"]
        bucket = per_gate.setdefault(decided or "(none)", {"pairs": 0, "strict": 0, "lenient": 0})
        bucket["pairs"] += 1
        if mapped:
            lenient += 1
            bucket["lenient"] += 1
        else:
            miss += 1
        if decided and decided in mapped:
            strict += 1
            bucket["strict"] += 1

    n = len(matched)
    return {
        "rows": len(rows),
        "matched": n,
        "unmatched": unmatched,
        "min_confidence": min_confidence,
        "strict": strict,
        "lenient": lenient,
        "miss": miss,
        "strict_pct": round(100.0 * strict / n, 1) if n else 0.0,
        "lenient_pct": round(100.0 * lenient / n, 1) if n else 0.0,
        "per_gate": per_gate,
        "entries": matched,
        "dry_run": False,
    }


def report(result: dict, mapping_path: Path) -> None:
    print("calibration run")
    print("  gate mapping   " + str(mapping_path))
    print("  log rows       " + str(result["rows"]))
    print("  matched rows   " + str(result["matched"]) + "  (min confidence "
          + format(result["min_confidence"], ".2f") + ")")
    if result.get("dry_run"):
        print("  dry run        no vg invocation; showing row<->file matching only")
        for u in result["unmatched"]:
            print("    unmatched  " + u["id"] + ": " + u["reason"])
        return
    n = result["matched"]
    if n == 0:
        print("\n  no rows matched; nothing to score")
        return
    print("")
    print("  strict   " + str(result["strict"]) + "/" + str(n) + "  ("
          + format(result["strict_pct"], ".1f") + "%)  named the deciding gate")
    print("  lenient  " + str(result["lenient"]) + "/" + str(n) + "  ("
          + format(result["lenient_pct"], ".1f") + "%)  named any gate-mapped reason to stop")
    print("  miss     " + str(result["miss"]) + "/" + str(n) + "  no gate-mapped rule fired")
    print("")
    print("  by deciding gate")
    print("    " + "gate".ljust(24) + "pairs".rjust(6) + "strict".rjust(8) + "lenient".rjust(9))
    for gate in sorted(result["per_gate"]):
        b = result["per_gate"][gate]
        print("    " + gate.ljust(24) + str(b["pairs"]).rjust(6)
              + str(b["strict"]).rjust(8) + str(b["lenient"]).rjust(9))
    if result["unmatched"]:
        print("")
        print("  unmatched rows: " + str(len(result["unmatched"])))
        for u in result["unmatched"][:20]:
            print("    " + u["id"] + ": " + u["reason"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--log", required=True, type=Path, help="labelled rejection log (csv, json or yaml)")
    parser.add_argument("--findings-dir", required=True, type=Path, help="original finding texts, named by row id")
    parser.add_argument("--gate-mapping", type=Path, default=DEFAULT_MAPPING,
                        help="rule -> gate-token mapping (default: docs/gate-mapping.yaml)")
    parser.add_argument("--vg", default="vg", help="path to the vg binary (default: vg on PATH)")
    parser.add_argument("--min-confidence", type=float, default=0.50,
                        help="minimum row-to-finding token containment to score a row (default: 0.50)")
    parser.add_argument("--dry-run", action="store_true", help="show row<->file matching and exit without invoking vg")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument("--out", type=Path, help="write the JSON result to this path as well")
    parser.add_argument("--expect", type=Path,
                        help="compare the result summary against an expected JSON and fail on any drift")
    args = parser.parse_args(argv)

    try:
        rows, text_col = load_log(args.log)
        findings = load_findings(args.findings_dir)
        mapping = load_mapping(args.gate_mapping)
    except (OSError, ValueError) as exc:
        print("calibrate: " + str(exc), file=sys.stderr)
        return 1

    if not args.dry_run:
        vg = shutil.which(args.vg) or (args.vg if Path(args.vg).exists() else None)
        if not vg:
            print("calibrate: cannot find the vg binary (" + args.vg + ").", file=sys.stderr)
            print("  Build it with: cargo build --release   then pass --vg target/release/vg", file=sys.stderr)
            print("  Or inspect the row<->file matching with --dry-run.", file=sys.stderr)
            return 1
    else:
        vg = args.vg

    result = score(rows, findings, mapping, vg, args.min_confidence, args.dry_run)
    result["log_text_column"] = text_col
    result["log"] = str(args.log)
    result["findings_dir"] = str(args.findings_dir)

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=False))
    else:
        report(result, args.gate_mapping)

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=2, sort_keys=False) + "\n", encoding="utf-8")
        if not args.json:
            print("\n  wrote " + str(args.out))

    if args.expect:
        return compare_expected(result, args.expect, quiet=args.json)
    return 0


# Keys that are asserted by --expect. The full result is kept in the expected
# file so a reader can see the run, but only these keys gate CI: they are the
# numbers a change in the rules would move, and they do not depend on which
# individual rule fired as long as the gate class is the same.
EXPECTED_KEYS = ("rows", "matched", "strict", "lenient", "miss", "per_gate")


def compare_expected(result: dict, path: Path, quiet: bool = False) -> int:
    try:
        expected = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        print("calibrate: cannot read " + str(path) + ": " + exc, file=sys.stderr)
        return 1
    diffs: list[str] = []
    for key in EXPECTED_KEYS:
        if expected.get(key) != result.get(key):
            diffs.append("  " + key + ": expected " + json.dumps(expected.get(key))
                         + ", got " + json.dumps(result.get(key)))
    if diffs:
        print("calibrate --expect: fixture drifted", file=sys.stderr)
        for d in diffs:
            print(d, file=sys.stderr)
        print("  If the rules changed on purpose, regenerate the expected file:", file=sys.stderr)
        print("    python tools/calibrate/calibrate.py --log examples/calibration/labels.csv", file=sys.stderr)
        print("      --findings-dir examples/calibration/findings --vg target/release/vg", file=sys.stderr)
        print("      --json --out examples/calibration/expected.json", file=sys.stderr)
        return 1
    if not quiet:
        print("\n  fixture matches " + str(path) + " (" + ", ".join(EXPECTED_KEYS) + ")")
    return 0


if __name__ == "__main__":
    sys.exit(main())
