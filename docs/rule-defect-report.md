# Reproduced rule defects, and the fixes

Nine rule defects were reported by reading the regexes rather than by running them. This page records
which of them were **actually reproduced** with a failing fixture, and which were not. A defect that
did not reproduce was left alone.

Every reproduced defect has a permanent regression fixture in
[tests/rules.fixture.yaml](../tests/rules.fixture.yaml) holding the exact counterexample text, and
the integration test drives the real engine rather than a re-implementation of it. Reverting a fix
fails cargo test --release.

| Rule | Defect | Reproduced? | Fix |
|---|---|---|---|
| P19 | Fires whenever a loss claim and a revert co-occur anywhere | **yes** | Structural matcher |
| P15 | Bare "documented behaviour" hard-kills "contradicts the documented behaviour" | **yes** | Contradiction suppression + escalation |
| P18 | Bare "prior audit" fires hard on "the prior audit missed this" | **yes** | Bare phrase removed; missed/did-not-cover suppression |
| P4 | Case-insensitive [A-Z] matches any letter; MockERC20 is scaffolding | **yes** | Case-sensitive group, word boundaries, allowlist, warn by default |
| P2 | One "permissionless" anywhere switches the rule off; only(owner...) misses "only the owner" | **yes** | Sentence-scoped suppression; prose forms added |
| P5 | Matches the PoC's own path | **yes** | ignore_sections: [poc] |
| P8 | Fires on the standard heading "Severity rationale" | **yes** | Pattern removed |
| P1 | Hard-kills the single word "hypothetical" | **yes** | Downgraded to a note; explicit admissions stay hard |
| P3, P1, P15 | No negation handling | **yes** | Engine-level 6-token negation window |

Nine of nine reproduced. None was left alone.

---

## P19 - headline vs proof, redesigned as structural

**Reproduced.** The old rule was an any_of loss claim with an all_of revert somewhere in the
document. The counterexample that proves the defect is a **valid** revert-based freeze: the headline
says "permanent fund lock", the proof shows the call reverts, and the report correctly asserts that a
committed storage slot keeps the funds stranded. The old rule hard-failed it, which is exactly the
SPAM boundary it was supposed to reserve for fabricated impact.

**Fix.** A hand-written structural matcher (engine::structural_match):

1. The **headline** section (before the first PoC/proof section) claims permanent or total loss.
2. The **proof** section shows only a revert, panic, abort or expected failure.
3. The document asserts **no persisted state** (no "persist", "storage", "after the call returns",
   "committed", "survives the").

When all three hold, the proof disproves the title and the rule is **hard**. When a persisted
consequence is asserted, the rule still fires but as a **note**: the claim may be true, and a human
should look. When the proof shows no failure at all, the rule is **silent**.

This required the section splitter to keep the heading line inside the section it introduces. A
finding's headline *is* its top heading; discarding it meant the loss claim was never seen at all.

## P15 - "documented behaviour" is not proof of intent

**Reproduced.** "The implementation contradicts the documented behaviour" is the strongest form of
the argument, and it contains the phrase "documented behaviour", which the old rule treated as a hard
kill.

**Fix.** The bare pattern is gone from any_of. none_of now suppresses the rule on the contradiction
forms (contradicts / violates / diverges from / inconsistent with the documented). The rule is warn by
default and escalates to hard only when the finding couples the claim to the project's own tests.

## P18 - "the prior audit missed this" is the opposite signal

**Reproduced.** The bare phrase "prior audit" hard-killed a report whose entire point was that the
prior audit did not cover the surface.

**Fix.** The bare phrase is removed. A match now requires a previously/already reported-or-identified
statement or a named source cue. none_of suppresses on missed, did not cover, does not cover, never
covered.

## P4 - [A-Z] under a case-insensitive compile

**Reproduced.** A (?i) compile makes [A-Z] match any letter, so "stubborn" and "mocked" matched
(Mock|Simulator|Fake|Stub)[A-Z]\w*. Separately, MockERC20 is ordinary PoC scaffolding and was being
treated as a defective PoC.

**Fix.** The identifier alternation is wrapped in a case-sensitive group with word boundaries. A
downgrade_on list drops the ordinary scaffolding tokens (MockERC20, MockToken, MockVault, Stub*), so
a bare mention never fires. The rule is warn by default and escalates to hard only on explicit
"instead of the real contract" / "simulates the contract" / "not the real contract" language.

## P2 - document-wide suppression and a missed prose form

**Reproduced, twice.**

1. only(owner|admin|...) requires the identifier to be glued to the word "only", so **"only the
   owner" was not matched at all**. Added prose forms.
2. none_of was document-scoped, so a single "permissionless" **anywhere in the document** switched
   the rule off - even in an unrelated sentence. The escape hatch now has to apply to the same
   sentence that named the privileged role, via none_of_scope: sentence, and it must be evidence that
   an unprivileged path exists.

## P5 - the PoC's own path

**Reproduced.** The path pattern matched the finding's own test file, so citing your harness killed
the report.

**Fix.** ignore_sections: [poc] - the rule does not evaluate paths inside a proof-of-concept or test
section. The over-broad "root cause is in" pattern was narrowed to "root cause is outside", because
the original also matched the in-scope case it was meant to allow.

## P8 - the standard heading

**Reproduced.** "Severity rationale" is what a *good* report calls that section, and the rule fired
on the heading alone.

**Fix.** The pattern is removed.

## P1 - "hypothetical"

**Reproduced.** The single word "hypothetical" hard-killed a report that was merely being careful
about a demonstrated mechanism.

**Fix.** The pattern is removed from any_of and the rule is warn by default. It escalates to hard on
the explicit admissions ("no current exploit path", "not currently exploitable"), which are the forms
that actually mean the impact does not exist.

## P3, P1, P15 and others - no negation handling

**Reproduced.** "There is no front-run exposure in this path" fired P3.

**Fix.** The engine evaluates patterns **sentence by sentence** and discards a match when a negation
cue (no, not, never, without, n't, neither) appears within the **6 tokens immediately before** it.
Rules that already handle negation through none_of can opt out with negation_aware: false, so the two
mechanisms do not fight.

---

## What this does not fix

The negation window is a heuristic. A cue further than six tokens back does not suppress a match, and
a cue in a different clause of the same sentence may suppress one it should not. Both behaviours are
asserted by tests, so they are known and intentional rather than accidental, but neither is correct
in every case. This is a text matcher over prose; see the Known limitations section of the README.

No claim here is a measurement of accuracy. The false-positive rate is unmeasured, because the
author's log contains no paid findings. See [control-set.md](control-set.md) and
[prospective-protocol.md](prospective-protocol.md).
