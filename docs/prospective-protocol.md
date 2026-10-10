# The prospective protocol

**Status: NOT RUN.** This document specifies a test that would produce the first
out-of-sample measurement of this tool. It has not been run. No result is
reported here, and none should be added to this file — put results in
[`docs/calibration.md`](calibration.md) under a dated heading instead, so the
specification stays frozen and the result is visibly separate from it.

## The problem with everything measured so far

Every number in [`docs/calibration.md`](calibration.md) is **in-sample**. The 22
anti-patterns were derived from the same 97 rejections they were then scored
against, and the patterns were written with hindsight. That makes the published
figures a ceiling, not an estimate.

The `--known-issues` mechanism is worse off: it was tested on the very rows it
was designed to fix, with patterns auto-generated from the rejection summaries
those rows contain.

A prospective test fixes both, because the labels are written **before** the tool
runs.

## The protocol

### 1. Freeze the tool

Pick a tag or a commit and record it.

```bash
git rev-parse HEAD          # record this
cargo build --release
sha256sum target/release/vg # record this too
```

Do not change a rule, a pattern, a weight or the mapping for the duration of the
test. If you do, the run is void and you start again with a new freeze.

### 2. Declare N before you start

Choose N — the number of findings you will label — **before** seeing any of them.
Twenty is a floor; a hundred is a different kind of claim. Record N in this file
when you run it.

### 3. Label before running

For each of the next N submissions you make:

1. Write the finding.
2. **Write down the expected outcome and the gate you believe will decide it**,
   before submitting and before running `vg`.
3. Submit.
4. Record the actual outcome when it arrives.

The label is written blind. This is the entire point of the protocol: it is what
separates a test from a demonstration.

A row is only usable if the label predates the outcome. A row labelled after the
fact is in-sample data wearing a prospective label.

### 4. Run the tool on the frozen version

Only after all N labels exist:

```bash
target/release/vg check findings/*.md --json
python tools/calibrate/calibrate.py --log prospective-log.csv \
  --findings-dir findings/ --vg target/release/vg
```

### 5. Report all of it

Publish, with the N and the freeze commit:

| Metric | Definition |
|---|---|
| **Strict** | The tool flagged the gate that actually decided it |
| **Lenient** | The tool flagged any gate-mapped reason to stop |
| **Miss** | No gate-mapped rule fired |
| **False KILL** | The tool hard-failed a finding that was accepted or paid |
| **HOLD-then-submitted** | The tool said HOLD and the submission was paid anyway |

That last row is the one nobody reports and the one that matters most: it is the
cost of listening to the tool. A tool that stops you from submitting a paid
finding has a price, and the price belongs in the same table as the benefit.

### 6. Report the whole N, including the boring rows

Every finding you wrote during the window goes in the denominator, including the
ones you abandoned before submitting. Dropping those rows biases the sample
toward findings interesting enough to finish.

## What the test cannot settle

- **N will be small.** A prospective run over one researcher's next 30 submissions
  gives a direction, not a rate.
- **One researcher, one ecosystem mix.** The result generalises no further than
  the population it was drawn from.
- **It measures this frozen version.** A later rule change invalidates it, which
  is why the commit hash is recorded.

## Why write the protocol down before running it

Because the alternative has already been done. The `--known-issues` mechanism was
described as the fix for the duplicate class, then tested on the data that
produced the claim, and the test was unflattering. Writing the test down first
means the result cannot be selected after the fact — and if it is unflattering
again, it is still the result.
