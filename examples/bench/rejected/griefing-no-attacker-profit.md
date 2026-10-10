# Withdrawal griefing

An attacker can repeatedly call the withdrawal path with a dust amount and force
the next user's call to consume extra gas. This is a griefing vector with no fund
loss and no attacker profit: the funds stay in the contract and no financial
beneficiary exists.

Severity Rationale: this might be considered out of scope, however the impact is
a denial of service.
