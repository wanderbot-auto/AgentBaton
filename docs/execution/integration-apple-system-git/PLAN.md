# macOS integration system Git

Base: `426cd1e530f8894cb38fff6c0386ae0451dea478`. Scope: AB-VAL-009/010/011 integration and supported macOS behavior, not a platform acceptance claim.

1. Preserve the real ARM failure and identify the two incompatible assumptions: PATH selected user-owned Homebrew Git; Apple `/usr/bin/git` had 78 links and failed the generic single-link guard.
2. Select the fixed Apple entry point for both Core observations and actual workspace capture/adoption/CAS calls. Share child environment policy. Preserve Linux PATH behavior.
3. Keep ordinary leaves single-linked. Add only an Apple-specific held-descriptor proof of a readonly local root APFS snapshot, including same-volume, root-owned non-writable ancestors and complete rechecks.
4. Add policy/refusal and command-construction unit tests. Run only source preservation checks and rustfmt here. Root performs independent review, compilation, Linux tests and real Apple Silicon/Intel CI.

No lockfile, migration, protocol, timeout, authority, budget, approval, Provider, signing or release changes. No global environment or Xcode configuration changes.

5. A test-only follow-up adds one actual macOS descriptor/volume diagnostic. It opens only the fixed Apple entry point and its three system ancestors, validates the production proof and `system_git` held pin, and never starts Git. The existing 11 tests and all product sources remain byte-identical to candidate f379e0a. Root's existing CI is the only intended actual Mac executor.
