# Changelog

Notable changes to `viability-gate`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Dates are the commit dates. Entries are written after the change, and a number
appears here only if it came from a command that can be re-run.

## Repository history reset - 2026-10-10

### Fixed

- **The declared MSRV was false.** `Cargo.toml` declared `rust-version = "1.74"`,
  but the dependency graph includes crates published under **edition 2024**, which
  Cargo 1.74 cannot parse, and `Cargo.lock` is lockfile **version 4**, which Cargo
  before 1.83 cannot read. The MSRV job therefore failed on every push. Verified
  directly: `cargo +1.74 build --release` fails with `this version of Cargo is older
  than the 2024 edition`; `cargo +1.85 build --release` and `cargo +1.85 test
  --release` both succeed against the committed lockfile. The declared MSRV is now
  1.85 and `docs/install.md` says so.

Repository history was reset on 2026-10-10 to remove identifying material from
earlier commits; earlier versions are no longer available.

The tree itself is unchanged except for the two fixes recorded here.

### Fixed

- **Five GitHub Actions expressions in `.github/workflows/ci.yml` were malformed**
  (a doubled brace in `runs-on`, the two MSRV toolchain references, and a
  `${{deny}` reference to a shell variable). They would have failed workflow
  parsing, so the job that was supposed to prove the repository clean could never
  have run. All five are corrected.
- **The token matcher now has fixtures.** `python scripts/check_tripwires.py --self-test`
  pins both directions: ordinary English that merely contains a token (suite,
  nearest, button) must pass, and the token itself must fail. CI runs it in the
  tripwires job, so a regression in the boundary rule cannot land silently.

## [Unreleased]

### Added

- **Ecosystem token packs** (`rules/packs/`). Language-specific tokens move out
  of the shipped catalog and into eight balanced packs (EVM/Solidity, Vyper,
  Move, Rust/Solana, Cairo, Cosmos/Go, TON, Polkadot/Substrate). The *presence*
  of any one ecosystem's tokens now says nothing about which ecosystem the author
  hunts. The catalog refers to a pack; it carries no ecosystem token itself.
- **`scripts/check_tripwires.py`** — three repo-wide checks: no private-workspace
  path prefixes anywhere; at least 7 balanced ecosystem packs; no pack token in
  the embedded catalog. Wired into CI.
- **Generated rule pages** (`docs/rules/P1.md`–`P22.md` plus an index), produced
  by `scripts/gen_rule_docs.py` from the catalog and
  `docs/rules/_annotations.yaml`. `--check` fails on drift and runs in CI, so a
  page cannot disagree with the rule it documents.
- **The calibration harness, published** as `tools/calibrate/calibrate.py`,
  with the rule-to-gate mapping restated publicly in `docs/gate-mapping.yaml`
  and a synthetic eleven-row fixture under `examples/calibration/` that runs end
  to end in CI with pinned numbers.
- **`docs/control-set.md`** — how to build an accepted/paid set from public
  reports so the false-positive rate can finally be measured. Status:
  **PENDING**, because it has not been built.
- **`docs/prospective-protocol.md`** — the frozen, pre-registered test that
  would produce the first out-of-sample measurement. Status: **NOT RUN**.
- **`docs/bench.md`** — the specification for `vg bench`, plus a synthetic
  four-rejected/four-accepted fixture under `examples/bench/`.
- **`docs/no-network.md`** — the no-network claim, verified with `cargo tree`
  (39 packages, none a network client) and a source grep, with the limits of that
  verification stated.
- **`docs/install.md`** — install paths, the `vg` name collision with the
  bioinformatics tool of the same name, and the fact that the crate ships both
  `vg` and `viability-gate`.
- **`docs/licensing.md`** — the current licence state (MIT throughout), with the
  open question about `rules/*.yaml` listed for the owner rather than resolved.
- **`docs/tripwires.md`** — what the tripwire checks cover, and what they
  deliberately exclude.
- **CI**: format, clippy, release tests, an MSRV job, an OS matrix
  (linux/macos/windows), `cargo deny check`, `cargo audit`, the no-network
  checks, the tripwires, generated-doc staleness, a README staleness check, and
  the calibration fixture.
- **Release workflow** on tag `v*`: five targets, SHA-256 checksums and build
  provenance attestations. It does not publish to crates.io.
- **Dependabot** for cargo and GitHub Actions, with updates grouped.
- **Issue templates** for bug reports and rule proposals, both of which refuse
  target names.
- **`deny.toml`**, **`SECURITY.md`**, **`CONTRIBUTING.md`**.

### Changed

- **`docs/calibration.md` rewritten for honesty.** The in-sample caveat now comes
  first instead of last. The "47% of the shortfall" claim is replaced by both
  figures with their denominators: **28 of 59 matched pairs (47%)** and **28 of
  45 non-hits (62%)**. The unmeasured false-positive rate is stated as
  unmeasured rather than left implied. Reproduction commands no longer point at
  private paths.
- Rule pages are the source of the "escape hatch" and "known false-positive"
  documentation; the README's catalog table becomes a summary that links to them.

### Removed

- Private-workspace path prefixes from the shipped catalog, the docs and the
  reproduction instructions. Each rule's source is now `doc: docs/rules/Pn.md`.

## [0.2.0] — 2026-10-09

### Added

- `--known-issues`: a user-supplied corpus of a program's known issues and
  prior-audit findings, matched against the draft finding. The honest status is
  **unvalidated** — see `docs/calibration.md`.
- `vg init`: scaffolds the four input files a new target needs, commented.
- The lenient calibration metric, reported alongside the strict one.
- A `viability-gate` binary alongside `vg`, because `vg` collides with an
  unrelated bioinformatics tool.

### Changed

- The scaffold no longer offers the removed `any` tier shape.

## [0.1.0] — 2026-10-08

### Added

- The CLI: 22 anti-patterns (P1–P22) and 6 pre-hunt gates (G1–G6).
- `check`, `gates`, `explain`, `rules`.
- The rule catalog and gate definitions as data, embedded in the binary.
- The fixture suite: one positive and one negative case per anti-pattern.

[Unreleased]: https://github.com/KhomDev/viability-gate/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/KhomDev/viability-gate/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/KhomDev/viability-gate/releases/tag/v0.1.0
