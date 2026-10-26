# Independent review · product foundation

Reviewer: native `/root/review` execution context, separate from the implementing workers. Base: `e158ade1a99e063ddce703ce4c0748febe449b43`; feature branch `codex/product-foundation`. This report reviews the working-tree file hashes below; it is not a claim that an uncommitted candidate passed remote CI.

## Verdict

No remaining blocking defect identified in the reviewed foundation slice after the corrections below. Suitable to submit for PR/real CI; merge remains conditional on exact-candidate required checks, integration verification and resolution of any subsequent findings. This is not acceptance of any complete work package, complete product, real Provider integration, or native desktop installation.

## Findings resolved during independent review

1. **High: repository import could execute configured Git clean/process filters.** `inspect` ran `git status` before filter rejection. Independently reproduced with a temporary Git repository whose clean filter created a marker, even with fsmonitor/hooks/global attributes disabled. Workspace implementation now rejects filters before status, ignores submodule traversal and treats gitlinks conservatively; regression verifies neither import nor isolated checkout runs that marker. Descriptor-based artifact reads, dirty-source protection and exact-head adoption checks reviewed.
2. **High: configuration ChangeSet could target the wrong managed connection root.** A plan made against root A could be applied to root B containing identical baseline bytes. ChangeSet now pins descriptor device/inode; apply rejects another root; recovery/rollback also reject transplanted journals. Both negative regression tests pass.
3. **Medium: stale daemon socket prevented desktop recovery after a crash.** Desktop now probes socket liveness, restarts only on absence/refusal and authenticates a live daemon. It does not bypass authentication/version/permission errors. A monotonic readiness deadline bounds its read-only authentication probe; no repeated five-second probes behind a nominal five-second loop.
4. **Medium: UI event wire contract and paging hid recent history.** UI now uses `next_cursor/sequence/gap`, starts with the latest 100 records, offers explicit earlier paging and visibly caps the in-memory history window. A 230-event fixture regression passes; fixture UI verification is distinct from native IPC.
5. **Medium: completed/idle ACP session IDs remained usable across transport reconnect without restoration.** All old associations are now detached on disconnect; controls/update normalization and delayed prompt results reject them until trusted session establishment. Active side effects remain unknown; no prompt replay. Added idle/completed reconnect regression passes.
6. **Medium: task snapshot silently truncated at 1000 records.** Core now returns an explicit resource-limit error rather than claiming an incomplete snapshot is complete. Pagination remains a recorded implementation limitation.
7. **Low: unauthenticated malformed requests exposed journal revision.** Error revision is now zero without successful authentication. Negative raw-socket tests pass.
8. **Validation discovery regression:** ignored installed dependency Markdown was interpreted as project documentation. Reviewed change uses Git's authored-file discovery while preserving link, reference and schema assertions. Regression covers ignored dependency files and rejects an authored broken link. The original reference manifest and design requirements/decisions remain unchanged; changing the active bootstrap validator hash is a reviewed tooling update.

## Independent verification actually run

- `cargo test --workspace --locked`: initial tree, 37 tests passed before later additions. Superseded for changed paths by the targeted runs below.
- `cargo test -p agentbaton-workspaces --locked`: latest workspace library, 13 safety tests passed.
- `cargo test -p agentbaton-core -p agentbaton-config -p agentbaton-providers --locked`: reviewed core/config and then-current protocol implementations, 61 tests passed (core 18, config 14, provider contracts 19 plus real fixture-child stdio 10). Included storage rollback, schema-v1 private backup/migration, unknown-outcome recovery, immutable dispatch binding, budget reservation rollback, authentication/frame limits, cross-root configuration rejection, corrupted backups, timeout/EOF/malformed provider output, explicit environment isolation, denied permissions and owned child-group cleanup.
- `cargo test -p agentbaton-providers --locked`: final protocol follow-up, 31 tests passed (20 contracts + 11 real fixture-child stdio). Supersedes the earlier 29 provider tests after adding delayed old-prompt-result rejection and terminating/reaping malformed session creation. No additional blocker found in final bounded transport review.
- `npm test -- --reporter=dot` in `apps/desktop`: 6 UI tests passed. The Tauri transport is mocked in these tests; no claim of real native desktop execution.

