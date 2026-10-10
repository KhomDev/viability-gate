# Unprivileged atomic drain via flash loan

Any caller can invoke this. The attack is atomic in one transaction via a flash
loan, and the PoC imports the real in-scope contract and asserts the balance
delta of 12.5 USDC.

Impact: the attacker profits 12.5 USDC and a third party loses their deposit.
