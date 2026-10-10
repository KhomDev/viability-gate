# `vg bench` fixture set

A tiny synthetic labelled set for [`vg bench`](../../docs/bench.md). Four rejected
findings, four accepted findings, and a labels file.

**This fixture measures nothing about the world.** It proves the harness runs and
that the columns mean what they say. Any number you compute from it describes
these eight synthetic files and nothing else. The real measurement is the one
you run on your own log; see [`docs/control-set.md`](../../docs/control-set.md).

## Run it

```bash
cargo build --release
target/release/vg bench \
  --rejected examples/bench/rejected \
  --accepted examples/bench/accepted \
  --labels   examples/bench/labels.csv
```

## What each file is for

### Rejected set

| File | The shape it exercises |
|---|---|
| `rejected/privileged-role-gated.md` | P2 — admin-gated against an exclusion |
| `rejected/griefing-no-attacker-profit.md` | P17 — no beneficiary; also P8 hedging |
| `rejected/testnet-only.md` | P11 — reproduced off the paying deployment |
| `rejected/front-run-operator.md` | P3 — race / operator-delay dependency |

### Accepted set

| File | What it shows |
|---|---|
| `accepted/atomic-unprivileged-drain.md` | A clean finding: permissionless, atomic, quantified, real PoC |
| `accepted/quantified-accounting-delta.md` | A clean finding: the accounting delta carries a number, so P13 is suppressed |
| `accepted/persisted-lock-demonstrated.md` | A headline that says "permanent lock" over a body that mentions a revert on the attacker's own path — the P19 false-positive mode, which the engine now *downgrades* to a note rather than hard-failing |
| `accepted/paid-dos-with-measured-liveness-loss.md` | A payable denial-of-service finding. P17 fires soft on it: this is a documented false-positive mode, and the fixture is here so the column shows a non-zero accepted fire |

The accepted set is deliberately **not** all-clean. A fixture set whose
false-positive columns are all zero cannot tell you whether the false-positive
columns work.

## The labels file

```csv
filename,label
atomic-unprivileged-drain.md,accepted
```

One row per finding, matched on filename. `vg bench` reports how many label rows
matched a finding, so a typo is visible as a count rather than as silence.

## Why there is no `expected.json` here

The calibration fixture under [`examples/calibration/`](../calibration/) pins its
numbers because the calibration method is the thing being protected. `vg bench`
output is a direct function of every rule in the catalog, so pinning it would
fail on every deliberate rule change and teach people to regenerate the pin
without reading the diff. Read the report instead.
