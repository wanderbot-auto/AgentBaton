# Local foundation integration and version-probe backport

Registry base `4e1efd3` was clean before this unit. Read and ACKed OWNER-002 from protected main `2d9361d`; retained the Quiet Canvas reference without importing prototype records or claiming its implementation here.

Merged foundation `222aafd` locally as `57f4665`. Only STATUS and BOOTSTRAP_MANIFEST conflicted. The manifest is the 92-path union (registry87/foundation92); all non-STATUS contents matched one source hash, OWNER_INBOX matched the approved foundation version, and the combined STATUS preserves both prior reports under explicitly historical headings. No source, test, dependency lock, CI or work-in-progress registry file was replaced to resolve those conflicts.

Inherited foundation fixes include SQLite preparation-fd close before Store::open, numeric-only database diagnostics, portable canonical stdio fixture workspaces, readiness waits and no scheduler handoff after useful transport progress. This is inherited reviewed code, not proof of the historical ARM failure's root cause.

Backport `aa258a6` carries only d7acacb's installation-probe progress-aware idle wait and its readiness helper; it does not overwrite the registry transport with the newer session transport. Actual useful reads continue immediately. Only idle iterations poll; descriptors already at EOF are excluded, avoiding permanent HUP readiness. The existing 500ms deadline,4MiB stderr volume and all tests remain. A new Linux4KiB-pipe fixture proves the same probe bounds: original fixed-sleep production timed out under the new test (exit101), corrected production passed. Failure evidence is retained; the initial formatting-only check failure was fixed without changing assertions or bounds.

At `aa258a6`:123 Rust tests (core34/config26/providers50/workspaces13), workspace fmt/Clippy,15 transport-mocked UI tests/typecheck/build, real daemon/CLI restart smoke,92-file integrity check, design/infrastructure validation and28 Python infrastructure tests passed. Exact logs/source hashes are in EVIDENCE.json; logs are local ignored artifacts.

No push, main merge, new-candidate CI, native registry macOS build, real Provider account/model test or full product acceptance is claimed. Parent independently reviews, pushes and retargets the existing draft PR#4 after the foundation prerequisite is integrated. Full Designv0.2 scope remains; OWNER-002 redesign and session/runtime work proceed in their dedicated branches.
