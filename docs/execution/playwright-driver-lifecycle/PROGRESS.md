# Progress

Source frozen: `65f7fd26bec8a88307178a69381389de6188efaa`. Only `.github/workflows/product.yml` changes: asyncio/public async Playwright import, async metadata helper and asyncio.run. The same Chromium executable path is passed to unchanged Goal/Graph subprocesses. No extra browser warmup, sleep, retry, timeout, stderr suppression or private-task operation is added.

Four actual local driver-only controls completed. Old sync: two exit0 runs both reproduced the exact pending init():344/Future TargetClosedError; init remained unfinished while its loop was closed. Async: two exit0 runs both had empty stderr, initialized task/future complete and zero callbacks before return. All four returned the same path. No Chromium/Core/network API was used. This proves the demonstrated metadata-only path can produce the symptom, not unique old-CI process attribution or future all-platform stability.

The actual candidate parent AST parsed/compiled without execution. All three subprocess call ASTs and all workflow YAML outside the one inline run are identical. Exact byte replacement and unchanged Environment/locks/bootstrap/STATUS/permission checks pass. Infrastructure static validation15pins and source diff checks exit0. The checker initially used job id ui rather than actual frontend; its KeyError/source/log are retained, and only that selector was corrected.

Frozen investigation and validation scripts/raw outputs/source hashes are losslessly archived. No source in the original fb7 tree, Cargo/target, current CI, remote PR, STATUS or bootstrap registration was changed. This leaf is authored after the investigation and still requires a separate independent code review. New exact CI/browser/macOS results are not claimed.
