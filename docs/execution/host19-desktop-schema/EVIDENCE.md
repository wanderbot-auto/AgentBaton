# Verification

Base and fetched target 1f756ce201b8cf10f4b1798053674f4e960ac77d. Fixed source 4d120da1d45a7df3a7c191dda611d76d5fc9f21a. EVIDENCE.json records command, Git tree, lock, source and built asset hashes. Logs are archived with their original bytes.

| Command | Result | Log SHA-256 |
| --- | --- | --- |
| Isolated npm ci with the checked-in lock | Existing pinned set installed, 107 packages | 773c133596d25ba3736b29f309695189b5338b58dac20d3b8d7f84b643a5fbfc |
| Selected schema19 reads on test-only c3b41a6e | 4 regressions failed as expected, 67 unselected | 4bfc41d8afce4ad800e78feecf5bbb28b18b7d29f50ff8699146c187623e949b |
| Targeted Vitest | 12 files, 138 passed | 69bb0f81ef88e726fd2a059f1369d0f88f95dc450db486e5280b1000b459f7f7 |
| TypeScript check | Passed | 81a908ca176d0b18d9dd2470e07fabb394420707cf4a00e9b2f28551bdcdc175 |
| Vite build | Passed, 130 modules transformed | 072a0d97217aeaff289663ba73aca96ee97f8aa8f94d6ffc32bbe0636ccca139 |
| Full desktop Vitest | 108 files, 1548 passed | ac5a099d1b5e41e47814e50c7b6efd352e29cc91570bd295688bb8344aab9503 |
| Full infrastructure | 922 total, 906 passed, 16 existing skips | 0cccd5927238d1d7fc9127da5d0e3e4a8823c0fada21ae31d0b872cd0606d54e |

The renderer accepts numeric schema19 with numeric API1 in the current snapshot, closed background DTO and defensive Integration refresh guard. Tests keep unsupported earlier versions and add schema18, schema20, malformed version types and unsupported API controls. The cached lifecycle case proves previously enabled revoke and cancel actions become disabled on a mismatched refresh. It checks that no later contextual read or Integration write command is invoked. Current fixtures moved to 19 and unrelated historical mocked17 values stay in place.

The four production files exactly equal their base bytes after applying only the six agreed literal replacements. This proves that React parallel reads, parent fingerprint, request epoch cleanup, sequence and revision floors, lifecycle publication, polling timing, native approval boundaries and layout did not move. Vercel React checks applied to existing Promise.all and transient ref paths, with no new request, effect, dependency or memoization layer. The existing generic unsupported/stale warning behavior remains in use. The specific schema text resource is synchronized for English and Chinese.

Package manifests and locked dependency bytes are identical to the task base. npm reported that its package install-script policy did not yet cover fsevents. No lock or opt-in permission changed. The built JavaScript was 961.83 kB minified, 282.89 kB gzip. Vite reported the standard 500 kB bundle size warning. Asset SHA-256 values are in EVIDENCE.json and the generated web assets are retained in this worktree's ignored dist directory.

The wrapper and all owned test/build subprocesses returned naturally. The validation dimension is Vitest/jsdom with mocked Tauri transport and an actual web asset build. Actual native app IPC, signed packaging, Provider/remote and hosted CI are out of scope.

Evidence log archive SHA-256 43f25bedab6b45bf93969e20fead0e2a707775a525c7d455b6e5a288549fa618.