The parent independently owns integrated fmt/clippy/typecheck/build/infrastructure checks, real batond/CLI crash smoke, exact commit capture and CI results. Those results must be recorded separately; this reviewer does not turn their command names into independent native evidence.

## Remaining limits and follow-up

- Actual macOS host compilation, Tauri IPC execution, native UI lifecycle, both development DMGs and installation acceptance still require real CI/platform evidence. Workflow structure and packaging paths were reviewed statically; no macOS result is inferred from Linux.
- Provider subprocess tests run controlled local fixtures. No real model/account was called. Program approval references, environment values and executor stop receipts must be resolved by trusted policy/supervision before exposing these primitives through product APIs.
- Config/workspace/protocol primitives are not yet a complete connected scheduler/capability/Profile product. No OS filesystem/network sandbox is implied. Provider children escaping their owned process group need stronger OS supervision.
- Managed configuration writes use conditional hash checks and cooperative locks; a same-user external edit in the final hash-to-rename interval cannot be excluded by this filesystem primitive. This residual is documented; do not claim a filesystem compare-and-swap guarantee. Root identity binding intentionally refuses cross-filesystem/copied-journal reuse until an explicit trusted restoration flow exists.
- Task listing currently refuses more than 1000 records; UI retains at most 500 events at once with visible explanation. Full paginated retrieval and retention behavior remain incomplete.
- Authentication/private runtime state is intended to restrict other local users, not fully defend against malicious code already controlling the same account. Full permission, secret-redaction, remote-host, cancellation/supervisor and lifecycle validation remain future applicable work.

## Exact reviewed files

SHA-256 hashes are from the reviewed working tree. Review evidence does not cover later edits to these files without follow-up.

