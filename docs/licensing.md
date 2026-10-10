# Licensing

## Current state

| Path | Contents | Licence |
|---|---|---|
| `src/**` | Rust source | **MIT** |
| `tests/**` | Test code and fixtures | **MIT** |
| `scripts/**` | Python tooling | **MIT** |
| `tools/**` | The calibration harness | **MIT** |
| `rules/anti-patterns.yaml` | The anti-pattern catalog (data) | **MIT** — see the note below |
| `rules/gates.yaml` | The gate definitions (data) | **MIT** — see the note below |
| `rules/packs/**` | Ecosystem token packs (data) | **MIT** — see the note below |
| `docs/**`, `README.md` | Documentation | **MIT** |
| `examples/**` | Synthetic fixtures | **MIT** |

The repository `LICENSE` is the MIT licence and it is the only licence text in
the tree. There is no per-directory licence file and no `SPDX-License-Identifier`
header anywhere.

## The open question: `rules/*.yaml`

**Listed for the repository owner; not resolved here, and not changed here.**

The rule catalog is prose written by the author, derived from the author's own
rejection log. It is a creative work, and MIT on the repository plainly covers it
by default. The ambiguity is narrower than that and has two parts:

1. **The catalog is data, not code.** A reader who wants to reuse the *catalog*
   — extract it into another tool, or ship it as a ruleset — may reasonably ask
   whether the MIT grant, which reads as a software licence, was intended to
   cover a data file. It almost certainly was, and the answer should be stated
   rather than inferred.
2. **The catalog is derived from a private log.** The derivation is the author's
   own analysis, so no third-party rights are implicated. But "derived from a
   dataset that is not published" is worth a sentence in the licence story,
   because a reuser cannot check the derivation.

**Recommended resolution** (a decision for the owner, not for this task): add a
short "Scope" paragraph to `README.md`'s License section stating that the MIT
grant covers the entire repository including `rules/` and `docs/`, and that the
catalog is original work derived from the author's own unpublished dataset. That
is one paragraph, it removes the ambiguity, and it needs no second licence file.

**Not done here, on purpose.** Legal wording is a stop gate. This document
records the state and the recommendation; it does not act on them.

## What is deliberately not bundled

The tool ships **no** third-party data:

- No program names, no chain names, no reward tables.
- No rejection log. The 97-row dataset lives in
  [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) under its
  own licence and is not vendored here.
- No prior-audit PDFs and no known-issues corpora. Those are user inputs
  (`--known-issues`), supplied at runtime and never redistributed.
- No control set. [`docs/control-set.md`](control-set.md) explains how to build
  one from public paid reports; fetching one into this repository is a stop gate.

That is why there is no `NOTICE` file and no third-party licence inventory: the
only third-party code is the crate graph, and `cargo` resolves and reports those
licences directly (`cargo deny check licenses` in CI).

## Contributions

By opening a pull request you agree to license your contribution under the MIT
licence, matching the repository. See [`CONTRIBUTING.md`](../CONTRIBUTING.md).
