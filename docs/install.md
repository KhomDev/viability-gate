# Install

Three ways, in order of how much you should trust them today.

## 1. From the repository (works now)

```bash
cargo install --git https://github.com/KhomDev/viability-gate
```

This installs **both** binaries, `vg` and `viability-gate`. They are the same
program; see [the name collision](#the-vg-name-collision) below.

Requires Rust 1.85 or newer (the MSRV is declared in `Cargo.toml` and checked in
CI). The floor is 1.85 rather than the edition year because the dependency graph
includes crates published under edition 2024, which older Cargo cannot even parse.

To install from a local checkout instead:

```bash
git clone https://github.com/KhomDev/viability-gate
cd viability-gate
cargo install --path .
```

## 2. From a release binary

Releases are published on tags matching `v*`. Each release carries prebuilt
binaries for Linux, macOS and Windows, a `SHA256SUMS` file, and a build
provenance attestation.

```bash
# pick the asset for your platform from the release page, then:
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS and Windows, `shasum -a 256` and `Get-FileHash -Algorithm SHA256` are
the equivalents. Verify the checksum before running the binary; that is what the
file is for.

The attestation (`gh attestation verify`) ties the binary to the workflow run
that built it. It proves the binary came from this repository's release
workflow — it does not prove the source is good, which is what
[`docs/calibration.md`](calibration.md) is for.

## 3. From crates.io

```bash
cargo install viability-gate
```

**Not yet available.** Publishing to crates.io is a deliberate stop gate: it is
irreversible, and the crate name is claimed permanently. Until it is done, use
one of the two methods above. The `cargo install viability-gate` line is in
`README.md` as the intended end state, not as a working instruction.

## The `vg` name collision

`vg` is already the name of an unrelated and widely packaged bioinformatics
tool — a variation-graph toolkit, distributed through Homebrew, conda and most
Linux distributions. If you have ever installed anything in that space, your
`PATH` may already have a `vg`, and `cargo install` will put this one somewhere
that shadows it or is shadowed by it.

That is why this crate ships **two binaries with the same code**:

| Binary | When to use it |
|---|---|
| `viability-gate` | Always safe. No known collision. Use this in scripts and CI |
| `vg` | Shorter, and matches every example in the docs. Use it if you have checked `which vg` |

```bash
which vg          # if this is not the tool you expect, use viability-gate
viability-gate check finding.md
```

Every command in the documentation takes either name.

## After installing

The rules are **embedded in the binary**, so there is no data directory to
populate and nothing to download. To point the tool at your own catalog instead:

```bash
vg --rules ./my-rules check finding.md
```

Start with:

```bash
vg init my-target/
```

which scaffolds the four input files a new target needs. See the
[`vg init` section of the README](../README.md#vg-init--start-here).

## Uninstall

```bash
cargo uninstall viability-gate
```

This removes both binaries, since they come from the same crate.
