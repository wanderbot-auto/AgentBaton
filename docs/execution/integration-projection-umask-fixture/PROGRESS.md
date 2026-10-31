# Actual progress

Original ba4612a: Cloud umask0077 observed. All18 pure projector units pass under0077; under0022 only test_complete_pair fails (17pass/1fail), reproducing the Environment symptom. Direct collect proves the generated intermediate private/driver is0755, while its normal leaf is0700; the original descriptor guard correctly rejects it.

The fixture now creates driver0700 explicitly. All19 pure units pass under both0022 and0077, including a new two-umask positive and an added0755 driver negative variant. A mutation that restores0755 is rejected by the original positive plus both new umask subcases (three failed assertions, as expected). Test output uses generated black PNGs, not screenshots or visual acceptance.

No projector, shared path reader, source/hash/cleanup/PNG guard or product source changed. CI logs do not print actual runner umask, so the local reproduction establishes the fixture defect; exact new three-platform CI remains necessary.
