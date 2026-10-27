# Project-binding fixture timing independent review

Reviewed source `4e18b72371e395468804406cb3dece34c9a86e7b`, documentation HEAD `4459346f4f3c2a509f5392198f430c76ef49e3b5`, baseline `130f3639cdf20a15acd7a4ab448f6ac400fb8770`. Scope is the first App.test project-binding fixture only under AB-START-001/OWNER-001/002. No source or product modification by reviewer.

Verdict: no blocking finding. The alternate input method preserves this test's project-binding semantics and strengthens readiness/value evidence without weakening acceptance assertions. Ready for primary integration and fresh exact-head CI; no corrected macOS pass is inferred from this Linux run.

The exact diff is9 additions/1 deletion in App.test.tsx and execution documentation only. Product/frontend/native source, dependency and lock files, test configuration and5000ms default deadline are unchanged. There is no skip, arbitrary sleep, forced ready state, added timeout or removed original expectation. The test awaits the actual active project option, requires it enabled, selects through userEvent and explicitly checks the selected ID. The archived option disabled assertion remains. Clicking the actual goal control focuses it; userEvent.paste emits its normal input/onChange path into the controlled textarea. The exact displayed goal value is asserted, then the real form button is clicked and the original api.create(goal,key,project.id) expectation remains. This does not directly call setState, prefill transport state or bypass form submission. Per-key input performance is not the purpose of this project-binding case.

Independently compared the complete suffix starting at the second App test against130f: all other11 cases are byte-identical. Both user.type creation and uncertain-create idempotency tests remain. No project binding, archived filtering or actual product behavior was altered.

Read the full retained Intel job log, which is byte-identical to `/workspace/.onboarding/provider-130f-job-121921608984.log`: run40422958570/job121921608984 checked out actual merge `0c1663aec65f94cc95ea8b1a1b8221e29e593ff4` (PR head130f merged into mainc4e72c6). Its first App project case reports5164ms and a5000ms timeout; total48passed/1failed. This is the observed whole-case failure only. There is no step-level timing, no evidence assigning the delay specifically to typing and no local reproduction of the Mac performance failure. Previous ARM/other checks do not establish current candidate native acceptance.

Independent commands, cwd `/workspace/.worktrees/ui-fixture-timing/apps/desktop`, both exit0:

- `npm test -- src/App.test.tsx`:12 tests /1 file passed, original5000ms configuration.
- `npm run typecheck`:passed.

These are local Linux/jsdom/mocked Tauri-transport results. Worker full49/typecheck/build outputs were read and their hashes verified; this reviewer did not rerun all49 or build because the only source change is this isolated fixture and App12/typecheck independently cover it. No macOS/native/browser/real Provider/model execution by reviewer. The failure remains retained and must not be relabeled passed; new exact-commit protected CI is required after integration into Root/PR10/Data.

All1 source and6 worker raw-log SHA-256 entries in EVIDENCE.json recompute exactly, including the retained original-local pass and full Intel failure. Source fingerprint remained stable after independent execution. Reviewer created an exact temporary node_modules symlink to existing locked Root dependencies and removed only that symlink after the run; installed dependency contents and locks were untouched. Only this REVIEW.md and ignored independent logs were written, with no staging/commit/push or subagents.

## Source and evidence SHA-256

| Artifact | SHA-256 |
| --- | --- |
| `apps/desktop/src/App.test.tsx` | `194c6c8cc63ed1ab120bde3cdb9283de9d5d263e6198ab674f440d1dc9002e44` |
| `/workspace/.onboarding/ui-fixture-timing-app12.log` | `1bfaf675460463ba77bf07310994c4cc6cb893aaac33db34f46e6f63eaabd644` |
| `/workspace/.onboarding/ui-fixture-timing-build.log` | `1187f48c0adbc3c443ee82478c1f53a53abacb74eea79a88bf2708f2b7507c63` |
| `/workspace/.onboarding/ui-fixture-timing-intel-failure.log` | `bc996f66680ab2a679799822d62e970b0d2fe8182c0788bd44a7d022c2f2e2a2` |
| `/workspace/.onboarding/ui-fixture-timing-original-local.log` | `4d96ee4302e8501a3fdcbff2b1ed6ef6ed8ef4e5fd8edb09fdaae059c48818e7` |
| `/workspace/.onboarding/ui-fixture-timing-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/ui-fixture-timing-ui49.log` | `72cbad2e40c2154060d17bcc0cc35db404d5817f0a0b7178d5c659cb2bc57319` |
| `/workspace/.onboarding/ui-fixture-timing-independent/app12.log` | `3edc66a12ef41f9ae908d97345d958f92fa690675b286f9b6c00bf13ef59eeac` |
| `/workspace/.onboarding/ui-fixture-timing-independent/typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
