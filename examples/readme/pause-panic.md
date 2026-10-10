# Critical: Permanent Fund Lock via Admin Pause Panic

## Summary

An admin can set the pause flag to zero, which causes every vault operation to
revert. The funds are permanently locked and unrecoverable.

## Impact

Complete loss of all funds for every depositor.
