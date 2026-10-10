# Ecosystem token packs

One YAML file per ecosystem. Each pack declares the tokens, path globs and test
markers that are specific to that ecosystem.

```yaml
id: evm-solidity
ecosystem: "EVM / Solidity"
tokens:        [".sol", "foundry", ...]   # substrings unique to the ecosystem
path_globs:    ["**/*.sol", ...]          # where its source lives
test_markers:  ["forge test", ...]        # how its test suite looks
notes: >-                                 # why it exists, what it excludes
```

## Why this directory exists

A rule catalog that knows exactly one ecosystem's vocabulary tells every reader
which ecosystem its author hunts. That is a disclosure about the author, and it
is the kind of disclosure that cannot be un-published once a rule ships.

So the ecosystem-specific tokens were moved **out** of
[`rules/anti-patterns.yaml`](../anti-patterns.yaml) and into packs, and the
catalog refers to a pack instead of carrying the token. Because the pack set is
balanced, the **presence** of any one ecosystem's tokens says nothing about
which ecosystem the author hunts. It only says which ecosystems the catalog
covers.

## The balance requirement

Two properties are enforced by [`scripts/check_tripwires.py`](../../scripts/check_tripwires.py):

1. **At least 7 packs are present.** A catalog that covers one ecosystem is a
   disclosure; a catalog that covers eight is a catalog.
2. **No pack has dramatically more entries than the others.** Entries are
   `len(tokens) + len(path_globs) + len(test_markers)`. The check fails when
   the largest pack has more than **2×** the entries of the smallest. This is a
   coarse guard, not a precise one — it exists so that adding "just one more"
   Solidity token to a Solidity pack is visibly unbalanced rather than invisible.

A third check asserts that **no token declared by any pack appears in the
shipped catalog**. See `docs/tripwires.md` for the exact surface that check
covers and what it deliberately excludes.

## Adding a pack

1. Copy an existing pack and change every field.
2. Keep the entry count within the balance bound (the check will tell you).
3. Run `python scripts/check_tripwires.py`.
4. Do **not** add ecosystem tokens to `rules/anti-patterns.yaml` directly —
   that is the failure this directory exists to prevent.

## What is deliberately *not* in the packs

- **Generic syntax.** `assert!(` is Rust, but it is also the host language of
  this tool: it appears in ordinary source files. A token that fires on the
  tool's own code is not an ecosystem signal, so it is not a pack token.
- **Anything target-specific.** Packs describe ecosystems, never programs,
  chains-with-one-user, or repositories. The rule that no shipped file names a
  target applies here too.
