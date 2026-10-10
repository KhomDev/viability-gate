# `vg bench` — measuring the catalog against a labelled set

```bash
vg bench --rejected DIR [--accepted DIR] [--labels labels.csv] [--json]
```

| Flag | Meaning |
|---|---|
| `--rejected DIR` | **Required.** A directory of findings that were rejected. `.md` and `.txt` files, read non-recursively |
| `--accepted DIR` | A directory of findings that were accepted or paid. Omit it and the false-KILL rate is reported as **UNMEASURED**, never as zero |
| `--labels labels.csv` | A CSV with a `filename,label` header. Used to check the set is labelled; the report states how many label rows matched a filename |
| `--json` | Machine-readable report |

## What it reports

Per rule, over the sets you supplied:

| Column | Definition |
|---|---|
| **REJ FIRE** | Findings in the rejected set this rule fired on, as a count and a rate |
| **ACC FIRE** | Findings in the accepted set this rule fired on |
| **ACC FALSE-KILL** | Accepted findings this rule **hard**-failed |

Plus two totals: hard-KILL on the rejected set, and hard-KILL on the accepted set
— the false-KILL count.

**A soft rule firing on an accepted finding is not a false kill.** It is a HOLD,
which is the tool's expected output for most drafts. The column that matters is
the hard one: a rule that turns an accepted finding into a KILL is the only
failure that costs a submission outright.

## Why the accepted set is the whole point

The catalog's precision has never been measured, because the author's own log
contains no paid findings. `vg bench` is the harness for the measurement; it
does not supply the measurement. See
[`docs/control-set.md`](control-set.md) for how to build an accepted set from
public paid reports, and [`docs/prospective-protocol.md`](prospective-protocol.md)
for the frozen test that would make the result credible.

## What a number from this command is, and is not

- **Is:** a measurement of the catalog against the set you supplied. Same input,
  same output — the tool is deterministic and offline.
- **Is not:** a validation. A high rejected fire rate on a set you selected
  because the catalog fires on it is circular. A low false-KILL rate on a
  five-finding accepted set has a confidence interval wider than the number.

Report the set's provenance with the number, every time. A rate without its
denominator and its selection rule is not a measurement.

## Fixture

[`examples/bench/`](../examples/bench/) holds a four-and-four synthetic set. It
proves the harness runs. It measures nothing.
