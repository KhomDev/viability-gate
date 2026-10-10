# Admin can zero the pause flag

The admin can set the pause flag to zero, which causes every vault operation to
halt. Only the owner may call this function, and the program's exclusion list
covers privileged-role findings.

Impact: total loss of availability for every depositor.
