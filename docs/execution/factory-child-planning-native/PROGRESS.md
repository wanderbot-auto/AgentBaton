# Progress

Source8d82dc789347fcc3bdbc0187cea53007324bf07a changes only Native approval.rs and lib.rs. The new true branch checks every policy/binding top-level field, exact selected outputs and writable output paths, fixed planning-only rule IDs, selected validator references/origin identity and false execution/workspace grants. It displays the entire server snapshot via the original page mechanism and keeps the original private finalize/CAS/receipt handling unchanged. Old false requests, action text and fixtures are retained byte-for-byte; explicit null is not normalized. Root confirmed UI omission forNone.

The four fixed Tauri Owner commands forward narrow parameters only. Submit preserves original plan text plus plan revision, scope revision and key. No caller/Agent credential, generic method, Goal route or Owner-finalize endpoint is added. List forwards bounded references, not concatenated raw plans.

46 pure static/DTO/mechanical controls pass; standalone Rustfmt parses/checks both files and diff check passes. Six planning and two bridge Rust tests are authored but NOT_RUN. No Cargo/typecheck/Native behavior/Core IPC/browser/macOS evidence exists for this candidate yet.

Initial DTO evidence capture rejected a hash mismatch. The frozen811 and09a copies prove only the listRefs comment changed; all public field bytes remain identical. This failed provenance assertion is recorded rather than relabeling a moving file. Core confirmed source_runs0/[] fields do not assert no historical validator runs. Digest computation and actual original-policy/freshness enforcement remain Core responsibilities. Independent review and a Native Rust lease are next.
