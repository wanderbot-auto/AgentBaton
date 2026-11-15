# Owner withdrawal with actual target drift

AB-VAL-009 / AB-PRJ-004/005, D18/D19. Extend the real bounded-three-round private UnixIPC/generated Git fixture with original Owner cancel and policy revocation while a real validator is held at its approved FIFO and the selected target has actually advanced. Preserve complete previous source prefix and all production limits.

Do not add a production test-control mechanism. The private stopped-command target postprobe has no reliable external gate; these tests intentionally verify running-validator withdrawal with real target drift, not withdrawal after stop or during postprobe queuing.
