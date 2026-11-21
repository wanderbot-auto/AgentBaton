# Integration browser candidate manifest schema 19

## Source and artifact binding

- Base `98413ca1f6e1006a74d5c6ff7f40494b0509da7c`.
- Tested source `c79a31fbc54f9dff846cfaeccbfb8a93b9e86043`.
- Current raw driver SHA-256 `d7d163f945431abe328922c3f6e378c7665d3d37b9f8accbdc2f8a04454526be`.
- The new exact single anchor restores base Git bytes with SHA-256
  `e5036fd15afa431228a9b11553b0602e144636dda58fecf4fbbbcaac8633f6f3`. The retained two-anchor layer then
  restores the earlier Git bytes with SHA-256 `0131bee1842f3856507def3f41d4d45f0a9b38511167886461fc96d8a32d37fe`.
  [Git byte proof](logs/source-bytes.json)
- Same-commit release ARM64 `batond` SHA-256 `355fc7d88894b475a391e11d5cba70ad1ab6a21a3011872b20b50a4b79a53f58`.
  `baton` SHA-256 `6f70be052ebe1c072b8cc23f62a0b75f768d598ecae36f55311b4752f42f201c`. The Cargo build used
  `cargo build --release --offline --locked --bin baton --bin batond`.
  The frontend used locked local npm install and the existing build entry.
  [Build and tool provenance](logs/tool-provenance.json)
- macOS 26.6.1 ARM64, Python 3.13.2, Playwright
  1.61.0 and Chromium
  149.0.7827.55. Existing installed macOS packages and Chromium
  were copied into task-private paths with verified hashes and checked by
  `pip check`. The checked-in Linux browser dependency lock stayed fixed.
  [Local input and output hashes](logs/local-artifacts.json)

## Verification

| Check | Result |
| --- | --- |
| Prior real CLI at the fixed baseline driver bytes | Exit 2 at `candidate_schema_guard`, runtime `NOT_RUN` |
| Exact fresh sidecar authority at the candidate | Real private Core snapshot and read-only SQLite both return integer 19, then a matching clean-source manifest is written |
| Standard CLI preflight on correct inputs | All source, mode, build and asset gates passed. Without `--run` the CLI stopped at `run_required` |
| Standard CLI negatives on those inputs | 18, 20, boolean, float 19.0 and string 19 refused before runtime. Wrong Core source, executable checksum and group-writable tool also refused |
| Current and historical tests | 95 passed |
| Complete infrastructure | 949 total with 933 passed and 16 existing skips |
| Standard CLI `--run --case all` | Exit 0, both actual Chromium cases passed |

The normal case reached `confirmed_local_ref` with validator exit 0, three
stages, and one compare-and-swap attempt. The command-failure case reached
`failed` with validator exit 7, two stages, and zero compare-and-swap attempts.
Both recorded unchanged source, Git index and original Command records.
Each case used the test bridge, its own private daemon and generated ACP. The
IPC transcript in each case contains successful schema19 snapshots. Each
case produced six screenshots and verified ownership and cleanup. The two
desktop terminal-state screenshots visually show the expected states.
[Normal report](logs/browser-all--normal--report.json) and
[command-failure report](logs/browser-all--command-failure--report.json)

The prior failure report is retained and matches the baseline Git driver.
The current read guards, original proof/timeout rules, report schema and all CI
files are unchanged. The unchanged raw report dimension still says Core18.
The actual schema19 proof comes from the preflight's Core/SQLite observation
and the IPC snapshots from each runtime. This macOS run covers the standard
local browser `--run --case all` without the optional notification probe.
Linux CI, outer schema probing, evidence projection, Native App and live
Provider acceptance remain separate.
[Prior refusal](logs/baseline-preflight-command.json) and
[current CLI command](logs/browser-all-command.json)

No test-owned process remains. The committed source preview against
`ec2ca48b12241451905157240238987c821e2b2a` is conflict-free. [Archive index](LOGS.json) records
37 archived receipts and logs. The local artifact index records the
12 PNGs, raw private transcripts and CLI refusal reports for independent review.