| File | SHA-256 |
| --- | --- |
| `.github/workflows/product.yml` | `da485c95b5a47bb9b3ef89649506c6d8ea4f8bc60bfe7cf5b9b76e7b809daa43` |
| `Cargo.lock` | `cfafaa1ead29450f030a6cae7adb11bb2881eac0056b36e21462a0ff874134ec` |
| `Cargo.toml` | `5dc88c14683dc23feac5ee1d6076ee96fc8f4a8eb149fa62cf35818553522c29` |
| `apps/cli/Cargo.toml` | `279e279bd616d2e034f11808fb9db0020cab9276afe78de8f23c935f7d43fed1` |
| `apps/cli/src/main.rs` | `e0f32e1d09166a3268243303dab65c676d318ab084b18d001b1907a1c5d0605d` |
| `apps/daemon/Cargo.toml` | `2e4e4352ec1011281766ffd65af22de4a65220b14b57c2a4dd1a1c098912934c` |
| `apps/daemon/src/main.rs` | `6e8beced1b16efd3a548e01a93bcd115c2719aebd28e5d40a16c12ea755e211b` |
| `apps/desktop/.gitignore` | `5479e45ae49f8c0127f6d9a24d5bfae1ab1b919a53588d137ebd852fb0679572` |
| `apps/desktop/README.md` | `8b067db5191dcc42c431717e186c260f1708d097243a250c133138ffaa5b3779` |
| `apps/desktop/index.html` | `9ee1d18aa84bf8ea7f8e8e080580c698ce38d17de139a16b58fe7f972ea167b8` |
| `apps/desktop/package-lock.json` | `58ab2fdd69b78db49a8db12fb22a6fd62846978a6af4d590d741ac303068856d` |
| `apps/desktop/package.json` | `62b1f3ad51ffbedd2dafd7ed30ad3848740d22c74fc4ad1e1dea291ec257e60b` |
| `apps/desktop/src-tauri/Cargo.lock` | `9b52d30ab9bb29f14f0e420ff6b10204d03211504c1883c70ccec9cba004895c` |
| `apps/desktop/src-tauri/Cargo.toml` | `06e94023ef2bd51b2b67dc33411e64f3adde7b5bc4b018230adf1ca0ad25b8ca` |
| `apps/desktop/src-tauri/build.rs` | `487059eaf8a947b80f20a9aacac038a5047b2ad69d2401b827376c67d6fe847f` |
| `apps/desktop/src-tauri/capabilities/main.json` | `090491e6dce371a7853acf44c4a65c38e2e372ac50df6a5e313a36a5ec8acf9a` |
| `apps/desktop/src-tauri/icons/icon.icns` | `c06b01170670fb61780e9b79b6e4c2a3bb54834649d2eb5ccd2888086fe5fa32` |
| `apps/desktop/src-tauri/icons/icon.png` | `a952c1ccc3361e9737a3be643ac4010b4200b6f8adf151fec86fac55778cb145` |
| `apps/desktop/src-tauri/icons/source.svg` | `b4b06b7cfef74e286db54d8896538f81fd47d862b03fe6127ffa3406360f515d` |
| `apps/desktop/src-tauri/src/main.rs` | `a47e7115c93e1ca034768ada6ad1f0ffe8e66d850f4d346ced740fc6738d35b6` |
| `apps/desktop/src-tauri/tauri.conf.json` | `ef186a4ea2196f98306259c85577fae86dea20044cb448c16a2eabf336c42ed9` |
| `apps/desktop/src/App.test.tsx` | `e823d33abf8fba8ab397cccde003b9f87f1932f992567a0d7d31a2d718eebe9f` |
| `apps/desktop/src/App.tsx` | `880d59ff1f7342b0e3effbd30ed41bb1f9f65ec65b4c981812ddd11252f5dc61` |
| `apps/desktop/src/api.ts` | `13ee409aae2f0075512a40ee27c8de4536a6c632fb3a7ad277e4cd0d24d80c7e` |
| `apps/desktop/src/i18n.ts` | `3754639a225bcc3ba8ec9de05d28a79b6fd5a523cc16f84223c85029a79cb9e1` |
| `apps/desktop/src/main.tsx` | `47bbb595a850847154dcb069ad195c21151fa6393059c1a0b7c25d9e46fd8498` |
| `apps/desktop/src/style.css` | `49690e781c8f1611658b3f2c672ac40986c4f1d2de63dfe83a9b2d993e9160c5` |
| `apps/desktop/src/test-setup.ts` | `ae4b6b92c8c0f06526032b2dedeb4fb0e93b85af2a138a423386f2e36894a685` |
| `apps/desktop/tsconfig.json` | `1926d10768671b06cebccef9170ad9fd808f9507d5da2c6c859da223589d9a46` |
| `apps/desktop/vite.config.ts` | `24034b497fe996d776c30c5cdb4329eb4c29d0971e2e37962c1fb553cd7f5046` |
| `crates/config/Cargo.toml` | `c8ed7e937b0e4f498fac85f682faab9e2a810d37b2a02fc83c6694be823a7cc7` |
| `crates/config/README.md` | `9d880634d0d555286e0e06eed417a4984f2751da4849ffaae6992138005c921f` |
| `crates/config/src/lib.rs` | `2950b2a440b8ee999abc551538d2a5a56cb973de4b71e6d9faf8b05bf7de1260` |
| `crates/config/tests/transactions.rs` | `efb15cab097e5751557e665e3d846e31641a813adbe1604ec39e5318cb768212` |
| `crates/core/Cargo.toml` | `1d0b11e403359fa4089bc81f7a3f3af233dbe59e50e4affdaa1e58a6e5616263` |
| `crates/core/src/execution.rs` | `21118edcb3ddceb33d9bad1dc10a3b670c862066ca39d5b4478bd3be8d67d19e` |
| `crates/core/src/ipc.rs` | `f384690b168a75dc1db9b0a9c86742593857357ed8d5769ecca5d23ffdf41c5c` |
| `crates/core/src/lib.rs` | `caf6551e018357e6ce7ec6a30d2713f7abc459ba420c354abf3602e6a3d5924e` |
| `crates/core/src/storage.rs` | `7d7c6d5e4e8d0c8296fb03e28c39a87339a189a4fcbe70bd5df3c2fc77d8fc04` |
| `crates/core/tests/execution.rs` | `d4b60bba9c9dbb87d812444862b205b5c8cb5b14a35802590177ca7bac57561e` |
| `crates/core/tests/ipc.rs` | `f5a9adfcb9a069c894896b6cf83a5dfeebdc3d0218e8a05ca62a1bbbb258a8c1` |
| `crates/core/tests/storage.rs` | `19a031ba383cccba3a5207412d0e150b38b5967864c1eac64547e2547eb03783` |
| `crates/providers/Cargo.toml` | `4f9e973c693f7f979e907f433d2f193bb5bc370c0f821ceccfa8fdf7950de209` |
| `crates/providers/EVIDENCE.json` | `7c12b3fdda5e3b51251c04b15bbd60916c11aa5126d77b18b81e9e92713f687d` |
| `crates/providers/README.md` | `4d4ca39917ae6278e445bf0e523629e5e66b6ac3b47c62d5648b0601b1bf3a33` |
| `crates/providers/src/framing.rs` | `22b56ac5349f5161fcef4721312131e63d7d83e67c5084e7395098a1bc6411f0` |
| `crates/providers/src/lib.rs` | `cf443f52971696bdafa44f196fe8c2082aeb3a3ac1e0f959d6a9c6e7bab7c6c1` |
| `crates/providers/src/transport.rs` | `5476cf03ae4035201b2c25e31a84cd05bc103710b9f602592127bc537ddd4e92` |
| `crates/providers/tests/contracts.rs` | `9a4d3f1a5f5a059e7b446b2d143b5f4ba990deb1bc8c49b399ff5ee31c734c38` |
| `crates/providers/tests/stdio.rs` | `db1525d7cb3e6dd29b531069ea5a2081b03e34ab63b1d962b6870992b597460a` |
| `crates/workspaces/Cargo.toml` | `599abfb08517918c7322380d2279a87a45d8b596ba321fd2ae65056b4fff3448` |
| `crates/workspaces/README.md` | `129c3d96e8342a3a8c577f488e76fe06c0584b0f94d7b350ceb8dd1894f168df` |
| `crates/workspaces/src/lib.rs` | `d441ae6b9d8747b228871097f2031dbb8b6674a1de208f5f63ef94ca2c96b99f` |
| `crates/workspaces/tests/safety.rs` | `733b25d5a1c8e242595bd676a8cf90ffb066e4cabe7d41290cc336bbccdc42fe` |
| `scripts/package-development-macos.sh` | `5f2234ec7e50d5a6d7dd71d22d4115b85d2a74c6f653b32877fd715a029e33d6` |
| `scripts/prepare-desktop-sidecars.sh` | `13cf3955cf320fa9feb1ee8682371b0a81b5adf56e387975dcb7d43038c9f452` |
| `scripts/smoke-local-product.py` | `5f05c2c074f30966cba029f8e80faf7ca4b96e7c17c2fcbab1fb0f1d606c23f4` |
| `scripts/validate_design.py` | `23427936c31e292b46ea6596d0523873e31e26962a71ca27041654ed3965b850` |
| `tests/infrastructure/test_design_discovery.py` | `077925c8da41255a42b60b8f91d15daed3e3f6702635ce227bf90f10677cad64` |

## Follow-up: macOS oversized-frame fixture

Independent reviewer approved the test-only correction after inspecting both failed CI job logs: send exactly MAX_FRAME+1 bytes without an unread trailing newline. RESOURCE_LIMIT and authenticated daemon liveness assertions remain. Five local IPC tests passed; new native CI required. Reviewed ipc test SHA-256: cd84c80bc4b66dfa59b4b3d3814a4251b4f47026dd102b9aa3501ed755e81118.
