# Synthetic calibration fixture

Eleven synthetic findings, a labels file, and the pinned result. This is the
fixture that proves [`tools/calibrate/calibrate.py`](../../tools/calibrate/calibrate.py)
runs end to end in CI.

**It is not the calibration dataset.** The published in-sample calibration in
[`docs/calibration.md`](../../docs/calibration.md) was scored against a private
log under disclosure embargo; neither the log nor the original findings are
published, and no number in that document can be recomputed from this directory.
This fixture exercises the *method* on inputs that are safe to publish.

## Files

| Path | What it is |
|---|---|
| `labels.csv` | One row per submission: id, the gate that decided it, and a one-line description |
| `findings/*.md` | The finding text for each row, named after the row id |
| `expected.json` | The pinned result summary |

## Run it

```bash
cargo build --release

# inspect the row <-> file pairing without invoking vg
python tools/calibrate/calibrate.py --log examples/calibration/labels.csv \
  --findings-dir examples/calibration/findings --dry-run

# score it, and fail if the pinned numbers moved
python tools/calibrate/calibrate.py --log examples/calibration/labels.csv \
  --findings-dir examples/calibration/findings --vg target/release/vg \
  --expect examples/calibration/expected.json
```

## The pinned numbers

Measured on 2026-10-10 against the catalog as it stood at that commit:

```text
strict   8/10  (80.0%)  named the deciding gate
lenient  9/10  (90.0%)  named any gate-mapped reason to stop
miss     1/10  no gate-mapped rule fired
```

**Read these as a smoke test, not as a result.** Ten rows is not a sample; the
rows were written to exercise each gate token, so they are far easier than real
findings. The single miss is deliberate: `R-010` is a rejected finding about
rounding-direction bias whose text carries no signal the catalog can reach, which
is what a real miss looks like.

`expected.json` pins `rows`, `matched`, `strict`, `lenient`, `miss` and
`per_gate`. If a rule change moves one of them, `--expect` fails and prints the
regeneration command. That is the point: a rule change that moves the score
should be a decision, not an accident.

## Why one row is unmatched

`R-010`'s finding file is named `finding-accounting-drift.md`, not `R-010.md`,
and its content overlap with the row description is below the 0.50 confidence
threshold. The harness pairs a row with the file named after it first, and falls
back to best content overlap otherwise. The fallback is exercised on purpose
here, and the unmatched row is reported rather than silently dropped — a run that
quietly scores 10 of 11 rows as if they were 10 of 10 would be the worst possible
failure mode for a measurement tool.
