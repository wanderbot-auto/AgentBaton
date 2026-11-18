# Conversation schema19 regression evidence

## Source and production boundary

- Fixed base `51b9d762b2acc6ffa0e1c31411bb154ed4326b99`.
- Tested source `9d1d1d99b17982b0acc4b275179e5edff5820821`.
- The live history test changes a single schema expectation to 19. Paging,
  snapshot sequence, bounds and revision assertions are byte-identical.
- Only the three claimed test files changed. The retained history planner,
  production Core, current Store, original host/conversation and genuine18
  fixtures and lock are byte-identical.
  [Source verification](logs/unchanged-source.json)
- The new test adapter first proves the exact owned empty Host19 schema. It
  removes the Host increment inside a transaction and requires the unchanged
  Conversation18 proof before commit. The legacy inverse still receives
  structural schema18. Controlled fixtures are generated locally. The
  separately sealed P0 fixture is the genuine earlier-program evidence.

## Results

| Check | Fixed baseline | Final source |
| --- | --- | --- |
| Conversation history library | 12 passed, 1 failed, exit 101 | 13 passed |
| Controlled Conversation18 inverse and Store migration | 1 passed, 2 failed, exit 101 | 6 passed |
| Host registry migration library | | 18 passed |
| Project history retention | | 7 passed |
| Genuine schema18 migration/recovery and accepted seals | | 2 passed, including 8 first-seal refusals |
| Workspace format and locked offline strict Clippy for the edited test target | | Passed |

The historical negative cases now reach their intended proof failures on
version 18, with exact rejection reasons, unchanged SQLite images and
unchanged main-file bytes. A separate unmodified Store19 input explicitly
fails the old 18 guard. The original caller-owned 18-to-17 rollback remains
covered. Actual controlled 17-to-19 and 18-to-19 opens compare retained schema
and typed rows independently of the added Host table and index objects.
The 0600 migration backups match the corresponding pre-migration images.
Reopening preserves the current image and original backups.

The first candidate `ada7d62b68befa27a221979e795f47067515ae50` fixed the baseline failure mechanisms and
exposed two backup read refusals with SQLite extended code 1550. macOS
temporary roots had a /var alias. The final test resolves the owned parent
before opening the backup with read-only NOFOLLOW. The guard, first red and
final green are retained.
[Baseline pagination](logs/baseline-history.log.gz) ·
[Baseline legacy tests](logs/baseline-schema18.log.gz) ·
[First candidate](logs/candidate-schema18.log.gz) ·
[Final legacy tests](logs/final-candidate-schema18.log.gz)

## Genuine historical data

The unchanged feature-gated target used the accepted P0 database
`613e624582d6437576bbe53fd773be5c880c5d7e2aa424c4fd6931817e77c1f1` from old source
`a4e03303b54bd5a5544131e95ad39662f8310e29`, with the pinned original frozen executables.
The existing manifests bind those executable paths, so the tools were used
read-only in place and the SQLite input and receipts were copied into the task.
Store19 retains the original rows. The confirmed private pre-v19 backup
SHA-256 is `2ef73ba584bd368923a5834bb0e84189f414cd43f739bfe0a020fd4777b9a71d`. Its copy opened under the exact frozen
schema18 daemon and CLI, returning API 1, schema18 and sequence 5.
The test verifies normal process exit and preserves the original seals.
The recovered genuine data contains old project/task/journal/idempotency
rows. Conversation18-specific negative inputs come from the controlled test.
[Input pins](logs/actual18-inputs.json) ·
[Genuine migration receipt](logs/genuine18-artifacts--migration-report.json) ·
[Local database artifact hashes](logs/genuine18-artifact-hashes.json)

[Final ARM64 test binary hashes](logs/final-binaries.json) ·
[Archive index and original command receipts](LOGS.json). Baseline binary
hashes were captured before Cargo reused those output paths.
The latest target preview against `4be7d4493a3e82624e2eb45329ec5c358503486a` is conflict-free.
