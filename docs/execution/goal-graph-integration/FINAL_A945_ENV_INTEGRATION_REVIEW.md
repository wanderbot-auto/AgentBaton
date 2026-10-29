# PR16 Environment60 final delta integration review

**SOURCE / RECORDS READY for an accurate new seven-check CI run; no must-fix found in this delta.** This is independent source/evidence review, not formal GitHub Approve or permission to merge. Candidate **`a94517ce373e279a29176ab8ab4f210897ea1a9b`**, clean `/workspace/.worktrees/goal-graph-integration`, compared with `bb343be5bb0403e995a97441ce16670c03bd0f33`. No remote request or mutation was made.

## Exact change and negative-test preservation

The only nonmetadata change is `tests/infrastructure/test_runtime_policy.py`. Reversing these **three exact replacements restores the entire old file byte-for-byte**: positive test name45→60; exact actual desktop expectation45→60; first invalid upper boundary46→61. All six functions remain, including the other negative values120/0/-1, exact workflow/job selectors, generic30 with31/45 refusal, runner/matrix/concurrency/fail-fast mutation refusal and malformed/dynamic/type rejection. The change follows the already approved finite60-minute desktop policy.

Workflow files, infrastructure validator, all Product/Core/Native/UI source, dependency locks and the130-requirement/27-decision baseline are byte-identical to bb. Product operation/per-test limits are unchanged. BOOTSTRAP keeps the same93 paths: only this test hash and STATUS hash differ; **the other91 values remain identical**, and all93 final registered hashes independently match immutable a945 Git bytes. Final test hash0474127b…097de; STATUS ad10d239…e2305. STATUS explains the old failure, source-equivalent local fix and need for new exact CI; it does not claim the new head remotely green.

## Complete local Environment command evidence

Revalidated immutable prior reports/manifests and their archive members:

- NonRust source9d11a6d: review `9bb690256ce8df28585f06ddc8a944a7c301d9af43baf5845743dc809f2f16cd`, manifest `380f5fa59e59c660fbae43cca78952ff7121e105a77efeba409566ba9e536fc5`;47-member archive `f2de730ae1cd7088d41b42aabdc3bf3a9785d6318e5279a16728366a1a3ca219`.
- Full-chain records89d88fc / actual Root Rust3a91bcec: review `c7b4352905cbd9fb3e63760a00efae79016603f0dc054c8599caa6d91fca43e5`, manifest `25509cf92efad610d6faf771a827f99ee6455529469abf9272b4eb5dbc01733d`;23-member archive `42cc2e577effb12a6e03881f461bcae0aca5fd7b8a1ee44cd157621489c479dd`.

All **21 critical workflow/test/validator/fixture inputs** are Git-byte/hash/size equal at a945,9d and actual Rust3a91. Thus source equivalence is explicit; commands are not falsely described as all executed on a945. Rust original `commands.json`/environment/raw stdout/stderr show exact locked environment-smoke test and all-target Clippy-Dwarnings commands, both exit0, correct dedicated target/cwd/time and recorded3a91 source. Two fixture tests passed with0failed/ignored/filtered; real compile/test/Clippy completion is present. Before/after tracked snapshots agree over1296 entries. Reviewer read records only, never accessed or wrote any target or invoked Cargo.

Root's final eight fresh records/log hashes all match and return0: preparation gate, strict doctor, bootstrap93, design130/27, infrastructure2workflows/15pins, **complete Python34**, root **Node4**, diff-check. Their RESULT array is command/log evidence, with no invented immutable execution-commit stamp; fixed input equivalence and correctly rebaselined final STATUS/bootstrap are separately checked. The prior independent nonRust run also covers all34 Python and Node4. The local chain therefore includes Rust fixture test/Clippy, not merely static checks that previously missed the stale policy test.

Reviewer additionally executed only the lightweight exact final-source runtime-policy **6 cases** and root Node **4 cases**, both exit0, no skips/failures. Those six are a selected subset of34 and are not added as another full suite. No Cargo/product tests or dependency installation were run. The actual Environment workflow also installs dependencies and, on Mac, probes Xcode/SDK; this Linux local chain does not establish a newly completed hosted workflow, those Mac prerequisites or new three-platform CI.

