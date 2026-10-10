# Permanent fund lock in the emergency exit

Titled impact: a permanent fund lock. The stranded balance remains in the vault
after the call returns, and the PoC asserts that delta against a fresh clone at
the pinned commit. The attacker's own attempt reverts, but the depositor's funds
are permanently locked regardless.

Impact: permanent lock of the depositor's principal, demonstrated below.
