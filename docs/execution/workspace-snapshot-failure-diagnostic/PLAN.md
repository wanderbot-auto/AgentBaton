# Readonly workspace snapshot failure context

Support AB-PRJ-002/003/005 and AB-REL-001 by preserving the original ten real Core tests and all source/DB/key/head safety checks. Add failure-only closed diagnostic context for the observed f744 ARM ENOENT; do not infer its root cause, skip .git, tolerate disappearing files or alter product behavior. Independently review, run the existing tests, then integrate with the material product fixes before exact remote CI.