## Original failures remain failed and preserved

The bb Environment failure packet has SHA `35650054090644d8369ecb49ad4e860d314fa5c8a05e7823e94879c5f0d4eb8d`,43 safe regular unique members. All declared payload hashes/sizes were rechecked. Original Linux/ARM/Intel Python logs each retain **Ran34 / FAILED(failures=2)** and the stale45-versus60 / invalid46 assertion failures. Old local34/twoFAIL remains in the nonRust package. Prior omission and parser corrections remain as historical review records. Old failed outcomes are never rewritten to pass.

Saved actual terminal metadata retains **Product40799957452 all4 success and Environment40799957732 all3 failure**, bound to featurebb343 and actual checkout `1a7682e60354d4a80200ea487725edfc99068a4c`, tree `f85f9e19513d1ecfd85a058e5f6fd14ba4ed1123`. This old head cannot merge with required Environment failures. The **new a945 candidate still needs its own accurate seven CI results**; source equality does not transfer old green Product checks or packages to the new head.

## Product audit subset and installer provenance

The new Git subset `CI_BB343_PRODUCT_NATIVE_AUDIT.tar.gz` is exactly355 safe regular unique members, SHA **`244781d5ac97e69636efbd0913058f8c96c84a8b1d7ce26dce40b382b0ba3b16`**. Every retained original payload hash/size/mode matches the original product manifest. All50 Native PNGs are present and rehashed; **none were viewed again by this reviewer**. Raw logs/reports/metadata and the prior review's actual-image dimension remain intact.

The subset explicitly omits **nine binary entries: seven official ZIPs and two raw DMGs**. It is not a self-contained installer archive. Full original363-member archive remains locally present with SHA **`08b76e059993338f484478948384355f8e4dd206645bd2be1680dceae8ed675e`**; its compressed bytes were streaming-hashed. All nine omitted original files also remain present and were streaming-hashed against the declared sizes/digests. Neither the full archive nor the200MB evidence tree was copied.

`DEVELOPMENT_DELIVERY_BB343.md` exact SHA **`34369f049507672a9a08529aec1096e1b307425fc3a2a21ccfd4c2dc3f5085fc`** agrees with actual local original bytes, preserved official API metadata and build records:

| Architecture | Official development artifact | Actual DMG bytes / SHA-256 | Original Mac signing inspection |
|---|---|---|---|
| ARM |[12866209525](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957452/artifacts/12866209525)|8577813 / d1e479eb701efa13ea7d62e9d5f6496f0877764aa09e1dbfe2ad0feca25af0d8|ad-hoc/linker-signed; TeamIdentifier not set; resources unsealed|
| Intel |[12866603973](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957452/artifacts/12866603973)|8984245 / c23937dd321e890cd881f9a10097d4ff15b53ab8840a9d2f95b4b6dc7624bf25|not signed at all|

The two original development ZIP digests/sizes match the preserved official API responses and run/head; SHA256SUMS, exact checkout/architecture build records, actual DMG hashes and UDIF koly trailers match. Signing statements come from the original Mac inspection/build records, not new Linux codesign validation. Neither package has Developer ID/notarization. **Manual mounting/installing/launching these default apps was not done**. Existing manual checklist, minimum-OS/Provider/remote/full-design limits remain truthful. No package is claimed to be a945-built.

## Actual reviewer operations and result

`python3 -B audit.py`: exit0, **1578 finite source/record/archive checks**, including complete streamed member inventories. `python3 -B check-light.py`: exit0, actual runtime6 and Node4 checks, original official metadata/ZIP URL bindings and three original34/twoFAIL logs. `light-commands.json` records exact argv/cwd/source/exit/output hashes. Source remained a945/clean after the selected tests; Python bytecode disabled and Node temporary/cache files restricted to this review directory.

Only small source/command metadata was copied into this own directory. No workflow trigger, remote/API acquisition, paid resource/model/account/credential change, formal Approve, push/merge, Cargo/browser/daemon, shared target, product/STATUS/source/lock/schema mutation, image reinspection, recursive agent or new management system. Core's heavy fourth regression remains separate and was not disturbed. This closes the requested source/records review; Root owns normal feature push and verification of all new exact CI checks before any protected merge.
