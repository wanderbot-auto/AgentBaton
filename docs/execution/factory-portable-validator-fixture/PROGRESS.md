# Progress

Implemented only fixture setup. At function entry the canonical system shell is opened with NOFOLLOW/NONBLOCK/CLOEXEC; held FD and path full metadata pins, root ownership, executable/non-setid/non-group-world-write mode, nlink1, bounded size and SHA equality are asserted. Existing library still performs its complete runtime checks.

After real bootstrap stop, create_new0600 writes the113-byte fixed script under the already ignored reports/ directory. Its exact SHA requirement is supplied in the normal private Owner proposal. The returned policy must retain original Factory inputs and exact script workspace root/device/inode/requirement. Final source checks retain script SHA and original system program FD/path pins. No generic constructor or generated ACP logic changed.

19 finite Linux controls passed:17 text cases match grep reference including invalid/valid, no final newline, multiline, whitespace, backslash, CRLF and Unicode; missing file never succeeds; generated hardlink remains refused by the same nlink predicate. This is shell text semantics, not Core execution, general binary grep equivalence, Mac or actor responsiveness.

Mechanical inverse of only approved setup changes restores the entire original fixture byte-for-byte, including all observer/failure/Owner/recovery/Graph/Grant/parent/ledger assertions. Direct rustfmt --check and git diff --check passed. Rust typecheck, actual normal/failure rework, Factory26, new Mac and CI have NOT_RUN here.

Source `5adb52ee38ce4d82d45b386df9c952687577cca2`; base `de0b911328c6703491902970ff608b908e3bf643`; branch `codex/factory-portable-validator-fixture`; isolated tree `/workspace/.worktrees/factory-portable-validator-fixture`. Only `crates/core/tests/fixtures/factory_child_goal_rework.rs` changed. Generated ACP source remains byte-equal. Source manifest `/workspace/.onboarding/factory-portable-validator-fixture/SOURCE_MANIFEST.json`, SHA `5d2c9c8c2a9d706ddb8abe38bb3655d14fe88e9e678701974bf05aa7158f54a9`. All original de0b logs/safe JSON remain in place.


## Linux ownership-assertion correction (02503b4)

Root integrated original5adb in2502102 and actually compiled it, then the exact single rework test failed101 before Actor startup:0passed/1failed in0.00s at the unconditional fixture uid0 assertion (observed65534, expected0). Raw `/workspace/.onboarding/pr18-portable-validator-single1/RESULT.json` and command.log are preserved; this is a new real fixture failure, not a successful portable flow. Root reports the test executor is uid1000 and Cloud system dash/grep are uid65534/nlink1/mode755.

Production `protected_program` and `protected_program_view` in workspaces/command_validation.rs917–925 are explicitly macOS-only. Linux uses the sealed-image branch and does not require the system executable owner to be root. Source `02503b417d5848b1d2d3350678d774285287ff1a` therefore adds exactly one `#[cfg(target_os = "macos")]` to this fixture's uid0 assertion. This corrects a test-only platform requirement; no production guard is relaxed, no fake-root execution is used, and all Mac ownership/ancestor enforcement remains. All other singlelink, mode, root/path/heldFD, size, SHA, script, deadline, budget and final assertions are byte-equal.

One-line reverse normalization restores the complete prior fixture, and production/provider/Cargo diff is empty. Direct rustfmt --check and git diff --check pass. The113B script and exact argv remain byte-equal, so prior19 Linux semantic controls stay their original historical dimension; they were not rerun or presented as current Rust validation. Author ran no Cargo/Actor/daemon/Mac/new CI. Root-owned exact actual retry and independent one-line review remain pending. Original ARM preflight and Intel4359ms Work overrun facts remain unclosed, and no Mac grep hardlink cause is declared.

New source manifest `/workspace/.onboarding/factory-portable-validator-platform-correction/SOURCE_MANIFEST.json`, SHA `0455bc3c0c3728af401f2fd373ab047733c9dd2ac17b689540b7c7246415243c`. Old5adb/b03 source,19controls and manifests are unchanged. A documentation formatter's literal-brace error occurred before any writes and is separately recorded; only documentation formatting was retried.
