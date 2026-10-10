#!/usr/bin/env python3
"""Check that README.md still agrees with the catalog and the calibration write-up.

This is the README-staleness job. It checks the claims that CAN be checked
mechanically, and it deliberately does not try to check tone, framing or the
prose that says what the tool is for -- a script cannot audit meaning.

What it checks:

  1. Every rule id in the catalog appears in README.md, or README links to the
     generated rule list. A catalog table that silently drops rules is how a
     "22 anti-patterns" claim becomes false.
  2. The rule-count claim matches the catalog.
  3. The strict and lenient calibration fractions in README.md match
     docs/calibration.md exactly. Two documents quoting different numbers is
     how a project ends up with two truths.
  4. Banned phrases that the calibration rewrite removed, and CLI names that do
     not exist. These are exact-match checks; each one has been true at least
     once.

Run: python scripts/check_readme.py
Exit code 0 = in sync, 1 = drift, 2 = could not run.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover
    print("check_readme: PyYAML is required (pip install pyyaml)", file=sys.stderr)
    sys.exit(2)

REPO_ROOT = Path(__file__).resolve().parent.parent
README = REPO_ROOT / "README.md"
CALIBRATION = REPO_ROOT / "docs" / "calibration.md"
CATALOG = REPO_ROOT / "rules" / "anti-patterns.yaml"

# Phrases the calibration rewrite removed, and the CLI surface. Each is here
# because it was wrong in this repository at least once.
BANNED = (
    ("Measured, not asserted",
     "the calibration is in-sample; use a heading that says so"),
    ("This tool is that mapping, executable",
     "replace with a computed statement of which log buckets the tool automates"),
    ("--explain",
     "the flag does not exist; the subcommand is '`vg explain <ID>`'"),
    ("vg rules list",
     "the subcommand is '`vg rules`'"),
    ("--fail-on KILL",
     "the flag is '`--fail-on-kill`'"),
    ("Design issue, not a security risk",
     "P18's title in the catalog is 'Known characteristic documented in a prior audit'"),
)

REQUIRED_SECTIONS = ("Known limitations",)

# Links the README must carry, each with the reason it is checked. A doc that is
# written and never linked is a doc nobody reads.
REQUIRED_LINKS = (
    ("examples/bench/", "the vg bench section must point at the bench fixture set, not at examples/calibration/"),
    ("docs/calibration.md", "the calibration write-up is the caveat every number depends on"),
    ("docs/control-set.md", "the false-positive rate is unmeasured and the README has to say where to measure it"),
    ("docs/install.md", "the install section must resolve, including the vg name collision"),
)


def load_rules() -> list[dict]:
    doc = yaml.safe_load(CATALOG.read_text(encoding="utf-8"))
    return doc["rules"]


def calibration_fractions() -> dict:
    """Pull the strict and lenient fractions out of docs/calibration.md."""
    text = CALIBRATION.read_text(encoding="utf-8")
    out = {}
    patterns = {
        "strict": r"\*\*Strict\*\*[^\n]*?\*\*(\d+)/(\d+)",
        "lenient": r"\*\*Lenient\*\*[^\n]*?\*\*(\d+)/(\d+)",
    }
    for key, pattern in patterns.items():
        m = re.search(pattern, text)
        if m:
            out[key] = (m.group(1), m.group(2))
    return out


def main() -> int:
    problems = []
    warnings = []

    for path in (README, CALIBRATION, CATALOG):
        if not path.exists():
            print("check_readme: missing " + str(path), file=sys.stderr)
            return 2

    readme = README.read_text(encoding="utf-8")
    rules = load_rules()
    ids = [r["id"] for r in rules]

    # 1. every rule id is reachable from the README
    missing = [rid for rid in ids if rid not in readme]
    links_to_index = "docs/rules/" in readme
    if missing and not links_to_index:
        problems.append(
            "README.md lists " + str(len(ids) - len(missing)) + " of " + str(len(ids))
            + " rule ids and does not link to docs/rules/: missing " + ", ".join(missing)
        )

    # 2. the rule count claim
    m = re.search(r"\*\*(\d+) entries?,?\s*P\d", readme)
    if not m:
        warnings.append("README.md states no rule count in the expected form")
    elif int(m.group(1)) != len(ids):
        problems.append(
            "README.md claims " + m.group(1) + " rules; the catalog has " + str(len(ids))
        )

    # 3. the calibration fractions agree between the two documents
    fracs = calibration_fractions()
    if not fracs:
        warnings.append("could not parse the strict/lenient fractions from docs/calibration.md")
    for key, (num, den) in fracs.items():
        needle = num + "/" + den
        if needle not in readme:
            problems.append(
                "README.md does not state " + key + " " + needle
                + " (docs/calibration.md does); the two documents disagree or one is stale"
            )

    # 3b. the automation table's buckets must exist in the gate mapping, and if
    # the sibling dataset is on disk, its row counts must match. This is the
    # table that says how much of the log the catalog can reach; a wrong number
    # here overstates the tool.
    mapping_path = REPO_ROOT / "docs" / "gate-mapping.yaml"
    reachable = set()
    if mapping_path.exists():
        mapping = yaml.safe_load(mapping_path.read_text(encoding="utf-8")) or {}
        reachable = set(mapping.get("reachable_tokens") or [])

    # Match a table row: | `bucket` | N | ... .
    #
    # Two things here are load-bearing and were both wrong at first:
    #   * the backtick is NOT escaped - a backslash before it inside a raw
    #     string is a literal backslash, and the pattern then matches nothing;
    #   * the leading \s* is required - this table sits inside a nested list
    #     item, so every row is indented.
    # Both bugs made the check pass silently, which is worse than failing.
    claimed = dict(
        re.findall(r"^\s*\| `(\w+)` \| (\d+) \|", readme, re.MULTILINE)
    )
    claimed = {k: int(v) for k, v in claimed.items()}
    claimed = {k: int(v) for k, v in claimed.items()}
    if reachable:
        unknown = sorted(set(claimed) - reachable)
        if unknown:
            problems.append(
                "README.md's automation table names bucket(s) that docs/gate-mapping.yaml "
                "does not list as reachable: " + ", ".join(unknown)
            )

    dataset = REPO_ROOT.parent / "rejection-taxonomy" / "data" / "rejections.csv"
    if claimed and dataset.exists():
        import csv
        from collections import Counter

        with dataset.open(encoding="utf-8") as fh:
            counts = Counter(row["earliest_gate"] for row in csv.DictReader(fh))
        for bucket, n in sorted(claimed.items()):
            actual = counts.get(bucket, 0)
            if actual != n:
                problems.append(
                    f"README.md says {bucket} is {n} row(s); the dataset says {actual}"
                )
        reachable_total = sum(counts.get(b, 0) for b in reachable)
        if f"**{reachable_total}**" not in readme:
            problems.append(
                f"README.md does not state the reachable total {reachable_total}; "
                f"recompute it from the dataset"
            )

    # 4. banned phrases
    for phrase, why in BANNED:
        if phrase in readme:
            problems.append("README.md contains " + repr(phrase) + ": " + why)

    # 5. required sections
    for section in REQUIRED_SECTIONS:
        if not re.search(r"^#+\s+" + re.escape(section), readme, re.MULTILINE | re.IGNORECASE):
            problems.append("README.md has no '" + section + "' section")

    # 6. required links
    for target, why in REQUIRED_LINKS:
        if target not in readme:
            problems.append("README.md does not link " + target + ": " + why)

    for w in warnings:
        print("warn  " + w)
    if problems:
        print("check_readme: README.md is stale (" + str(len(problems)) + " problem(s))")
        for p in problems:
            print("  " + p)
        return 1
    print("check_readme: README.md agrees with the catalog and docs/calibration.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
